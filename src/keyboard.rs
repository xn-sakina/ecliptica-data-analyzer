use std::{
    sync::atomic::Ordering,
    thread,
    time::{Duration, Instant},
};

#[cfg(target_os = "windows")]
use std::sync::OnceLock;

#[cfg(not(target_os = "windows"))]
use keytap::{EventKind, Key, RecvTimeoutError, Tap};
#[cfg(target_os = "windows")]
use parking_lot::Mutex;

use crate::runtime::EventLevel;
use crate::runtime::{SharedState, WasdMetricSample, wasd_window_round};

const WASD_IDLE_THRESHOLD: Duration = Duration::from_secs(10);
#[cfg(not(target_os = "windows"))]
const LISTENER_POLL_INTERVAL: Duration = Duration::from_millis(100);
#[cfg(target_os = "windows")]
const WINDOWS_IDLE_TIMER_INTERVAL_MS: u32 = 50;

#[cfg(target_os = "windows")]
static WINDOWS_HOOK_STATE: OnceLock<Mutex<Option<WindowsHookState>>> = OnceLock::new();

#[cfg(target_os = "windows")]
struct WindowsHookState {
    shared: SharedState,
    tracker: WasdIdleTracker,
}

pub fn spawn(shared: SharedState) -> thread::JoinHandle<()> {
    thread::Builder::new()
        .name("ecliptica-wasd-listener".to_owned())
        .spawn(move || run(shared))
        .expect("failed to start WASD listener")
}

#[cfg(target_os = "windows")]
fn run(shared: SharedState) {
    shared.set_wasd_metric(true, WasdMetricSample::default());
    let hook_state = WINDOWS_HOOK_STATE.get_or_init(|| Mutex::new(None));
    *hook_state.lock() = Some(WindowsHookState {
        shared: shared.clone(),
        tracker: WasdIdleTracker::new(Instant::now()),
    });

    if let Err(error) = run_windows_hook_loop(&shared) {
        shared.event(
            EventLevel::Error,
            format!(
                "{}: {error}",
                shared.text(crate::i18n::text::WASD_INIT_FAILED)
            ),
        );
    }

    *hook_state.lock() = None;
    shared.set_wasd_metric(false, WasdMetricSample::default());
}

#[cfg(target_os = "windows")]
fn run_windows_hook_loop(shared: &SharedState) -> anyhow::Result<()> {
    use std::ptr::{null, null_mut};
    use windows_sys::Win32::{
        System::LibraryLoader::GetModuleHandleW,
        UI::WindowsAndMessaging::{
            DispatchMessageW, GetMessageW, KillTimer, MSG, SetTimer, SetWindowsHookExW,
            TranslateMessage, UnhookWindowsHookEx, WH_KEYBOARD_LL, WM_TIMER,
        },
    };

    // SAFETY: The hook callback has the required system ABI and remains valid
    // for the entire message loop. The current executable module owns it.
    let hook = unsafe {
        SetWindowsHookExW(
            WH_KEYBOARD_LL,
            Some(windows_keyboard_hook),
            GetModuleHandleW(null()),
            0,
        )
    };
    if hook.is_null() {
        anyhow::bail!("SetWindowsHookExW: {}", std::io::Error::last_os_error());
    }

    // The timer only evaluates the 10-second idle threshold and wakes the
    // loop for shutdown. KeyDown itself updates the flag inside the hook.
    // SAFETY: A thread timer uses a null HWND and is removed before returning.
    let timer = unsafe { SetTimer(null_mut(), 0, WINDOWS_IDLE_TIMER_INTERVAL_MS, None) };
    if timer == 0 {
        // SAFETY: `hook` was returned successfully above and is still owned.
        unsafe { UnhookWindowsHookEx(hook) };
        anyhow::bail!("SetTimer: {}", std::io::Error::last_os_error());
    }

    let mut message = MSG::default();
    let loop_result = loop {
        // SAFETY: `message` is valid writable storage for the duration of the call.
        let result = unsafe { GetMessageW(&mut message, null_mut(), 0, 0) };
        if result == -1 {
            break Err(anyhow::anyhow!(
                "GetMessageW: {}",
                std::io::Error::last_os_error()
            ));
        }
        if result == 0 || shared.shutdown.load(Ordering::Relaxed) {
            break Ok(());
        }
        if message.message == WM_TIMER {
            if let Some(hook_state) = WINDOWS_HOOK_STATE.get() {
                let mut hook_state = hook_state.lock();
                if let Some(state) = hook_state.as_mut() {
                    update_metric(&state.shared, &mut state.tracker, Instant::now());
                }
            }
        }
        // SAFETY: The message was initialized by GetMessageW.
        unsafe {
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    };

    // SAFETY: Both handles were created by this function and are released once.
    unsafe {
        KillTimer(null_mut(), timer);
        UnhookWindowsHookEx(hook);
    }
    loop_result
}

#[cfg(target_os = "windows")]
unsafe extern "system" fn windows_keyboard_hook(
    code: i32,
    wparam: windows_sys::Win32::Foundation::WPARAM,
    lparam: windows_sys::Win32::Foundation::LPARAM,
) -> windows_sys::Win32::Foundation::LRESULT {
    use std::ptr::null_mut;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CallNextHookEx, KBDLLHOOKSTRUCT, WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN, WM_SYSKEYUP,
    };

    let message = wparam as u32;
    if code >= 0 && matches!(message, WM_KEYDOWN | WM_SYSKEYDOWN | WM_KEYUP | WM_SYSKEYUP) {
        // SAFETY: For WH_KEYBOARD_LL key messages Windows specifies that
        // `lparam` points to a valid KBDLLHOOKSTRUCT for this callback.
        let event = unsafe { &*(lparam as *const KBDLLHOOKSTRUCT) };
        if let Some(key) = wasd_virtual_key_bit(event.vkCode) {
            if let Some(hook_state) = WINDOWS_HOOK_STATE.get() {
                let mut hook_state = hook_state.lock();
                if let Some(state) = hook_state.as_mut() {
                    let now = Instant::now();
                    let pressed = matches!(message, WM_KEYDOWN | WM_SYSKEYDOWN);
                    state.tracker.record_key_event(key, pressed, now);
                    // Publish directly in the keyboard callback: press and release
                    // transitions must immediately update the idle window.
                    update_metric(&state.shared, &mut state.tracker, now);
                }
            }
        }
    }

    // SAFETY: Forwarding every hook event is required by the Windows hook contract.
    unsafe { CallNextHookEx(null_mut(), code, wparam, lparam) }
}

