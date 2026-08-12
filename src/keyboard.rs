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
        CallNextHookEx, KBDLLHOOKSTRUCT, WM_KEYDOWN, WM_SYSKEYDOWN,
    };

    if code >= 0 && matches!(wparam as u32, WM_KEYDOWN | WM_SYSKEYDOWN) {
        // SAFETY: For WH_KEYBOARD_LL key messages Windows specifies that
        // `lparam` points to a valid KBDLLHOOKSTRUCT for this callback.
        let event = unsafe { &*(lparam as *const KBDLLHOOKSTRUCT) };
        if is_wasd_virtual_key(event.vkCode) {
            if let Some(hook_state) = WINDOWS_HOOK_STATE.get() {
                let mut hook_state = hook_state.lock();
                if let Some(state) = hook_state.as_mut() {
                    let now = Instant::now();
                    state.tracker.record_activity(now);
                    // Publish directly in the KeyDown callback: no polling interval is
                    // involved between the physical event and clearing the idle flag.
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
            Ok(event) if is_wasd_activity(event.kind) => tracker.record_activity(event.time),
            Ok(_) | Err(RecvTimeoutError::Timeout) => {}
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
fn is_wasd_activity(kind: EventKind) -> bool {
    match kind {
        EventKind::KeyDown(key) | EventKind::KeyRepeat(key) => {
            matches!(key, Key::W | Key::A | Key::S | Key::D)
        }
        EventKind::KeyUp(_) => false,
    }
}

#[cfg(any(target_os = "windows", test))]
fn is_wasd_virtual_key(key: u32) -> bool {
    matches!(key, 0x57 | 0x41 | 0x53 | 0x44)
}

#[derive(Debug)]
struct WasdIdleTracker {
    last_activity: Instant,
    active_round: Option<u64>,
    longest_standstill: Duration,
    completed_round: Option<(u64, Duration)>,
}

impl WasdIdleTracker {
    fn new(started_at: Instant) -> Self {
        Self {
            last_activity: started_at,
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
            self.longest_standstill = self
                .longest_standstill
                .max(now.saturating_duration_since(self.last_activity));
            self.completed_round = Some((previous_round, self.longest_standstill));
        }
        if round.is_some() {
            // A new combat round must never inherit idle time accumulated in
            // the lobby or in the previous round.
            self.last_activity = now;
            self.longest_standstill = Duration::ZERO;
        }
        self.active_round = round;
    }

    fn record_activity(&mut self, at: Instant) {
        if self.active_round.is_some() {
            self.longest_standstill = self
                .longest_standstill
                .max(at.saturating_duration_since(self.last_activity));
        }
        self.last_activity = self.last_activity.max(at);
    }

    fn is_idle(&self, now: Instant) -> bool {
        self.active_round.is_some()
            && now.saturating_duration_since(self.last_activity) >= WASD_IDLE_THRESHOLD
    }

    fn sample(&self, now: Instant) -> WasdMetricSample {
        let current_longest = self.active_round.map(|_| {
            self.longest_standstill
                .max(now.saturating_duration_since(self.last_activity))
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
    fn only_wasd_down_and_repeat_events_count_as_activity() {
        assert!(is_wasd_activity(EventKind::KeyDown(Key::W)));
        assert!(is_wasd_activity(EventKind::KeyDown(Key::A)));
        assert!(is_wasd_activity(EventKind::KeyRepeat(Key::S)));
        assert!(is_wasd_activity(EventKind::KeyRepeat(Key::D)));
        assert!(!is_wasd_activity(EventKind::KeyUp(Key::W)));
        assert!(!is_wasd_activity(EventKind::KeyDown(Key::Space)));
    }

    #[test]
    fn windows_virtual_key_filter_accepts_exactly_wasd() {
        for key in [0x57, 0x41, 0x53, 0x44] {
            assert!(is_wasd_virtual_key(key));
        }
        for key in [0x20, 0x45, 0x25, 0x00] {
            assert!(!is_wasd_virtual_key(key));
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
    fn every_wasd_activity_immediately_resets_the_sliding_window() {
        let start = Instant::now();
        let mut tracker = WasdIdleTracker::new(start);
        tracker.set_active_round(Some(1), start);
        assert!(tracker.is_idle(start + Duration::from_secs(10)));

        let key_press = start + Duration::from_secs(11);
        tracker.record_activity(key_press);
        assert!(!tracker.is_idle(key_press));
        assert!(!tracker.is_idle(key_press + Duration::from_millis(9_999)));
        assert!(tracker.is_idle(key_press + Duration::from_secs(10)));
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

        tracker.record_activity(start + Duration::from_secs(4));
        tracker.record_activity(start + Duration::from_secs(11));
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
