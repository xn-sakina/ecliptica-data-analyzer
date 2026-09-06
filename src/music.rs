//! Current system media metadata for message templates.

use std::{sync::Arc, thread};

#[cfg(target_os = "windows")]
use std::{sync::atomic::Ordering, time::Duration};

use parking_lot::RwLock;

use crate::{analysis::GameSnapshot, runtime::SharedState};

#[cfg(target_os = "windows")]
const POLL_INTERVAL: Duration = Duration::from_secs(1);
#[cfg(target_os = "windows")]
const OPERATION_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct MusicState {
    title: String,
    artist: String,
    playing: bool,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct SharedMusic(Arc<RwLock<MusicState>>);

impl SharedMusic {
    #[cfg(any(target_os = "windows", test))]
    fn replace(&self, state: MusicState) {
        *self.0.write() = state;
    }

    pub(crate) fn apply_to(&self, snapshot: &mut GameSnapshot) {
        let state = self.0.read();
        snapshot.music_title.clone_from(&state.title);
        snapshot.music_artist.clone_from(&state.artist);
        snapshot.has_music = state.playing;
    }
}

#[cfg(target_os = "windows")]
pub fn spawn(shared: SharedState) -> thread::JoinHandle<()> {
    thread::Builder::new()
        .name("ecliptica-music-reader".to_owned())
        .spawn(move || run_windows(shared))
        .expect("failed to start music reader")
}

#[cfg(not(target_os = "windows"))]
pub fn spawn(_shared: SharedState) -> thread::JoinHandle<()> {
    thread::Builder::new()
        .name("ecliptica-music-reader".to_owned())
        .spawn(|| {})
        .expect("failed to start music reader")
}

#[cfg(target_os = "windows")]
fn run_windows(shared: SharedState) {
    use windows::{
        Media::Control::{
            GlobalSystemMediaTransportControlsSessionManager,
            GlobalSystemMediaTransportControlsSessionPlaybackStatus,
        },
        Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize, RoUninitialize},
    };

    // The media APIs are WinRT APIs and this worker owns its apartment for its
    // entire lifetime. S_FALSE (already initialized) is also a successful HRESULT.
    if let Err(error) = unsafe { RoInitialize(RO_INIT_MULTITHREADED) } {
        tracing::warn!(%error, "failed to initialize Windows media reader");
        return;
    }
    struct ApartmentGuard;
    impl Drop for ApartmentGuard {
        fn drop(&mut self) {
            unsafe { RoUninitialize() };
        }
    }
    let _apartment = ApartmentGuard;

    // Polling deliberately avoids event subscriptions: there are no listener
    // tokens to unregister, and every WinRT object is owned by this worker.
    while !shared.shutdown.load(Ordering::Relaxed) {
        let manager = GlobalSystemMediaTransportControlsSessionManager::RequestAsync()
            .ok()
            .and_then(|operation| wait_for_operation(operation, &shared));
        let Some(manager) = manager else {
            shared.music.replace(MusicState::default());
            shared.refresh_music();
            sleep_interruptible(&shared, Duration::from_secs(5));
            continue;
        };

        while !shared.shutdown.load(Ordering::Relaxed) {
            let state = match manager.GetCurrentSession() {
                Ok(session) => {
                    let playing = session
                        .GetPlaybackInfo()
                        .and_then(|info| info.PlaybackStatus())
                        .is_ok_and(|status| {
                            status
                                == GlobalSystemMediaTransportControlsSessionPlaybackStatus::Playing
                        });
                    let properties = session
                        .TryGetMediaPropertiesAsync()
                        .ok()
                        .and_then(|operation| wait_for_operation(operation, &shared));
                    MusicState {
                        title: properties
                            .as_ref()
                            .and_then(|value| value.Title().ok())
                            .map(|value| value.to_string().trim().to_owned())
                            .unwrap_or_default(),
                        artist: properties
                            .as_ref()
                            .and_then(|value| value.Artist().ok())
                            .map(|value| value.to_string().trim().to_owned())
                            .unwrap_or_default(),
                        playing,
                    }
                }
                Err(_) => MusicState::default(),
            };
            shared.music.replace(state);
            shared.refresh_music();
            sleep_interruptible(&shared, POLL_INTERVAL);
        }
    }
}

#[cfg(target_os = "windows")]
fn wait_for_operation<T>(
    operation: windows_future::IAsyncOperation<T>,
    shared: &SharedState,
) -> Option<T>
where
    T: windows::core::RuntimeType + 'static,
{
    use windows_future::AsyncStatus;

    let started = std::time::Instant::now();
    loop {
        match operation.Status() {
            Ok(AsyncStatus::Completed) => {
                let result = operation.GetResults().ok();
                let _ = operation.Close();
                return result;
            }
            Ok(AsyncStatus::Canceled | AsyncStatus::Error) | Err(_) => {
                let _ = operation.Close();
                return None;
            }
            Ok(_) => {}
        }

        if shared.shutdown.load(Ordering::Relaxed) || started.elapsed() >= OPERATION_TIMEOUT {
            let _ = operation.Cancel();
            let _ = operation.Close();
            return None;
        }
        thread::sleep(Duration::from_millis(10));
    }
}

#[cfg(target_os = "windows")]
fn sleep_interruptible(shared: &SharedState, duration: Duration) {
    const SLICE: Duration = Duration::from_millis(100);
    let mut remaining = duration;
    while !remaining.is_zero() && !shared.shutdown.load(Ordering::Relaxed) {
        let step = remaining.min(SLICE);
        thread::sleep(step);
        remaining = remaining.saturating_sub(step);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_music_fields_apply_independently() {
        let shared = SharedMusic::default();
        shared.replace(MusicState {
            title: "Song".to_owned(),
            artist: String::new(),
            playing: true,
        });
        let mut snapshot = GameSnapshot::default();

        shared.apply_to(&mut snapshot);

        assert_eq!(snapshot.music_title, "Song");
        assert_eq!(snapshot.music_artist, "");
        assert!(snapshot.has_music);
    }

    #[test]
    fn unavailable_media_clears_all_previous_values() {
        let shared = SharedMusic::default();
        shared.replace(MusicState {
            title: "Old song".to_owned(),
            artist: "Old artist".to_owned(),
            playing: true,
        });
        shared.replace(MusicState::default());
        let mut snapshot = GameSnapshot {
            music_title: "stale".to_owned(),
            music_artist: "stale".to_owned(),
            has_music: true,
            ..GameSnapshot::default()
        };

        shared.apply_to(&mut snapshot);

        assert!(snapshot.music_title.is_empty());
        assert!(snapshot.music_artist.is_empty());
        assert!(!snapshot.has_music);
    }

    #[test]
    fn paused_media_keeps_metadata_but_disables_the_playing_flag() {
        let shared = SharedMusic::default();
        shared.replace(MusicState {
            title: "Paused song".to_owned(),
            artist: String::new(),
            playing: false,
        });
        let mut snapshot = GameSnapshot::default();

        shared.apply_to(&mut snapshot);

        assert_eq!(snapshot.music_title, "Paused song");
        assert!(snapshot.music_artist.is_empty());
        assert!(!snapshot.has_music);
    }
}