#[cfg(not(target_os = "windows"))]
fn run(shared: SharedState) {
    let tap = match Tap::builder().capacity(128).build() {
        Ok(tap) => tap,
        Err(error) => {
            shared.set_wasd_metric(false, WasdMetricSample::default());
            shared.event(
                EventLevel::Warning,
                format!(
                    "{}: {error}",
                    shared.text(crate::i18n::text::WASD_KEYBOARD_INIT_FAILED)
                ),
            );
            return;
        }
    };

    let mut tracker = WasdIdleTracker::new(Instant::now());
    shared.set_wasd_metric(true, WasdMetricSample::default());

    while !shared.shutdown.load(Ordering::Relaxed) {
        match tap.recv_timeout(LISTENER_POLL_INTERVAL) {
            Ok(event) => {
                if let Some((key, pressed)) = wasd_key_event(event.kind) {
                    tracker.record_key_event(key, pressed, event.time);
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => {
                shared.set_wasd_metric(false, WasdMetricSample::default());
                shared.event(
                    EventLevel::Error,
                    shared.text(crate::i18n::text::WASD_INTERRUPTED),
                );
                return;
            }
        }
        update_metric(&shared, &mut tracker, Instant::now());
    }

    // Dropping `tap` after this function returns removes the OS listener and
    // joins keytap's platform thread. Publishing unavailable first ensures no
    // stale `true` value survives a graceful shutdown path.
    shared.set_wasd_metric(false, WasdMetricSample::default());
}

fn update_metric(shared: &SharedState, tracker: &mut WasdIdleTracker, now: Instant) {
    // The no-WASD metric is a combat-round sliding window, not an
    // application-lifetime idle timer. Entering an eligible round starts a
    // fresh 10-second window; lobby and spectator states keep it false.
    let active_round = wasd_window_round(&shared.snapshot.read());
    tracker.set_active_round(active_round, now);
    shared.set_wasd_metric(true, tracker.sample(now));
}

#[cfg(not(target_os = "windows"))]
fn wasd_key_event(kind: EventKind) -> Option<(u8, bool)> {
    match kind {
        EventKind::KeyDown(key) | EventKind::KeyRepeat(key) => {
            wasd_key_bit(key).map(|key| (key, true))
        }
        EventKind::KeyUp(key) => wasd_key_bit(key).map(|key| (key, false)),
    }
}

#[cfg(not(target_os = "windows"))]
fn wasd_key_bit(key: Key) -> Option<u8> {
    match key {
        Key::W => Some(1 << 0),
        Key::A => Some(1 << 1),
        Key::S => Some(1 << 2),
        Key::D => Some(1 << 3),
        _ => None,
    }
}

#[cfg(any(target_os = "windows", test))]
fn wasd_virtual_key_bit(key: u32) -> Option<u8> {
    match key {
        0x57 => Some(1 << 0),
        0x41 => Some(1 << 1),
        0x53 => Some(1 << 2),
        0x44 => Some(1 << 3),
        _ => None,
    }
}

#[derive(Debug)]
struct WasdIdleTracker {
    pressed_keys: u8,
    standstill_started_at: Option<Instant>,
    active_round: Option<u64>,
    longest_standstill: Duration,
    completed_round: Option<(u64, Duration)>,
}

impl WasdIdleTracker {
    fn new(started_at: Instant) -> Self {
        Self {
            pressed_keys: 0,
            standstill_started_at: Some(started_at),
            active_round: None,
            longest_standstill: Duration::ZERO,
            completed_round: None,
        }
    }

    fn set_active_round(&mut self, round: Option<u64>, now: Instant) {
        if round == self.active_round {
            return;
        }
        if let Some(previous_round) = self.active_round {
            self.finish_standstill(now);
            self.completed_round = Some((previous_round, self.longest_standstill));
        }
        if round.is_some() {
            // A new combat round must never inherit idle time accumulated in
            // the lobby or in the previous round. A key already held at round
            // start keeps the standstill window paused until its release.
            self.standstill_started_at = (self.pressed_keys == 0).then_some(now);
            self.longest_standstill = Duration::ZERO;
        } else {
            self.standstill_started_at = None;
        }
        self.active_round = round;
    }

    fn record_key_event(&mut self, key: u8, pressed: bool, at: Instant) {
        let was_pressed = self.pressed_keys != 0;
        if pressed {
            self.pressed_keys |= key;
        } else {
            self.pressed_keys &= !key;
        }
        let is_pressed = self.pressed_keys != 0;

        if self.active_round.is_none() || was_pressed == is_pressed {
            return;
        }
        if is_pressed {
            self.finish_standstill(at);
            self.standstill_started_at = None;
        } else {
            self.standstill_started_at = Some(at);
        }
    }

    fn finish_standstill(&mut self, at: Instant) {
        if let Some(started_at) = self.standstill_started_at {
            self.longest_standstill = self
                .longest_standstill
                .max(at.saturating_duration_since(started_at));
        }
    }

    fn is_idle(&self, now: Instant) -> bool {
        self.active_round.is_some()
            && self.standstill_started_at.is_some_and(|started_at| {
                now.saturating_duration_since(started_at) >= WASD_IDLE_THRESHOLD
            })
    }

    fn sample(&self, now: Instant) -> WasdMetricSample {
        let current_longest = self.active_round.map(|_| {
            self.standstill_started_at
                .map(|started_at| now.saturating_duration_since(started_at))
                .map_or(self.longest_standstill, |current| {
                    self.longest_standstill.max(current)
                })
                .as_secs()
        });
        WasdMetricSample {
            active_round: self.active_round,
            idle: self.is_idle(now),
            longest_standstill_seconds: current_longest.unwrap_or(0),
            completed_round: self.completed_round.map(|(round, _)| round),
            completed_longest_standstill_seconds: self
                .completed_round
                .map(|(_, duration)| duration.as_secs())
                .unwrap_or(0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(not(target_os = "windows"))]
    fn wasd_press_repeat_and_release_events_are_tracked() {
        assert_eq!(wasd_key_event(EventKind::KeyDown(Key::W)), Some((1, true)));
        assert_eq!(wasd_key_event(EventKind::KeyDown(Key::A)), Some((2, true)));
        assert_eq!(
            wasd_key_event(EventKind::KeyRepeat(Key::S)),
            Some((4, true))
        );
        assert_eq!(wasd_key_event(EventKind::KeyUp(Key::D)), Some((8, false)));
        assert_eq!(wasd_key_event(EventKind::KeyDown(Key::Space)), None);
    }

    #[test]
    fn windows_virtual_key_filter_accepts_exactly_wasd() {
        assert_eq!(wasd_virtual_key_bit(0x57), Some(1));
        assert_eq!(wasd_virtual_key_bit(0x41), Some(2));
        assert_eq!(wasd_virtual_key_bit(0x53), Some(4));
        assert_eq!(wasd_virtual_key_bit(0x44), Some(8));
        for key in [0x20, 0x45, 0x25, 0x00] {
            assert_eq!(wasd_virtual_key_bit(key), None);
        }
    }

    #[test]
    fn combat_window_starts_false_and_becomes_true_after_ten_seconds() {
        let start = Instant::now();
        let mut tracker = WasdIdleTracker::new(start);
        assert!(!tracker.is_idle(start + Duration::from_secs(30)));

        let round_start = start + Duration::from_secs(30);
        tracker.set_active_round(Some(1), round_start);
        assert!(!tracker.is_idle(round_start));
        assert!(!tracker.is_idle(round_start + Duration::from_millis(9_999)));
        assert!(tracker.is_idle(round_start + Duration::from_secs(10)));
    }

    #[test]
    fn a_wasd_press_pauses_and_release_restarts_the_sliding_window() {
        let start = Instant::now();
        let mut tracker = WasdIdleTracker::new(start);
        tracker.set_active_round(Some(1), start);
        assert!(tracker.is_idle(start + Duration::from_secs(10)));

        let key_press = start + Duration::from_secs(11);
        tracker.record_key_event(1, true, key_press);
        assert!(!tracker.is_idle(key_press));
        assert!(!tracker.is_idle(key_press + Duration::from_secs(30)));

        let key_release = key_press + Duration::from_secs(30);
        tracker.record_key_event(1, false, key_release);
        assert!(!tracker.is_idle(key_release + Duration::from_millis(9_999)));
        assert!(tracker.is_idle(key_release + Duration::from_secs(10)));
    }

    #[test]
    fn overlapping_wasd_keys_keep_the_idle_window_paused_until_all_are_released() {
        let start = Instant::now();
        let mut tracker = WasdIdleTracker::new(start);
        tracker.set_active_round(Some(1), start);

        tracker.record_key_event(1, true, start + Duration::from_secs(2));
        tracker.record_key_event(2, true, start + Duration::from_secs(3));
        tracker.record_key_event(1, false, start + Duration::from_secs(20));
        assert!(!tracker.is_idle(start + Duration::from_secs(30)));

        tracker.record_key_event(2, false, start + Duration::from_secs(30));
        assert!(!tracker.is_idle(start + Duration::from_secs(39)));
        assert!(tracker.is_idle(start + Duration::from_secs(40)));
    }

    #[test]
    fn each_new_round_gets_a_fresh_window() {
        let start = Instant::now();
        let mut tracker = WasdIdleTracker::new(start);
        tracker.set_active_round(Some(1), start);
        assert!(tracker.is_idle(start + Duration::from_secs(10)));

        let lobby = start + Duration::from_secs(12);
        tracker.set_active_round(None, lobby);
        assert!(!tracker.is_idle(lobby + Duration::from_secs(30)));

        let next_round = lobby + Duration::from_secs(30);
        tracker.set_active_round(Some(2), next_round);
        assert!(!tracker.is_idle(next_round));
        assert!(tracker.is_idle(next_round + Duration::from_secs(10)));
    }

    #[test]
    fn a_new_round_resets_even_without_an_observed_lobby_snapshot() {
        let start = Instant::now();
        let mut tracker = WasdIdleTracker::new(start);
        tracker.set_active_round(Some(7), start);
        assert!(tracker.is_idle(start + Duration::from_secs(10)));

        let next_round = start + Duration::from_secs(20);
        tracker.set_active_round(Some(8), next_round);
        assert!(!tracker.is_idle(next_round));
        assert!(tracker.is_idle(next_round + Duration::from_secs(10)));
    }

    #[test]
    fn longest_standstill_uses_the_largest_gap_in_the_round() {
        let start = Instant::now();
        let mut tracker = WasdIdleTracker::new(start);
        tracker.set_active_round(Some(3), start);

        tracker.record_key_event(1, true, start + Duration::from_secs(4));
        tracker.record_key_event(1, false, start + Duration::from_secs(4));
        tracker.record_key_event(2, true, start + Duration::from_secs(11));
        tracker.record_key_event(2, false, start + Duration::from_secs(11));
        let active = tracker.sample(start + Duration::from_secs(13));
        assert_eq!(active.longest_standstill_seconds, 7);

        tracker.set_active_round(None, start + Duration::from_secs(14));
        let completed = tracker.sample(start + Duration::from_secs(20));
        assert_eq!(completed.completed_round, Some(3));
        assert_eq!(completed.completed_longest_standstill_seconds, 7);
    }

    #[test]
    fn a_round_without_wasd_counts_from_round_start_to_finish() {
        let start = Instant::now();
        let mut tracker = WasdIdleTracker::new(start);
        tracker.set_active_round(Some(4), start);
        tracker.set_active_round(None, start + Duration::from_secs(25));

        let completed = tracker.sample(start + Duration::from_secs(30));
        assert_eq!(completed.completed_longest_standstill_seconds, 25);
    }
}
