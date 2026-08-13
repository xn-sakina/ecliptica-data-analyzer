use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::JoinHandle,
};

use crossbeam_channel::{Receiver, Sender, bounded};
use parking_lot::RwLock;

use crate::{
    analysis::GameSnapshot,
    audio::{self, SoundCommand},
    config::AppConfig,
    i18n::TextPair,
    log_reader, osc,
};

pub struct LiveConfig {
    pub value: AppConfig,
    pub revision: u64,
}

#[derive(Debug, Clone)]
pub struct SystemEvent {
    pub level: EventLevel,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventLevel {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, Copy, Default)]
struct WasdMetricState {
    available: bool,
    sample: WasdMetricSample,
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct WasdMetricSample {
    pub(crate) active_round: Option<u64>,
    pub(crate) idle: bool,
    pub(crate) longest_standstill_seconds: u64,
    pub(crate) completed_round: Option<u64>,
    pub(crate) completed_longest_standstill_seconds: u64,
}

impl WasdMetricState {
    fn apply_to(self, snapshot: &mut GameSnapshot) {
        let snapshot_round = wasd_window_round(snapshot);
        snapshot.wasd_listener_available = self.available;
        snapshot.no_wasd_for_10s = self.available
            && snapshot_round.is_some()
            && self.sample.active_round == snapshot_round
            && self.sample.idle;
        if let Some(report) = snapshot.round_report.as_mut() {
            let report_round = snapshot.combat_round_epoch;
            let longest = if self.sample.active_round == Some(report_round) {
                Some(self.sample.longest_standstill_seconds)
            } else if self.sample.completed_round == Some(report_round) {
                Some(self.sample.completed_longest_standstill_seconds)
            } else {
                None
            };
            report.has_longest_standstill_data = self.available && longest.is_some();
            report.longest_standstill_seconds = longest.unwrap_or(0);
        }
    }
}

#[derive(Clone)]
pub struct SharedState {
    pub snapshot: Arc<RwLock<GameSnapshot>>,
    pub config: Arc<RwLock<LiveConfig>>,
    pub shutdown: Arc<AtomicBool>,
    pub events: Sender<SystemEvent>,
    wasd_metric: Arc<RwLock<WasdMetricState>>,
}

impl SharedState {
    pub fn text(&self, pair: TextPair) -> &'static str {
        pair.get(self.config.read().value.language)
    }

    pub fn event(&self, level: EventLevel, message: impl Into<String>) {
        let message = message.into();
        tracing::info!(?level, %message);
        let _ = self.events.try_send(SystemEvent { level, message });
    }

    pub(crate) fn set_wasd_metric(&self, available: bool, sample: WasdMetricSample) {
        *self.wasd_metric.write() = WasdMetricState {
            available,
            sample: if available {
                sample
            } else {
                WasdMetricSample::default()
            },
        };
        self.apply_wasd_metric(&mut self.snapshot.write());
    }

    pub fn apply_wasd_metric(&self, snapshot: &mut GameSnapshot) {
        self.wasd_metric.read().apply_to(snapshot);
    }
}

pub(crate) fn wasd_window_round(snapshot: &GameSnapshot) -> Option<u64> {
    snapshot
        .round_metrics_active
        .then_some(snapshot.combat_round_epoch)
}

pub struct Runtime {
    pub shared: SharedState,
    pub events: Receiver<SystemEvent>,
    pub sounds: Sender<SoundCommand>,
    handles: Vec<JoinHandle<()>>,
}

impl Runtime {
    pub fn start(config: AppConfig) -> Self {
        let (event_tx, event_rx) = bounded(256);
        let (sound_tx, sound_rx) = bounded(8);
        let shared = SharedState {
            snapshot: Arc::new(RwLock::new(GameSnapshot::default())),
            config: Arc::new(RwLock::new(LiveConfig {
                value: config,
                revision: 0,
            })),
            shutdown: Arc::new(AtomicBool::new(false)),
            events: event_tx,
            wasd_metric: Arc::new(RwLock::new(WasdMetricState::default())),
        };
        let handles = vec![
            log_reader::spawn(shared.clone()),
            osc::spawn(shared.clone()),
            audio::spawn(shared.clone(), sound_rx),
            crate::keyboard::spawn(shared.clone()),
        ];
        Self {
            shared,
            events: event_rx,
            sounds: sound_tx,
            handles,
        }
    }

    pub fn shutdown(&mut self) {
        self.shared.shutdown.store(true, Ordering::SeqCst);
        for handle in self.handles.drain(..) {
            if handle.join().is_err() {
                tracing::error!(
                    "{}",
                    crate::i18n::text::BACKGROUND_THREAD_FAILED
                        .get(self.shared.config.read().value.language)
                );
            }
        }
    }
}

impl Drop for Runtime {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::RoundPhase;

    #[test]
    fn idle_metric_cannot_cross_combat_rounds() {
        let previous_round_idle = WasdMetricState {
            available: true,
            sample: WasdMetricSample {
                active_round: Some(7),
                idle: true,
                ..WasdMetricSample::default()
            },
        };
        let mut next_round = GameSnapshot {
            phase: RoundPhase::Combat,
            combat_round_epoch: 8,
            ..GameSnapshot::default()
        };

        previous_round_idle.apply_to(&mut next_round);

        assert!(next_round.wasd_listener_available);
        assert!(!next_round.no_wasd_for_10s);
    }

    #[test]
    fn idle_metric_only_applies_to_its_exact_active_round() {
        let current_round_idle = WasdMetricState {
            available: true,
            sample: WasdMetricSample {
                active_round: Some(8),
                idle: true,
                ..WasdMetricSample::default()
            },
        };
        let mut snapshot = GameSnapshot {
            phase: RoundPhase::Combat,
            round_metrics_active: true,
            combat_round_epoch: 8,
            ..GameSnapshot::default()
        };

        current_round_idle.apply_to(&mut snapshot);
        assert!(snapshot.no_wasd_for_10s);

        snapshot.phase = RoundPhase::Lobby;
        snapshot.round_metrics_active = false;
        current_round_idle.apply_to(&mut snapshot);
        assert!(!snapshot.no_wasd_for_10s);
    }

    #[test]
    fn completed_round_standstill_is_attached_to_its_report_only() {
        let metric = WasdMetricState {
            available: true,
            sample: WasdMetricSample {
                completed_round: Some(8),
                completed_longest_standstill_seconds: 27,
                ..WasdMetricSample::default()
            },
        };
        let mut snapshot = GameSnapshot {
            phase: RoundPhase::Lobby,
            combat_round_epoch: 8,
            round_report: Some(crate::analysis::RoundReport {
                has_duration_data: true,
                has_output_data: true,
                duration_seconds: 60,
                total_damage: 100,
                average_dps: 2.0,
                max_dps: 10,
                effective_dps: 4.0,
                burst_10s_dps: Some(5.0),
                dps_growth_rate: 0.0,
                has_dps_growth_rate: false,
                damage_taken: 3,
                has_longest_standstill_data: false,
                longest_standstill_seconds: 0,
            }),
            ..GameSnapshot::default()
        };

        metric.apply_to(&mut snapshot);
        let report = snapshot.round_report.as_ref().unwrap();
        assert!(report.has_longest_standstill_data);
        assert_eq!(report.longest_standstill_seconds, 27);

        snapshot.combat_round_epoch = 9;
        metric.apply_to(&mut snapshot);
        assert!(
            !snapshot
                .round_report
                .as_ref()
                .unwrap()
                .has_longest_standstill_data
        );
    }
}
