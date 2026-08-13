use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use unicode_normalization::UnicodeNormalization;

use crate::log_protocol::{LogParser, ParsedEvent, ProtocolDiagnostic};

/// Keep the last non-zero complete-second DPS visible briefly so it can be read
/// without changing the real 30-second or round averages.
const LATEST_DPS_HOLD_SECONDS: i64 = 3;
/// Mark the player as being in danger when strictly more than 50 incoming
/// damage is logged inside this rolling window.
const RAPID_DAMAGE_WINDOW_SECONDS: i64 = 10;
const RAPID_DAMAGE_DANGER_THRESHOLD: u64 = 50;
/// Mark an active player as idle after this many seconds without any
/// outgoing damage. The timer starts at the round marker when no hit exists.
const NO_DPS_WINDOW_SECONDS: i64 = 10;
/// A hit keeps its surrounding output segment active for this many seconds.
/// Overlapping grace intervals are merged, so walking and long waits do not
/// dilute effective DPS while short gaps inside a combo still count.
const EFFECTIVE_OUTPUT_GRACE_SECONDS: i64 = 3;
const BURST_WINDOW_SECONDS: i64 = 10;
// Model contract, assumptions, confidence gates, and tuning procedure:
// resources/rules/step-estimator.md. Keep that specification and its
// regression baselines in sync whenever changing the constants below.
/// The phase modifier grows faster later in a run.  This small linear term is
/// fitted from the bundled complete-run fixtures; the learned intercept is
/// still taken from the current room so different play speeds can correct it.
const STEP_PHASE_ACCELERATION: f64 = 0.05;
const MIN_STEP_PHASE_DELTA: f64 = 0.025;
const MAX_STEP_PHASE_DELTA: f64 = 0.16;
const MIN_STEP_CYCLE_SECONDS: i64 = 180;
const MAX_STEP_CYCLE_SECONDS: i64 = 1_800;
/// Time may regularize the learned phase curve when it agrees with the room's
/// observed phase deltas.  Once the implied intercept differs by this much,
/// the duration prior is treated as incompatible and receives zero weight.
const STEP_TIME_PRIOR_REJECTION_DELTA: f64 = 0.025;
const MAX_STEP_TIME_PRIOR_WEIGHT: f64 = 0.2;
/// A full run is normally a little over two hours.  Reserve twenty minutes
/// for Jim and use the remaining time only as a weak prior for the number of
/// pre-Jim stage transitions.  This is never interpreted as time played since
/// the local user joined.
const FULL_RUN_PRIOR_SECONDS: f64 = 135.0 * 60.0;
const FINAL_BOSS_PRIOR_SECONDS: f64 = 20.0 * 60.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RoundPhase {
    Outside,
    Syncing,
    Lobby,
    Combat,
}

impl RoundPhase {
    pub fn label(self) -> &'static str {
        match self {
            Self::Outside => "OUTSIDE",
            Self::Syncing => "SYNCING",
            Self::Lobby => "LOBBY",
            Self::Combat => "COMBAT",
        }
    }

    pub fn display_label(self, language: crate::i18n::Language) -> &'static str {
        match self {
            Self::Outside => crate::i18n::text::PHASE_OUTSIDE.get(language),
            Self::Syncing => crate::i18n::text::PHASE_SYNCING.get(language),
            Self::Lobby => crate::i18n::text::PHASE_LOBBY.get(language),
            Self::Combat => crate::i18n::text::PHASE_COMBAT.get(language),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DataStatus {
    Searching,
    Recovering,
    Live,
    Stale,
    Error,
}

impl DataStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Searching => "SEARCHING",
            Self::Recovering => "RECOVERING",
            Self::Live => "LIVE",
            Self::Stale => "STALE",
            Self::Error => "ERROR",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoundReport {
    /// Whether a usable round-start timestamp was observed or inferred.
    #[serde(default)]
    pub has_duration_data: bool,
    /// Whether at least one outgoing-damage record exists in this report.
    /// Kept separately so a real zero-damage record remains distinguishable
    /// from an incoming-only round with no output sample at all.
    #[serde(default)]
    pub has_output_data: bool,
    /// Full stage duration, from the stage marker to intermission/lobby.
    pub duration_seconds: u64,
    pub total_damage: u64,
    pub average_dps: f64,
    pub max_dps: u64,
    /// Total damage divided by the union of three-second post-hit intervals.
    pub effective_dps: f64,
    /// Highest complete ten-second damage window in this round.
    pub burst_10s_dps: Option<f64>,
    /// Percentage change in effective DPS from the previous comparable round.
    #[serde(default)]
    pub dps_growth_rate: f64,
    /// Whether a previous positive-output round was available for comparison.
    #[serde(default)]
    pub has_dps_growth_rate: bool,
    pub damage_taken: u64,
    /// Whether the global WASD listener observed this round from its active start.
    #[serde(default)]
    pub has_longest_standstill_data: bool,
    /// Longest continuous interval without W, A, S, or D during this round.
    #[serde(default)]
    pub longest_standstill_seconds: u64,
}

impl RoundReport {
    pub fn duration_text(&self) -> String {
        if self.has_duration_data {
            format_duration(self.duration_seconds)
        } else {
            "-".to_owned()
        }
    }

    pub fn average_dps_text(&self) -> String {
        if self.has_output_data {
            format!("{:.1}", self.average_dps)
        } else {
            "-".to_owned()
        }
    }

    pub fn effective_dps_text(&self) -> String {
        if self.has_output_data {
            format!("{:.1}", self.effective_dps)
        } else {
            "-".to_owned()
        }
    }

    pub fn max_dps_text(&self) -> String {
        if self.has_output_data {
            self.max_dps.to_string()
        } else {
            "-".to_owned()
        }
    }

    pub fn burst_10s_dps_text(&self) -> String {
        self.burst_10s_dps
            .map(|value| format!("{value:.1}"))
            .unwrap_or_else(|| "-".to_owned())
    }

    pub fn dps_growth_rate_text(&self) -> String {
        if self.has_dps_growth_rate {
            format!("{:.1}", self.dps_growth_rate)
        } else {
            "0".to_owned()
        }
    }

    pub fn longest_standstill_text(&self) -> String {
        if self.has_longest_standstill_data {
            format_seconds(self.longest_standstill_seconds)
        } else {
            "-".to_owned()
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DpsHistoryPoint {
    /// Whole seconds since this Ecliptica visit started.
    pub elapsed_seconds: u64,
    /// Personal outgoing damage recorded in that second.
    pub dps: u64,
    /// Locally observed combat identity. Zero means lobby/outside combat.
    #[serde(default)]
    pub combat_round_epoch: u64,
    /// Estimated one-based round in the full run when the heuristic is ready.
    #[serde(default)]
    pub estimated_step: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameSnapshot {
    pub phase: RoundPhase,
    /// Latest complete-second personal DPS anywhere inside Ecliptica, including
    /// the upgrade lobby. This is intentionally independent from round metrics.
    #[serde(default)]
    pub realtime_dps: u64,
    /// Whether the visit-wide real-time stream has observed a damage record.
    #[serde(default)]
    pub has_realtime_dps_data: bool,
    /// Complete visit-wide one-second DPS series for the overview chart.
    #[serde(default)]
    pub dps_history: Vec<DpsHistoryPoint>,
    pub latest_dps: u64,
    pub average_dps: f64,
    pub round_average_dps: f64,
    pub round_effective_dps: f64,
    pub round_burst_10s_dps: Option<f64>,
    pub round_damage_taken: u64,
    /// Highest complete-second DPS observed during the current Ecliptica visit.
    pub max_dps: u64,
    /// Whether a damage record has ever been observed during this Ecliptica
    /// visit. Unlike `max_dps > 0`, this preserves a real zero as valid data.
    pub has_max_dps_data: bool,
    /// Personal round metrics are valid only after an explicit lobby marker
    /// followed by an explicit stage marker. World combat signals alone cannot
    /// prove that the local player is alive or participating.
    #[serde(default)]
    pub round_metrics_active: bool,
    /// The completed round retained while the player is back in the upgrade
    /// lobby. It is cleared as soon as the next stage starts.
    pub round_report: Option<RoundReport>,
    /// Whether the completed-round step estimate has enough observations to
    /// be exposed to report templates.
    #[serde(default)]
    pub has_step_estimate: bool,
    /// Estimated one-based combat round in the complete run (not the number of
    /// rounds observed since a late join).
    #[serde(default)]
    pub current_step: u32,
    /// Estimated number of ordinary combat rounds before the Jim round.  Zero
    /// in the lobby means the next combat entry is expected to be Jim.
    #[serde(default)]
    pub until_boss_step: u32,
    /// Whether at least one damage record has been observed in this combat.
    /// Before this becomes true, zero would mean "unknown" rather than real zero DPS.
    pub has_damage_data: bool,
    /// Whether more than 50 incoming damage was logged during the rolling
    /// 10-second window ending at the current snapshot time.
    pub rapid_damage_danger: bool,
    /// Whether an active player in combat has dealt no damage for at least 10
    /// seconds. Lobby, sync, and spectator states stay false.
    pub no_dps_for_10s: bool,
    /// Whether the global WASD listener is active and the idle signal is valid.
    pub wasd_listener_available: bool,
    /// Whether an active combat round has seen no W, A, S, or D key-down/repeat
    /// event in its rolling 10-second window. Entering a round resets it false.
    pub no_wasd_for_10s: bool,
    /// Internal identity used to reset round-scoped live metrics even when a
    /// lobby and the next stage are consumed in the same log-reader batch.
    #[doc(hidden)]
    pub combat_round_epoch: u64,
    pub boss_lock: Option<String>,
    pub boss: Option<String>,
    pub in_ecliptica: bool,
    pub boss_active: bool,
    pub status: DataStatus,
    pub source: Option<String>,
    /// Aggregated compatibility problems observed in the current log visit.
    #[serde(default)]
    pub data_quality: DataQuality,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DataQuality {
    pub degraded: bool,
    pub issues: Vec<DataQualityIssue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataQualityIssue {
    pub code: String,
    pub message: String,
    pub occurrences: u64,
}

impl Default for GameSnapshot {
    fn default() -> Self {
        Self {
            phase: RoundPhase::Outside,
            realtime_dps: 0,
            has_realtime_dps_data: false,
            dps_history: Vec::new(),
            latest_dps: 0,
            average_dps: 0.0,
            round_average_dps: 0.0,
            round_effective_dps: 0.0,
            round_burst_10s_dps: None,
            round_damage_taken: 0,
            max_dps: 0,
            has_max_dps_data: false,
            round_metrics_active: false,
            round_report: None,
            has_step_estimate: false,
            current_step: 0,
            until_boss_step: 0,
            has_damage_data: false,
            rapid_damage_danger: false,
            no_dps_for_10s: false,
            wasd_listener_available: false,
            no_wasd_for_10s: false,
            combat_round_epoch: 0,
            boss_lock: None,
            boss: None,
            in_ecliptica: false,
            boss_active: false,
            status: DataStatus::Searching,
            source: None,
            data_quality: DataQuality::default(),
        }
    }
}

#[derive(Default)]
pub struct Analyzer {
    parser: LogParser,
    snapshot: GameSnapshot,
    realtime_damage_by_second: BTreeMap<i64, u64>,
    damage_by_second: BTreeMap<i64, u64>,
    round_damage_by_second: BTreeMap<i64, u64>,
    damage_taken_by_second: BTreeMap<i64, u64>,
    last_nonzero_dps: Option<(i64, u64)>,
    last_nonzero_realtime_dps: Option<(i64, u64)>,
    dps_idle_since: Option<i64>,
    round_first_damage_second: Option<i64>,
    round_started_second: Option<i64>,
    round_damage_total: u64,
    round_damage_taken_total: u64,
    round_max_dps: u64,
    /// Set only by an explicit lobby/intermission marker and consumed by the
    /// next stage marker. This deliberately avoids inferring local player state
    /// from world-level Stage/Boss restoration logs.
    round_baseline_ready: bool,
    visit_started_second: Option<i64>,
    history_through_second: Option<i64>,
    previous_round_effective_dps: Option<f64>,
    step_estimator: StepEstimator,
    protocol_issues: BTreeMap<String, DataQualityIssue>,
    pending_protocol_diagnostics: Vec<ProtocolDiagnostic>,
}

#[derive(Debug, Clone, Copy)]
struct PhaseObservation {
    second: i64,
    phase: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct StepEstimate {
    current: u32,
    until_boss: u32,
}

#[derive(Debug, Default)]
struct StepEstimator {
    observations: Vec<PhaseObservation>,
    saw_run_origin: bool,
    total_observed_stages: u32,
}

impl StepEstimator {
    fn reset(&mut self) {
        self.observations.clear();
        self.saw_run_origin = false;
        self.total_observed_stages = 0;
    }

    fn observe_stage(&mut self, second: i64, phase: Option<f64>) {
        let Some(phase) = phase.filter(|value| value.is_finite() && (0.0..=1.0).contains(value))
        else {
            return;
        };
        if self
            .observations
            .last()
            .is_some_and(|last| (last.phase - phase).abs() < 0.000_001)
        {
            return;
        }
        // A lower phase means a fresh run/session was encountered without a
        // dependable room-leave marker.  Do not blend two runs.
        if self
            .observations
            .last()
            .is_some_and(|last| phase + 0.01 < last.phase)
        {
            self.reset();
        }
        if phase <= 0.001 {
            self.saw_run_origin = true;
        }
        self.total_observed_stages = self.total_observed_stages.saturating_add(1);
        self.observations.push(PhaseObservation { second, phase });
        if self.observations.len() > 24 {
            self.observations.remove(0);
        }
    }

    fn estimate(&self) -> Option<StepEstimate> {
        let latest = *self.observations.last()?;
        let samples: Vec<(f64, f64)> = self
            .observations
            .windows(2)
            .filter_map(|pair| {
                let elapsed = pair[1].second.saturating_sub(pair[0].second);
                let delta = pair[1].phase - pair[0].phase;
                ((MIN_STEP_CYCLE_SECONDS..=MAX_STEP_CYCLE_SECONDS).contains(&elapsed)
                    && (MIN_STEP_PHASE_DELTA..=MAX_STEP_PHASE_DELTA).contains(&delta))
                .then(|| {
                    (
                        delta - STEP_PHASE_ACCELERATION * pair[0].phase,
                        elapsed as f64,
                    )
                })
            })
            .collect();

        // Two completed transitions are the minimum needed to distinguish a
        // local late-join counter from a stable full-run estimate.
        if samples.len() < 2 {
            return None;
        }
        let mean_intercept =
            samples.iter().map(|(value, _)| value).sum::<f64>() / samples.len() as f64;
        let mean_cycle =
            samples.iter().map(|(_, seconds)| seconds).sum::<f64>() / samples.len() as f64;
        let spread = (samples
            .iter()
            .map(|(value, _)| (value - mean_intercept).powi(2))
            .sum::<f64>()
            / samples.len() as f64)
            .sqrt();
        if spread > 0.035 {
            return None;
        }

        // The two-hour prior describes a complete run, not this user's local
        // session. Convert it to an expected transition count using only
        // observed stage-to-stage cycle duration. It remains useful only while
        // its implied phase curve agrees with the directly observed deltas;
        // extreme fast/slow rooms automatically reduce its weight to zero.
        let prior_transitions = ((FULL_RUN_PRIOR_SECONDS - FINAL_BOSS_PRIOR_SECONDS) / mean_cycle)
            .round()
            .clamp(8.0, 16.0) as u32;
        let prior_intercept = intercept_for_transitions(prior_transitions);
        let time_prior_weight = step_time_prior_weight(mean_intercept, prior_intercept);
        let intercept = (mean_intercept * (1.0 - time_prior_weight)
            + prior_intercept * time_prior_weight)
            .clamp(0.035, 0.095);

        let current = if self.saw_run_origin {
            // All stages since phase zero are present, so this ordinal is
            // authoritative even though the remaining-round projection is not.
            self.total_observed_stages
        } else {
            nearest_step_for_phase(latest.phase, intercept)
        };
        let transitions_to_final = transitions_until_final(latest.phase, intercept);
        Some(StepEstimate {
            current,
            until_boss: transitions_to_final.saturating_sub(1),
        })
    }
}

fn next_estimated_phase(phase: f64, intercept: f64) -> f64 {
    (phase + intercept + STEP_PHASE_ACCELERATION * phase).min(1.0)
}

fn intercept_for_transitions(transitions: u32) -> f64 {
    let mut low = 0.02;
    let mut high = 0.12;
    for _ in 0..32 {
        let middle = (low + high) * 0.5;
        let mut phase = 0.0;
        for _ in 0..transitions {
            phase = next_estimated_phase(phase, middle);
        }
        if phase >= 0.995 {
            high = middle;
        } else {
            low = middle;
        }
    }
    (low + high) * 0.5
}

fn step_time_prior_weight(observed_intercept: f64, prior_intercept: f64) -> f64 {
    let compatibility = (1.0
        - (observed_intercept - prior_intercept).abs() / STEP_TIME_PRIOR_REJECTION_DELTA)
        .clamp(0.0, 1.0);
    MAX_STEP_TIME_PRIOR_WEIGHT * compatibility
}

fn nearest_step_for_phase(target: f64, intercept: f64) -> u32 {
    let mut best_step = 1;
    let mut best_distance = target.abs();
    let mut phase = 0.0;
    for step in 2..=24 {
        phase = next_estimated_phase(phase, intercept);
        let distance = (phase - target).abs();
        if distance < best_distance {
            best_distance = distance;
            best_step = step;
        }
        if phase >= 1.0 {
            break;
        }
    }
    best_step
}

fn transitions_until_final(mut phase: f64, intercept: f64) -> u32 {
    let mut transitions = 0;
    while phase < 0.995 && transitions < 24 {
        phase = next_estimated_phase(phase, intercept);
        transitions += 1;
    }
    transitions
}

impl Analyzer {
    pub fn process_line(&mut self, line: &str) {
        let parsed = self.parser.parse(line);
        if let Some(diagnostic) = parsed.diagnostic {
            self.record_protocol_diagnostic(diagnostic);
        }
        let Some(event) = parsed.event else {
            return;
        };
        match event {
            ParsedEvent::EnterEcliptica { second } => {
                self.reset_all();
                self.snapshot.in_ecliptica = true;
                self.snapshot.phase = RoundPhase::Syncing;
                self.visit_started_second = Some(second);
            }
            ParsedEvent::LeaveRoom { second } => {
                self.finish_visit(second);
            }
            ParsedEvent::Stage { second, phase } => {
                self.update_dps_history(second.saturating_sub(1));
                if self.snapshot.phase == RoundPhase::Combat && self.round_started_second.is_some()
                {
                    self.record_protocol_diagnostic(ProtocolDiagnostic {
                        code: "intermission_missing",
                        message: "战斗中直接观察到下一阶段，未观察到 intermission/lobby 日志；上一回合按不完整数据丢弃并安全开始新回合".to_owned(),
                    });
                }
                self.step_estimator.observe_stage(second, phase);
                if self.round_baseline_ready {
                    self.start_round(second);
                } else {
                    // A Stage emitted during room restoration is
                    // indistinguishable from a newly started Stage. Expose the
                    // world phase, but wait for an explicit lobby -> stage
                    // boundary before collecting personal round metrics.
                    self.start_untracked_round();
                }
                if let Some(estimate) = self.step_estimator.estimate() {
                    self.snapshot.has_step_estimate = true;
                    self.snapshot.current_step = estimate.current;
                    self.snapshot.until_boss_step = estimate.until_boss;
                }
                self.snapshot.in_ecliptica = true;
                self.snapshot.phase = RoundPhase::Combat;
                self.round_baseline_ready = false;
            }
            ParsedEvent::Intermission { second } => {
                self.update_dps_history(second.saturating_sub(1));
                // Repeated lobby markers are common after Jim. Preserve the
                // report already archived by the final-boss death marker and
                // do not reactivate its completed step estimate.
                if self.snapshot.round_metrics_active {
                    self.finish_round(second);
                    if let Some(estimate) = self.step_estimator.estimate() {
                        self.snapshot.has_step_estimate = true;
                        self.snapshot.current_step = estimate.current;
                        self.snapshot.until_boss_step = estimate.until_boss;
                    }
                } else {
                    self.reset_combat();
                }
                self.snapshot.in_ecliptica = true;
                self.snapshot.phase = RoundPhase::Lobby;
                self.round_baseline_ready = true;
            }
            ParsedEvent::Boss { second: _, name } => {
                // Boss is a world-state signal, not evidence that the local
                // player is alive. It may refine an already known/unknown
                // combat phase, but it never starts personal round metrics.
                if self.snapshot.phase != RoundPhase::Lobby {
                    self.snapshot.in_ecliptica = true;
                    self.snapshot.phase = RoundPhase::Combat;
                    self.snapshot.boss_active = true;
                    self.snapshot.boss_lock = None;
                    self.snapshot.boss = Some(name);
                }
            }
            ParsedEvent::BossDefeated { second, name } => {
                // Jim Phase 3 is the actual end of the run. The game's lobby
                // marker arrives much later, while the party is already in the
                // ending/gathering scene, so publish the report immediately.
                if self.snapshot.phase == RoundPhase::Combat && is_jim_final_phase(&name) {
                    self.update_dps_history(second.saturating_sub(1));
                    if self.snapshot.round_metrics_active {
                        self.finish_round(second);
                    } else {
                        self.reset_combat();
                    }
                    self.snapshot.phase = RoundPhase::Lobby;
                    self.snapshot.has_step_estimate = false;
                    self.snapshot.current_step = 0;
                    self.snapshot.until_boss_step = 0;
                    self.round_baseline_ready = true;
                    return;
                }
                if self
                    .snapshot
                    .boss
                    .as_deref()
                    .is_some_and(|boss| boss_object_matches(boss, &name))
                {
                    self.clear_boss();
                }
            }
            ParsedEvent::Ownership { object, player } => {
                if self.snapshot.boss_active
                    && self
                        .snapshot
                        .boss
                        .as_deref()
                        .is_some_and(|boss| boss_object_matches(boss, &object))
                {
                    self.snapshot.boss_lock = Some(player);
                }
            }
            ParsedEvent::Damage { second, amount } => {
                // Damage is a metric, not a round-boundary event. It may help
                // classify a just-joined room that is still Syncing, but must
                // never override an already-known Lobby phase (for example
                // when the player attacks the lobby training dummy).
                if self.snapshot.in_ecliptica {
                    self.snapshot.has_realtime_dps_data = true;
                    let realtime_total = self.realtime_damage_by_second.entry(second).or_default();
                    *realtime_total = realtime_total.saturating_add(amount);
                }
                if self.snapshot.in_ecliptica && self.snapshot.round_metrics_active {
                    self.dps_idle_since = Some(second);
                    self.snapshot.has_damage_data = true;
                    self.snapshot.has_max_dps_data = true;
                    let second_total = self.damage_by_second.entry(second).or_default();
                    *second_total = second_total.saturating_add(amount);
                    let round_second_total = self.round_damage_by_second.entry(second).or_default();
                    *round_second_total = round_second_total.saturating_add(amount);
                    self.round_max_dps = self.round_max_dps.max(*second_total);
                    self.round_started_second.get_or_insert(second);
                    self.round_first_damage_second.get_or_insert(second);
                    self.round_damage_total = self.round_damage_total.saturating_add(amount);
                }
            }
            ParsedEvent::DamageTaken { second, amount } => {
                if self.snapshot.in_ecliptica && self.snapshot.round_metrics_active {
                    let second_total = self.damage_taken_by_second.entry(second).or_default();
                    *second_total = second_total.saturating_add(amount);
                    self.round_damage_taken_total =
                        self.round_damage_taken_total.saturating_add(amount);
                    self.snapshot.round_damage_taken = self.round_damage_taken_total;
                }
            }
        }
    }

    pub fn snapshot_at(&mut self, now_second: i64) -> GameSnapshot {
        let rapid_damage_first = now_second.saturating_sub(RAPID_DAMAGE_WINDOW_SECONDS);
        self.damage_taken_by_second
            .retain(|second, _| *second >= rapid_damage_first);
        let recent_damage_taken = self
            .damage_taken_by_second
            .range(rapid_damage_first..=now_second)
            .fold(0_u64, |total, (_, damage)| total.saturating_add(*damage));
        self.snapshot.rapid_damage_danger = recent_damage_taken > RAPID_DAMAGE_DANGER_THRESHOLD;
        self.snapshot.no_dps_for_10s = self.snapshot.round_metrics_active
            && self
                .dps_idle_since
                .is_some_and(|second| now_second.saturating_sub(second) >= NO_DPS_WINDOW_SECONDS);
        let last_complete = now_second.saturating_sub(1);
        self.update_dps_history(last_complete);
        let first = last_complete.saturating_sub(29);
        self.realtime_damage_by_second.retain(|second, _| {
            *second >= last_complete.saturating_sub(LATEST_DPS_HOLD_SECONDS + 1)
        });
        if let Some(latest) = self.realtime_damage_by_second.get(&last_complete).copied() {
            self.snapshot.realtime_dps = latest;
            if latest > 0 {
                self.last_nonzero_realtime_dps = Some((last_complete, latest));
            }
        } else {
            self.snapshot.realtime_dps = self
                .last_nonzero_realtime_dps
                .filter(|(second, _)| {
                    last_complete.saturating_sub(*second) <= LATEST_DPS_HOLD_SECONDS
                })
                .map(|(_, damage)| damage)
                .unwrap_or(0);
        }
        if let Some(maximum) = self
            .damage_by_second
            .range(..=last_complete)
            .map(|(_, damage)| *damage)
            .max()
        {
            self.snapshot.max_dps = self.snapshot.max_dps.max(maximum);
        }
        self.damage_by_second
            .retain(|second, _| *second >= first - 2);
        if let Some(latest) = self.damage_by_second.get(&last_complete).copied() {
            self.snapshot.latest_dps = latest;
            if latest > 0 {
                self.last_nonzero_dps = Some((last_complete, latest));
            }
        } else {
            self.snapshot.latest_dps = self
                .last_nonzero_dps
                .filter(|(second, _)| {
                    last_complete.saturating_sub(*second) <= LATEST_DPS_HOLD_SECONDS
                })
                .map(|(_, damage)| damage)
                .unwrap_or(0);
        }
        let total: u64 = self
            .damage_by_second
            .range(first..=last_complete)
            .fold(0_u64, |total, (_, damage)| total.saturating_add(*damage));
        self.snapshot.average_dps = total as f64 / 30.0;
        self.snapshot.round_average_dps = self
            .round_first_damage_second
            .map(|first| last_complete.saturating_sub(first).saturating_add(1).max(1))
            .map(|elapsed_seconds| self.round_damage_total as f64 / elapsed_seconds as f64)
            .unwrap_or(0.0);
        let metric_end_second = metric_end_exclusive(&self.round_damage_by_second, now_second);
        let effective_seconds =
            effective_output_seconds(&self.round_damage_by_second, metric_end_second);
        self.snapshot.round_effective_dps = if effective_seconds > 0 {
            self.round_damage_total as f64 / effective_seconds as f64
        } else {
            0.0
        };
        self.snapshot.round_burst_10s_dps = burst_dps(
            &self.round_damage_by_second,
            self.round_first_damage_second,
            metric_end_second,
        );
        self.snapshot.round_damage_taken = self.round_damage_taken_total;
        self.snapshot.clone()
    }

    pub fn reset_all(&mut self) {
        self.reset_combat();
        self.step_estimator.reset();
        self.realtime_damage_by_second.clear();
        self.last_nonzero_realtime_dps = None;
        self.snapshot.realtime_dps = 0;
        self.snapshot.has_realtime_dps_data = false;
        self.snapshot.dps_history.clear();
        self.snapshot.max_dps = 0;
        self.snapshot.has_max_dps_data = false;
        self.snapshot.round_report = None;
        self.snapshot.in_ecliptica = false;
        self.snapshot.phase = RoundPhase::Outside;
        self.round_baseline_ready = false;
        self.visit_started_second = None;
        self.history_through_second = None;
        self.previous_round_effective_dps = None;
        self.protocol_issues.clear();
        self.pending_protocol_diagnostics.clear();
        self.snapshot.data_quality = DataQuality::default();
    }

    /// Returns newly discovered compatibility problems for the runtime event
    /// stream. Aggregated counters remain available on every snapshot.
    pub fn take_protocol_diagnostics(&mut self) -> Vec<ProtocolDiagnostic> {
        std::mem::take(&mut self.pending_protocol_diagnostics)
    }

    fn record_protocol_diagnostic(&mut self, diagnostic: ProtocolDiagnostic) {
        // Do not let a value parsed before a format change masquerade as a
        // current value. Historical aggregates stay intact, while live fields
        // fail closed until a valid signal is observed again.
        match diagnostic.code {
            "damage" => {
                self.snapshot.has_damage_data = false;
                self.snapshot.has_realtime_dps_data = false;
                self.snapshot.latest_dps = 0;
                self.snapshot.realtime_dps = 0;
            }
            "boss" | "boss_defeated" => self.clear_boss(),
            "ownership" => self.snapshot.boss_lock = None,
            _ => {}
        }
        let issue = self
            .protocol_issues
            .entry(diagnostic.code.to_owned())
            .or_insert_with(|| DataQualityIssue {
                code: diagnostic.code.to_owned(),
                message: diagnostic.message.clone(),
                occurrences: 0,
            });
        issue.occurrences = issue.occurrences.saturating_add(1);
        if issue.occurrences == 1 {
            self.pending_protocol_diagnostics.push(diagnostic);
        }
        self.snapshot.data_quality = DataQuality {
            degraded: true,
            issues: self.protocol_issues.values().cloned().collect(),
        };
    }

    fn start_round(&mut self, second: i64) {
        self.reset_combat();
        self.snapshot.combat_round_epoch = self.snapshot.combat_round_epoch.wrapping_add(1).max(1);
        self.snapshot.round_report = None;
        self.snapshot.round_metrics_active = true;
        self.round_started_second = Some(second);
        self.dps_idle_since = Some(second);
    }

    fn start_untracked_round(&mut self) {
        self.reset_combat();
        self.snapshot.combat_round_epoch = self.snapshot.combat_round_epoch.wrapping_add(1).max(1);
        self.snapshot.round_report = None;
    }

    fn finish_round(&mut self, second: i64) {
        if self.round_first_damage_second.is_some() || self.round_damage_taken_total > 0 {
            let duration_seconds = self
                .round_started_second
                .map(|start| second.saturating_sub(start).max(1) as u64)
                .unwrap_or(1);
            // Average DPS still uses the elapsed output span, but that
            // implementation detail is no longer exposed as a report metric.
            let output_elapsed_seconds = self
                .round_first_damage_second
                .map(|start| second.saturating_sub(start).max(1) as u64)
                .unwrap_or(1);
            let metric_end_second = metric_end_exclusive(&self.round_damage_by_second, second);
            let effective_seconds =
                effective_output_seconds(&self.round_damage_by_second, metric_end_second).max(1);
            let effective_dps = self.round_damage_total as f64 / effective_seconds as f64;
            let burst_10s_dps = burst_dps(
                &self.round_damage_by_second,
                self.round_first_damage_second,
                metric_end_second,
            );
            let growth = self
                .round_first_damage_second
                .is_some()
                .then(|| {
                    effective_dps_growth_rate(self.previous_round_effective_dps, effective_dps)
                })
                .flatten();
            self.snapshot.round_report = Some(RoundReport {
                has_duration_data: self.round_started_second.is_some(),
                has_output_data: self.round_first_damage_second.is_some(),
                duration_seconds,
                total_damage: self.round_damage_total,
                average_dps: self.round_damage_total as f64 / output_elapsed_seconds as f64,
                max_dps: self.round_max_dps,
                effective_dps,
                burst_10s_dps,
                dps_growth_rate: growth.unwrap_or(0.0),
                has_dps_growth_rate: growth.is_some(),
                damage_taken: self.round_damage_taken_total,
                // The runtime WASD tracker enriches this report after the
                // analyzer has archived the log-derived combat values.
                has_longest_standstill_data: false,
                longest_standstill_seconds: 0,
            });
            if self.round_started_second.is_some() {
                self.previous_round_effective_dps =
                    self.round_first_damage_second.map(|_| effective_dps);
            }
        } else {
            // Initial lobby/intermission markers and rounds without either
            // personal output or incoming damage must not masquerade as reports.
            self.snapshot.round_report = None;
            if self.round_started_second.is_some() {
                self.previous_round_effective_dps = None;
                self.record_protocol_diagnostic(ProtocolDiagnostic {
                    code: "combat_metrics_missing",
                    message: "完整阶段内未观察到输出或承伤日志；数值按 0/未知处理（可能确实无伤害，也可能是游戏停止打印相关日志）".to_owned(),
                });
            }
        }
        self.reset_combat();
    }

    fn reset_combat(&mut self) {
        // Startup recovery can scan several complete rounds before publishing
        // its first snapshot. Archive the round peak before discarding buckets
        // so the visit-wide maximum is restored without replay side effects.
        self.snapshot.max_dps = self.snapshot.max_dps.max(self.round_max_dps);
        self.damage_by_second.clear();
        self.round_damage_by_second.clear();
        self.damage_taken_by_second.clear();
        self.last_nonzero_dps = None;
        self.dps_idle_since = None;
        self.round_first_damage_second = None;
        self.round_started_second = None;
        self.round_damage_total = 0;
        self.round_damage_taken_total = 0;
        self.round_max_dps = 0;
        self.snapshot.latest_dps = 0;
        self.snapshot.average_dps = 0.0;
        self.snapshot.round_average_dps = 0.0;
        self.snapshot.round_effective_dps = 0.0;
        self.snapshot.round_burst_10s_dps = None;
        self.snapshot.round_damage_taken = 0;
        self.snapshot.has_damage_data = false;
        self.snapshot.rapid_damage_danger = false;
        self.snapshot.no_dps_for_10s = false;
        self.snapshot.round_metrics_active = false;
        self.snapshot.has_step_estimate = false;
        self.snapshot.current_step = 0;
        self.snapshot.until_boss_step = 0;
        self.clear_boss();
    }

    fn clear_boss(&mut self) {
        self.snapshot.boss_active = false;
        self.snapshot.boss = None;
        self.snapshot.boss_lock = None;
    }

    fn finish_visit(&mut self, second: i64) {
        self.update_dps_history(second);
        let completed_history = std::mem::take(&mut self.snapshot.dps_history);
        let completed_quality = self.snapshot.data_quality.clone();
        let completed_issues = self.protocol_issues.clone();
        let pending_diagnostics = std::mem::take(&mut self.pending_protocol_diagnostics);
        self.reset_all();
        self.snapshot.dps_history = completed_history;
        self.snapshot.data_quality = completed_quality;
        self.protocol_issues = completed_issues;
        self.pending_protocol_diagnostics = pending_diagnostics;
    }

    fn update_dps_history(&mut self, through_second: i64) {
        let Some(start) = self.visit_started_second else {
            return;
        };
        // Reject synthetic/far-future snapshot times used by recovery tests;
        // a real visit is expected to stay well below this generous limit.
        if through_second < start || through_second.saturating_sub(start) > 24 * 60 * 60 {
            return;
        }
        let first = self
            .history_through_second
            .map_or(start, |last| last.saturating_add(1));
        for second in first..=through_second {
            self.snapshot.dps_history.push(DpsHistoryPoint {
                elapsed_seconds: second.saturating_sub(start) as u64,
                dps: self
                    .realtime_damage_by_second
                    .get(&second)
                    .copied()
                    .unwrap_or(0),
                combat_round_epoch: (self.snapshot.phase == RoundPhase::Combat)
                    .then_some(self.snapshot.combat_round_epoch)
                    .unwrap_or(0),
                estimated_step: (self.snapshot.phase == RoundPhase::Combat
                    && self.snapshot.has_step_estimate)
                    .then_some(self.snapshot.current_step),
            });
        }
        self.history_through_second = Some(through_second);
    }
}

fn effective_dps_growth_rate(previous: Option<f64>, current: f64) -> Option<f64> {
    let previous = previous?;
    (previous.is_finite() && current.is_finite() && previous > f64::EPSILON)
        .then_some((current - previous) / previous * 100.0)
}

fn is_jim_final_phase(name: &str) -> bool {
    normalized_name(name) == normalized_name("JimBringerPhase3")
}

impl GameSnapshot {
    pub fn realtime_dps_text(&self) -> String {
        if self.has_realtime_dps_data {
            self.realtime_dps.to_string()
        } else {
            "-".to_owned()
        }
    }

    pub fn latest_dps_text(&self) -> String {
        if self.has_damage_data {
            self.latest_dps.to_string()
        } else {
            "-".to_owned()
        }
    }

    pub fn average_dps_text(&self) -> String {
        if self.has_damage_data {
            format!("{:.1}", self.average_dps)
        } else {
            "-".to_owned()
        }
    }

    pub fn round_average_dps_text(&self) -> String {
        if self.has_damage_data {
            format!("{:.1}", self.round_average_dps)
        } else {
            "-".to_owned()
        }
    }

    pub fn round_effective_dps_text(&self) -> String {
        if self.has_damage_data {
            format!("{:.1}", self.round_effective_dps)
        } else {
            "-".to_owned()
        }
    }

    pub fn round_burst_10s_dps_text(&self) -> String {
        self.round_burst_10s_dps
            .map(|value| format!("{value:.1}"))
            .unwrap_or_else(|| "-".to_owned())
    }

    pub fn max_dps_text(&self) -> String {
        if self.has_max_dps_data {
            self.max_dps.to_string()
        } else {
            "-".to_owned()
        }
    }
}

fn effective_output_seconds(damage_by_second: &BTreeMap<i64, u64>, end_exclusive: i64) -> u64 {
    let mut total = 0_u64;
    let mut interval_start: Option<i64> = None;
    let mut interval_end = 0_i64;

    for second in damage_by_second
        .keys()
        .copied()
        .filter(|second| *second < end_exclusive)
    {
        let hit_end = second.saturating_add(EFFECTIVE_OUTPUT_GRACE_SECONDS);
        match interval_start {
            None => {
                interval_start = Some(second);
                interval_end = hit_end;
            }
            Some(start) if second > interval_end => {
                total = total
                    .saturating_add(interval_end.min(end_exclusive).saturating_sub(start) as u64);
                interval_start = Some(second);
                interval_end = hit_end;
            }
            Some(_) => interval_end = interval_end.max(hit_end),
        }
    }

    if let Some(start) = interval_start {
        total = total.saturating_add(interval_end.min(end_exclusive).saturating_sub(start) as u64);
    }
    total
}

fn metric_end_exclusive(damage_by_second: &BTreeMap<i64, u64>, requested_end: i64) -> i64 {
    damage_by_second
        .last_key_value()
        .map(|(second, _)| second.saturating_add(1))
        .unwrap_or(requested_end)
        .max(requested_end)
}

fn burst_dps(
    damage_by_second: &BTreeMap<i64, u64>,
    first_damage_second: Option<i64>,
    end_exclusive: i64,
) -> Option<f64> {
    let first = first_damage_second?;
    if end_exclusive.saturating_sub(first) < BURST_WINDOW_SECONDS {
        return None;
    }

    let last_start = end_exclusive.saturating_sub(BURST_WINDOW_SECONDS);
    let window_damage = |start: i64| {
        let end = start.saturating_add(BURST_WINDOW_SECONDS - 1);
        damage_by_second
            .range(start..=end)
            .fold(0_u64, |total, (_, damage)| total.saturating_add(*damage))
    };
    // With non-negative damage, a maximum window can start at the first hit or
    // end on a hit. Checking those event-derived boundaries avoids iterating
    // every wall-clock second when a recovered snapshot uses a far-future time.
    let mut maximum = window_damage(first);
    for second in damage_by_second
        .keys()
        .copied()
        .filter(|second| *second >= first && *second < end_exclusive)
    {
        let start = second
            .saturating_sub(BURST_WINDOW_SECONDS - 1)
            .clamp(first, last_start);
        maximum = maximum.max(window_damage(start));
    }
    Some(maximum as f64 / BURST_WINDOW_SECONDS as f64)
}

fn format_duration(total_seconds: u64) -> String {
    let hours = total_seconds / 3_600;
    let minutes = total_seconds % 3_600 / 60;
    let seconds = total_seconds % 60;
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes:02}:{seconds:02}")
    }
}

fn format_seconds(total_seconds: u64) -> String {
    total_seconds.to_string()
}

pub fn normalized_name(value: &str) -> String {
    value.nfkc().collect::<String>().trim().to_lowercase()
}

fn boss_object_matches(boss: &str, object: &str) -> bool {
    normalized_name(boss) == normalized_name(object)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Local, TimeZone};

    fn line(second: u32, message: &str) -> String {
        format!("2026.08.03 01:02:{second:02} Debug      -  {message}")
    }

    fn timeline_line(offset_seconds: i64, message: &str) -> String {
        let time = Local
            .with_ymd_and_hms(2026, 8, 3, 1, 0, 0)
            .earliest()
            .unwrap()
            + chrono::Duration::seconds(offset_seconds);
        format!(
            "{} Debug      -  {message}",
            time.format("%Y.%m.%d %H:%M:%S")
        )
    }

    fn timeline_stage(offset_seconds: i64, phase: f64) -> String {
        timeline_line(
            offset_seconds,
            &format!("ECLIPTICA - now in stage: Stage_Demo on phase: {phase} as class: Twinmage"),
        )
    }

    #[test]
    fn step_estimate_waits_for_enough_late_join_evidence_and_then_corrects() {
        let mut analyzer = Analyzer::default();
        analyzer.process_line(&timeline_line(0, "[Behaviour] Entering Room: Ecliptica"));
        analyzer.process_line(&timeline_stage(1, 0.6089));
        analyzer.process_line(&timeline_line(401, "ECLIPTICA - now in intermission"));
        assert!(
            !analyzer
                .snapshot_at(timeline_timestamp(401))
                .has_step_estimate
        );

        analyzer.process_line(&timeline_stage(601, 0.7092));
        analyzer.process_line(&timeline_line(1_001, "ECLIPTICA - now in intermission"));
        assert!(
            !analyzer
                .snapshot_at(timeline_timestamp(1_001))
                .has_step_estimate
        );

        analyzer.process_line(&timeline_stage(1_201, 0.8047));
        analyzer.process_line(&timeline_line(1_601, "ECLIPTICA - now in intermission"));
        let estimate = analyzer.snapshot_at(timeline_timestamp(1_601));
        assert!(estimate.has_step_estimate);
        assert!(estimate.current_step >= 8);
        assert_eq!(estimate.until_boss_step, 1);

        analyzer.process_line(&timeline_stage(1_801, 0.9133));
        analyzer.process_line(&timeline_line(2_201, "ECLIPTICA - now in intermission"));
        let corrected = analyzer.snapshot_at(timeline_timestamp(2_201));
        assert!(corrected.has_step_estimate);
        assert_eq!(corrected.until_boss_step, 0);
    }

    #[test]
    fn full_run_prior_never_treats_late_join_time_as_elapsed_run_time() {
        let mut estimator = StepEstimator::default();
        estimator.observe_stage(10_000, Some(0.6089));
        estimator.observe_stage(10_600, Some(0.7092));
        estimator.observe_stage(11_200, Some(0.8047));

        let estimate = estimator.estimate().expect("two transitions are enough");
        assert!(
            estimate.current >= 8,
            "must locate by phase, not local uptime"
        );
        assert_eq!(estimate.until_boss, 1);
    }

    #[test]
    fn extreme_cycle_time_does_not_override_phase_path_for_late_join() {
        for cycle_seconds in [240, 600, 1_500] {
            let mut estimator = StepEstimator::default();
            estimator.observe_stage(10_000, Some(0.608_929_6));
            estimator.observe_stage(10_000 + cycle_seconds, Some(0.709_211_9));
            estimator.observe_stage(10_000 + cycle_seconds * 2, Some(0.804_672_5));

            let before_jim = estimator.estimate().expect("estimate before Jim");
            assert_eq!(
                (before_jim.current, before_jim.until_boss),
                (11, 1),
                "phase evidence was overridden at {cycle_seconds}s per cycle"
            );

            estimator.observe_stage(10_000 + cycle_seconds * 3, Some(0.913_315_8));
            let jim_lobby = estimator.estimate().expect("estimate in Jim lobby");
            assert_eq!(
                (jim_lobby.current, jim_lobby.until_boss),
                (12, 0),
                "phase evidence was overridden at {cycle_seconds}s per cycle"
            );
        }
    }

    #[test]
    fn extreme_full_run_duration_keeps_jim_boundary_stable() {
        let phases = [
            0.0,
            0.059_993_23,
            0.121_846_1,
            0.184_059_2,
            0.247_373,
            0.331_366_9,
            0.418_190_5,
            0.513_472_8,
            0.608_929_6,
            0.709_211_9,
            0.804_672_5,
            0.913_315_8,
        ];

        // 11 * 480s = 88 minutes to the Jim preparation lobby; 11 * 900s =
        // 165 minutes. Once the origin is observed, both durations must preserve
        // the exact observed round number and the same Jim boundary.
        for cycle_seconds in [480, 900] {
            let mut estimator = StepEstimator::default();
            for (index, phase) in phases.into_iter().enumerate() {
                estimator.observe_stage(index as i64 * cycle_seconds, Some(phase));
                if index == 10 {
                    let estimate = estimator.estimate().expect("estimate before Jim");
                    assert_eq!((estimate.current, estimate.until_boss), (11, 1));
                }
            }

            let estimate = estimator.estimate().expect("estimate in Jim lobby");
            assert_eq!((estimate.current, estimate.until_boss), (12, 0));
        }
    }

    #[test]
    fn slow_run_can_add_rounds_without_time_prior_erasing_them() {
        // A rule-compatible lower-intercept path takes 16 ordinary stages and
        // reaches the Jim preparation lobby after 150 minutes. The denser
        // phase trajectory, rather than the nominal 135-minute duration, is
        // the evidence that the run contains more rounds.
        let phases = [
            0.0,
            0.042_058_558,
            0.086_220_045,
            0.132_589_605,
            0.181_277_644,
            0.232_400_085,
            0.286_078_647,
            0.342_441_138,
            0.401_621_754,
            0.463_761_4,
            0.529_008_028,
            0.597_516_988,
            0.669_451_396,
            0.744_982_524,
            0.824_290_209,
            0.907_563_277,
        ];

        let mut full_run = StepEstimator::default();
        for (index, phase) in phases.into_iter().enumerate() {
            full_run.observe_stage(index as i64 * 600, Some(phase));
            if index == 14 {
                let estimate = full_run.estimate().expect("estimate before Jim");
                assert_eq!((estimate.current, estimate.until_boss), (15, 1));
            }
        }
        let estimate = full_run.estimate().expect("estimate in Jim lobby");
        assert_eq!((estimate.current, estimate.until_boss), (16, 0));

        let mut late_join = StepEstimator::default();
        late_join.observe_stage(0, Some(phases[12]));
        late_join.observe_stage(600, Some(phases[13]));
        late_join.observe_stage(1_200, Some(phases[14]));
        let estimate = late_join.estimate().expect("late join estimate before Jim");
        assert_eq!((estimate.current, estimate.until_boss), (15, 1));

        late_join.observe_stage(1_800, Some(phases[15]));
        let estimate = late_join
            .estimate()
            .expect("late join estimate in Jim lobby");
        assert_eq!((estimate.current, estimate.until_boss), (16, 0));
    }

    #[test]
    fn incompatible_time_prior_loses_all_weight() {
        assert_eq!(step_time_prior_weight(0.06, 0.06), 0.2);
        assert!((step_time_prior_weight(0.06, 0.0725) - 0.1).abs() < 1e-12);
        assert_eq!(step_time_prior_weight(0.06, 0.085), 0.0);
        assert_eq!(step_time_prior_weight(0.06, 0.10), 0.0);
    }

    #[test]
    fn observed_phase_zero_makes_current_step_an_exact_ordinal() {
        let mut estimator = StepEstimator::default();
        estimator.observe_stage(0, Some(0.0));
        estimator.observe_stage(600, Some(0.0604));
        estimator.observe_stage(1_200, Some(0.1228));

        let estimate = estimator.estimate().expect("two transitions are enough");
        assert_eq!(estimate.current, 3);
    }

    #[test]
    fn complete_fixture_like_run_reaches_one_then_zero_before_jim() {
        let mut estimator = StepEstimator::default();
        for (second, phase) in [
            (0, 0.0),
            (600, 0.059_993_23),
            (1_200, 0.121_846_1),
            (1_800, 0.184_059_2),
            (2_400, 0.247_373),
            (3_000, 0.331_366_9),
            (3_600, 0.418_190_5),
            (4_200, 0.513_472_8),
            (4_800, 0.608_929_6),
            (5_400, 0.709_211_9),
            (6_000, 0.804_672_5),
        ] {
            estimator.observe_stage(second, Some(phase));
        }
        let penultimate = estimator.estimate().expect("complete path converges");
        assert_eq!(penultimate.current, 11);
        assert_eq!(penultimate.until_boss, 1);

        estimator.observe_stage(6_600, Some(0.913_315_8));
        let final_lobby = estimator.estimate().expect("complete path stays converged");
        assert_eq!(final_lobby.current, 12);
        assert_eq!(final_lobby.until_boss, 0);
    }

    fn timestamp(second: u32) -> i64 {
        Local
            .with_ymd_and_hms(2026, 8, 3, 1, 2, second)
            .earliest()
            .unwrap()
            .timestamp()
    }

    fn timeline_timestamp(offset_seconds: i64) -> i64 {
        Local
            .with_ymd_and_hms(2026, 8, 3, 1, 0, 0)
            .earliest()
            .unwrap()
            .timestamp()
            + offset_seconds
    }

    #[test]
    fn standstill_duration_is_displayed_as_total_seconds() {
        assert_eq!(format_seconds(12), "12");
        assert_eq!(format_seconds(74), "74");
    }

    #[test]
    fn parses_strike_and_non_strike_into_complete_seconds() {
        let mut analyzer = Analyzer::default();
        analyzer.process_line(&line(
            0,
            "[Behaviour] Entering Room: Ecliptica - Demo Playtest",
        ));
        analyzer.process_line(&line(1, "ECLIPTICA - now in intermission"));
        analyzer.process_line(&line(2, "ECLIPTICA - now in stage: Stage_Demo"));
        analyzer.process_line(&line(5, "Dealing 30 STRIKE damage"));
        analyzer.process_line(&line(5, "Dealing 12 NON-STRIKE damage"));
        let timestamp = Local
            .with_ymd_and_hms(2026, 8, 3, 1, 2, 6)
            .earliest()
            .unwrap()
            .timestamp();
        let snapshot = analyzer.snapshot_at(timestamp);
        assert_eq!(snapshot.latest_dps, 42);
        assert_eq!(snapshot.average_dps, 1.4);
    }

    #[test]
    fn distinguishes_missing_damage_data_from_a_real_zero_second() {
        let mut analyzer = Analyzer::default();
        analyzer.process_line(&line(
            0,
            "[Behaviour] Entering Room: Ecliptica - Demo Playtest",
        ));
        analyzer.process_line(&line(1, "ECLIPTICA - now in intermission"));
        analyzer.process_line(&line(2, "ECLIPTICA - now in stage: Stage_Demo"));
        let base = Local
            .with_ymd_and_hms(2026, 8, 3, 1, 2, 0)
            .earliest()
            .unwrap()
            .timestamp();
        assert_eq!(analyzer.snapshot_at(base + 5).latest_dps_text(), "-");

        analyzer.process_line(&line(5, "Dealing 42 STRIKE damage"));
        assert_eq!(analyzer.snapshot_at(base + 6).latest_dps_text(), "42");
        assert_eq!(analyzer.snapshot_at(base + 7).latest_dps_text(), "42");
        assert_eq!(analyzer.snapshot_at(base + 9).latest_dps_text(), "42");
        assert_eq!(analyzer.snapshot_at(base + 10).latest_dps_text(), "0");
    }

    #[test]
    fn realtime_dps_keeps_tracking_lobby_damage_without_changing_round_metrics() {
        let mut analyzer = Analyzer::default();
        analyzer.process_line(&line(
            0,
            "[Behaviour] Entering Room: Ecliptica - Demo Playtest",
        ));
        analyzer.process_line(&line(1, "ECLIPTICA - now in intermission"));
        analyzer.process_line(&line(5, "Dealing 42 STRIKE damage"));

        let lobby = analyzer.snapshot_at(timestamp(6));
        assert_eq!(lobby.phase, RoundPhase::Lobby);
        assert_eq!(lobby.realtime_dps_text(), "42");
        assert_eq!(lobby.latest_dps_text(), "-");
        assert!(!lobby.has_damage_data);

        analyzer.process_line(&line(10, "ECLIPTICA - now in stage: Stage_Demo"));
        analyzer.process_line(&line(12, "Dealing 30 STRIKE damage"));
        let combat = analyzer.snapshot_at(timestamp(13));
        assert_eq!(combat.realtime_dps_text(), "30");
        assert_eq!(combat.latest_dps_text(), "30");

        analyzer.process_line(&line(14, "ECLIPTICA - now in intermission"));
        let next_lobby = analyzer.snapshot_at(timestamp(15));
        assert_eq!(next_lobby.realtime_dps_text(), "30");
        assert_eq!(next_lobby.latest_dps_text(), "-");
    }

    #[test]
    fn max_dps_preserves_a_real_zero_damage_record_as_available() {
        let mut analyzer = Analyzer::default();
        analyzer.process_line(&line(
            0,
            "[Behaviour] Entering Room: Ecliptica - Demo Playtest",
        ));
        analyzer.process_line(&line(1, "ECLIPTICA - now in intermission"));
        analyzer.process_line(&line(2, "ECLIPTICA - now in stage: Stage_Demo"));
        analyzer.process_line(&line(5, "Dealing 0 STRIKE damage"));

        let snapshot = analyzer.snapshot_at(timestamp(6));
        assert!(snapshot.has_max_dps_data);
        assert_eq!(snapshot.max_dps_text(), "0");

        analyzer.process_line(&line(7, "ECLIPTICA - now in intermission"));
        let report = analyzer
            .snapshot_at(timestamp(8))
            .round_report
            .expect("a real zero-damage record should still produce a report");
        assert!(report.has_output_data);
        assert_eq!(report.average_dps_text(), "0.0");
        assert_eq!(report.max_dps_text(), "0");
        assert_eq!(report.effective_dps_text(), "0.0");

        analyzer.process_line(&line(9, "[Behaviour] OnLeftRoom"));
        let outside = analyzer.snapshot_at(timestamp(10));
        assert!(!outside.has_max_dps_data);
        assert_eq!(outside.max_dps_text(), "-");
    }

    #[test]
    fn rapid_damage_danger_uses_a_rolling_ten_second_strict_threshold() {
        let mut analyzer = Analyzer::default();
        analyzer.process_line(&line(
            0,
            "[Behaviour] Entering Room: Ecliptica - Demo Playtest",
        ));
        analyzer.process_line(&line(1, "ECLIPTICA - now in intermission"));
        analyzer.process_line(&line(2, "ECLIPTICA - now in stage: Stage_Demo"));

        analyzer.process_line(&line(
            5,
            "damage has been taken: 30, from source: projectile1",
        ));
        analyzer.process_line(&line(
            14,
            "damage has been taken: 20, from source: attack_Spit",
        ));
        assert!(!analyzer.snapshot_at(timestamp(14)).rapid_damage_danger);

        analyzer.process_line(&line(14, "damage has been taken: 1, from source:"));
        assert!(analyzer.snapshot_at(timestamp(14)).rapid_damage_danger);
        assert!(analyzer.snapshot_at(timestamp(15)).rapid_damage_danger);
        assert!(!analyzer.snapshot_at(timestamp(16)).rapid_damage_danger);

        analyzer.process_line(&line(17, "damage has been taken: 60, from source: Boss"));
        assert!(analyzer.snapshot_at(timestamp(17)).rapid_damage_danger);
        analyzer.process_line(&line(18, "ECLIPTICA - now in intermission"));
        assert!(!analyzer.snapshot_at(timestamp(18)).rapid_damage_danger);
    }

    #[test]
    fn no_dps_flag_requires_ten_quiet_combat_seconds() {
        let mut analyzer = Analyzer::default();
        analyzer.process_line(&line(
            0,
            "[Behaviour] Entering Room: Ecliptica - Demo Playtest",
        ));
        analyzer.process_line(&line(1, "ECLIPTICA - now in intermission"));
        analyzer.process_line(&line(2, "ECLIPTICA - now in stage: Stage_Demo"));

        assert!(!analyzer.snapshot_at(timestamp(11)).no_dps_for_10s);
        assert!(analyzer.snapshot_at(timestamp(12)).no_dps_for_10s);

        analyzer.process_line(&line(12, "Dealing 30 STRIKE damage"));
        assert!(!analyzer.snapshot_at(timestamp(12)).no_dps_for_10s);
        assert!(!analyzer.snapshot_at(timestamp(21)).no_dps_for_10s);
        assert!(analyzer.snapshot_at(timestamp(22)).no_dps_for_10s);

        analyzer.process_line(&line(23, "ECLIPTICA - now in intermission"));
        assert!(!analyzer.snapshot_at(timestamp(23)).no_dps_for_10s);
    }

    #[test]
    fn no_dps_flag_ignores_late_join_spectators() {
        let mut analyzer = Analyzer::default();
        analyzer.process_line(&line(
            0,
            "[Behaviour] Entering Room: Ecliptica - Active Instance",
        ));
        analyzer.process_line(&line(1, "ECLIPTICA - now in stage: Stage_Active"));

        let snapshot = analyzer.snapshot_at(timestamp(30));
        assert_eq!(snapshot.phase, RoundPhase::Combat);
        assert!(!snapshot.round_metrics_active);
        assert!(!snapshot.no_dps_for_10s);
    }

    #[test]
    fn round_average_uses_real_damage_and_elapsed_complete_seconds() {
        let mut analyzer = Analyzer::default();
        analyzer.process_line(&line(
            0,
            "[Behaviour] Entering Room: Ecliptica - Demo Playtest",
        ));
        analyzer.process_line(&line(1, "ECLIPTICA - now in intermission"));
        analyzer.process_line(&line(2, "ECLIPTICA - now in stage: Stage_Demo"));
        analyzer.process_line(&line(5, "Dealing 60 STRIKE damage"));
        let base = Local
            .with_ymd_and_hms(2026, 8, 3, 1, 2, 0)
            .earliest()
            .unwrap()
            .timestamp();

        assert_eq!(analyzer.snapshot_at(base + 6).round_average_dps, 60.0);
        assert_eq!(analyzer.snapshot_at(base + 8).round_average_dps, 20.0);

        analyzer.process_line(&line(8, "Dealing 20 NON-STRIKE damage"));
        assert_eq!(analyzer.snapshot_at(base + 9).round_average_dps, 20.0);
        analyzer.process_line(&line(10, "ECLIPTICA - now in intermission"));
        assert_eq!(
            analyzer.snapshot_at(base + 11).round_average_dps_text(),
            "-"
        );
    }

    #[test]
    fn archives_round_report_during_intermission_and_clears_it_on_next_stage() {
        let mut analyzer = Analyzer::default();
        analyzer.process_line(&line(
            0,
            "[Behaviour] Entering Room: Ecliptica - Demo Playtest",
        ));
        analyzer.process_line(&line(0, "ECLIPTICA - now in intermission"));
        analyzer.process_line(&line(1, "ECLIPTICA - now in stage: Stage_Demo"));
        analyzer.process_line(&line(5, "Dealing 30 STRIKE damage"));
        analyzer.process_line(&line(5, "Dealing 12 NON-STRIKE damage"));
        let base = Local
            .with_ymd_and_hms(2026, 8, 3, 1, 2, 0)
            .earliest()
            .unwrap()
            .timestamp();
        // The live 30-second map has already discarded the early peak by the
        // time the round ends; the independent round accumulator must retain it.
        analyzer.snapshot_at(base + 45);
        analyzer.process_line(&line(50, "ECLIPTICA - now in intermission"));

        let report = analyzer
            .snapshot_at(i64::MAX)
            .round_report
            .expect("completed round should be archived");
        assert_eq!(report.duration_seconds, 49);
        assert_eq!(report.duration_text(), "00:49");
        assert_eq!(report.total_damage, 42);
        assert!((report.average_dps - 42.0 / 45.0).abs() < f64::EPSILON);
        assert_eq!(report.max_dps, 42);
        assert_eq!(report.effective_dps, 14.0);
        assert_eq!(report.burst_10s_dps, Some(4.2));
        assert_eq!(report.damage_taken, 0);

        analyzer.process_line(&line(55, "ECLIPTICA - now in stage: Stage_Next"));
        assert!(analyzer.snapshot_at(i64::MAX).round_report.is_none());
    }

    #[test]
    fn lobby_combat_noise_preserves_report_until_a_stage_marker() {
        let mut analyzer = Analyzer::default();
        analyzer.process_line(&line(
            0,
            "[Behaviour] Entering Room: Ecliptica - Demo Playtest",
        ));
        analyzer.process_line(&line(0, "ECLIPTICA - now in intermission"));
        analyzer.process_line(&line(1, "ECLIPTICA - now in stage: Stage_First"));
        analyzer.process_line(&line(5, "Dealing 42 STRIKE damage"));
        analyzer.process_line(&line(10, "ECLIPTICA - now in intermission"));

        let report = analyzer.snapshot_at(timestamp(11));
        assert_eq!(report.phase, RoundPhase::Lobby);
        assert!(report.round_report.is_some());
        let completed_epoch = report.combat_round_epoch;

        analyzer.process_line(&line(12, "Dealing 999 STRIKE damage"));
        analyzer.process_line(&line(
            12,
            "damage has been taken: 50, from source: TrainingDummy",
        ));
        analyzer.process_line(&line(
            12,
            "ECLIPTICA - now fighting boss: Maxipuss(Clone) on phase: 1",
        ));

        let still_lobby = analyzer.snapshot_at(timestamp(12));
        assert_eq!(still_lobby.phase, RoundPhase::Lobby);
        assert!(still_lobby.round_report.is_some());
        assert_eq!(still_lobby.combat_round_epoch, completed_epoch);
        assert!(!still_lobby.has_damage_data);
        assert_eq!(still_lobby.round_damage_taken, 0);
        assert!(still_lobby.boss.is_none());

        analyzer.process_line(&line(13, "ECLIPTICA - now in stage: Stage_Next"));
        let combat = analyzer.snapshot_at(timestamp(13));
        assert_eq!(combat.phase, RoundPhase::Combat);
        assert!(combat.round_report.is_none());
        assert_eq!(combat.combat_round_epoch, completed_epoch + 1);
        assert!(combat.round_metrics_active);
    }

    #[test]
    fn jim_phase_three_death_immediately_archives_report_and_hides_step_estimate() {
        let mut analyzer = Analyzer::default();
        analyzer.process_line(&line(
            0,
            "[Behaviour] Entering Room: Ecliptica - Demo Playtest",
        ));
        analyzer.process_line(&line(1, "ECLIPTICA - now in lobby"));
        analyzer.process_line(&line(
            2,
            "ECLIPTICA - now in stage: Stage_Bringer on phase: 1 as class: Twinmage",
        ));
        analyzer.snapshot.has_step_estimate = true;
        analyzer.snapshot.current_step = 12;
        analyzer.snapshot.until_boss_step = 0;
        analyzer.process_line(&line(5, "Dealing 120 STRIKE damage"));
        analyzer.process_line(&line(
            6,
            "ECLIPTICA - now fighting boss: JimBringerPhase3(Clone) on phase: 1",
        ));
        analyzer.process_line(&line(
            10,
            "Boss JimBringerPhase3 dead, personal damage dealt:",
        ));

        let ending_scene = analyzer.snapshot_at(timestamp(10));
        assert_eq!(ending_scene.phase, RoundPhase::Lobby);
        assert_eq!(
            ending_scene
                .round_report
                .as_ref()
                .map(|report| report.total_damage),
            Some(120)
        );
        assert!(!ending_scene.has_step_estimate);
        assert_eq!(ending_scene.current_step, 0);
        assert_eq!(ending_scene.until_boss_step, 0);

        // The real logs repeat the Phase 3 death line and emit the initial
        // lobby marker about a minute later. Neither may replace the report or
        // turn "0 rounds until Jim" back on.
        analyzer.process_line(&line(
            11,
            "Boss JimBringerPhase3 dead, personal damage dealt:",
        ));
        analyzer.process_line(&line(58, "ECLIPTICA - now in lobby"));
        let initial_lobby = analyzer.snapshot_at(timestamp(58));
        assert_eq!(
            initial_lobby
                .round_report
                .as_ref()
                .map(|report| report.total_damage),
            Some(120)
        );
        assert!(!initial_lobby.has_step_estimate);
    }

    #[test]
    fn earlier_jim_phase_death_does_not_end_the_round() {
        let mut analyzer = Analyzer::default();
        analyzer.process_line(&line(0, "ECLIPTICA - now in intermission"));
        analyzer.process_line(&line(1, "ECLIPTICA - now in stage: Stage_Bringer"));
        analyzer.process_line(&line(2, "Dealing 40 STRIKE damage"));
        analyzer.process_line(&line(
            3,
            "Boss JimBringerPhase2 dead, personal damage dealt:",
        ));

        let snapshot = analyzer.snapshot_at(timestamp(3));
        assert_eq!(snapshot.phase, RoundPhase::Combat);
        assert!(snapshot.round_report.is_none());
        assert!(snapshot.has_damage_data);
    }

    #[test]
    fn tracks_effective_dps_ten_second_burst_and_total_damage_taken() {
        let mut analyzer = Analyzer::default();
        analyzer.process_line(&line(
            0,
            "[Behaviour] Entering Room: Ecliptica - Demo Playtest",
        ));
        analyzer.process_line(&line(1, "ECLIPTICA - now in intermission"));
        analyzer.process_line(&line(2, "ECLIPTICA - now in stage: Stage_Demo"));
        analyzer.process_line(&line(5, "Dealing 30 STRIKE damage"));
        assert_eq!(analyzer.snapshot_at(timestamp(5)).round_effective_dps, 30.0);
        analyzer.process_line(&line(6, "Dealing 30 NON-STRIKE damage"));
        analyzer.process_line(&line(
            7,
            "damage has been taken: 20, from source: projectile1",
        ));
        analyzer.process_line(&line(12, "Dealing 40 STRIKE damage"));
        analyzer.process_line(&line(14, "damage has been taken: 35, from source: Boss"));

        let snapshot = analyzer.snapshot_at(timestamp(16));
        assert!((snapshot.round_effective_dps - 100.0 / 7.0).abs() < f64::EPSILON);
        assert_eq!(snapshot.round_burst_10s_dps, Some(10.0));
        assert_eq!(snapshot.round_damage_taken, 55);

        analyzer.process_line(&line(16, "ECLIPTICA - now in intermission"));
        let report = analyzer
            .snapshot_at(timestamp(17))
            .round_report
            .expect("completed round should be archived");
        assert!((report.effective_dps - 100.0 / 7.0).abs() < f64::EPSILON);
        assert_eq!(report.burst_10s_dps, Some(10.0));
        assert_eq!(report.damage_taken, 55);
    }

    #[test]
    fn dps_growth_always_compares_effective_dps() {
        let effective_growth = effective_dps_growth_rate(Some(100.0), 80.0);
        assert_eq!(effective_growth, Some(-20.0));
        assert_eq!(effective_dps_growth_rate(Some(0.0), 80.0), None);
    }

    #[test]
    fn consecutive_completed_rounds_publish_growth_only_after_a_comparable_previous_round() {
        let mut analyzer = Analyzer::default();
        analyzer.process_line(&timeline_line(0, "[Behaviour] Entering Room: Ecliptica"));
        analyzer.process_line(&timeline_line(1, "ECLIPTICA - now in intermission"));
        analyzer.process_line(&timeline_line(2, "ECLIPTICA - now in stage: Stage_One"));
        for second in 3..13 {
            analyzer.process_line(&timeline_line(second, "Dealing 10 STRIKE damage"));
        }
        analyzer.process_line(&timeline_line(13, "ECLIPTICA - now in intermission"));
        let first = analyzer.snapshot_at(timeline_timestamp(13));
        let first_report = first.round_report.as_ref().unwrap();
        assert!(!first_report.has_dps_growth_rate);
        assert_eq!(first_report.dps_growth_rate_text(), "0");

        analyzer.process_line(&timeline_line(20, "ECLIPTICA - now in stage: Stage_Two"));
        for second in 21..31 {
            analyzer.process_line(&timeline_line(second, "Dealing 20 STRIKE damage"));
        }
        analyzer.process_line(&timeline_line(31, "ECLIPTICA - now in intermission"));
        let second = analyzer.snapshot_at(timeline_timestamp(31));
        let second_report = second.round_report.as_ref().unwrap();
        assert!(second_report.has_dps_growth_rate);
        assert_eq!(second_report.dps_growth_rate, 100.0);
        assert_eq!(second_report.dps_growth_rate_text(), "100.0");
    }

    #[test]
    fn visit_dps_history_includes_zero_seconds_and_survives_leave_until_next_entry() {
        let mut analyzer = Analyzer::default();
        analyzer.process_line(&line(0, "[Behaviour] Entering Room: Ecliptica"));
        analyzer.process_line(&line(1, "ECLIPTICA - now in intermission"));
        analyzer.process_line(&line(2, "ECLIPTICA - now in stage: Stage_Demo"));
        analyzer.process_line(&line(3, "Dealing 42 STRIKE damage"));

        let live = analyzer.snapshot_at(timestamp(5));
        assert_eq!(
            live.dps_history,
            vec![
                DpsHistoryPoint {
                    elapsed_seconds: 0,
                    dps: 0,
                    combat_round_epoch: 0,
                    estimated_step: None,
                },
                DpsHistoryPoint {
                    elapsed_seconds: 1,
                    dps: 0,
                    combat_round_epoch: 0,
                    estimated_step: None,
                },
                DpsHistoryPoint {
                    elapsed_seconds: 2,
                    dps: 0,
                    combat_round_epoch: 1,
                    estimated_step: None,
                },
                DpsHistoryPoint {
                    elapsed_seconds: 3,
                    dps: 42,
                    combat_round_epoch: 1,
                    estimated_step: None,
                },
                DpsHistoryPoint {
                    elapsed_seconds: 4,
                    dps: 0,
                    combat_round_epoch: 1,
                    estimated_step: None,
                },
            ]
        );

        analyzer.process_line(&line(6, "[Behaviour] OnLeftRoom"));
        let left = analyzer.snapshot_at(timestamp(7));
        assert!(!left.in_ecliptica);
        assert_eq!(left.dps_history.last().unwrap().elapsed_seconds, 6);

        analyzer.process_line(&line(8, "[Behaviour] Entering Room: Ecliptica"));
        let new_visit = analyzer.snapshot_at(timestamp(9));
        assert_eq!(
            new_visit.dps_history,
            vec![DpsHistoryPoint {
                elapsed_seconds: 0,
                dps: 0,
                combat_round_epoch: 0,
                estimated_step: None,
            }]
        );
    }

    #[test]
    fn dps_history_keeps_round_boundaries_and_estimated_step() {
        let mut analyzer = Analyzer::default();
        analyzer.process_line(&timeline_line(0, "[Behaviour] Entering Room: Ecliptica"));
        analyzer.process_line(&timeline_line(1, "ECLIPTICA - now in intermission"));
        analyzer.process_line(&timeline_stage(2, 0.0));
        analyzer.process_line(&timeline_line(12, "ECLIPTICA - now in intermission"));
        analyzer.process_line(&timeline_stage(602, 0.0604));
        analyzer.process_line(&timeline_line(612, "ECLIPTICA - now in intermission"));
        analyzer.process_line(&timeline_stage(1_202, 0.1228));

        let snapshot = analyzer.snapshot_at(timeline_timestamp(1_204));
        let current = snapshot
            .dps_history
            .iter()
            .filter(|point| point.combat_round_epoch == snapshot.combat_round_epoch)
            .collect::<Vec<_>>();
        assert!(!current.is_empty());
        assert!(snapshot.has_step_estimate);
        assert!(
            current
                .iter()
                .all(|point| { point.estimated_step == Some(snapshot.current_step) })
        );
    }

    #[test]
    fn ten_second_burst_waits_for_a_complete_window() {
        let mut analyzer = Analyzer::default();
        analyzer.process_line(&line(
            0,
            "[Behaviour] Entering Room: Ecliptica - Demo Playtest",
        ));
        analyzer.process_line(&line(1, "ECLIPTICA - now in intermission"));
        analyzer.process_line(&line(2, "ECLIPTICA - now in stage: Stage_Demo"));
        analyzer.process_line(&line(5, "Dealing 100 STRIKE damage"));

        assert_eq!(
            analyzer.snapshot_at(timestamp(14)).round_burst_10s_dps,
            None
        );
        assert_eq!(
            analyzer.snapshot_at(timestamp(15)).round_burst_10s_dps,
            Some(10.0)
        );
    }

    #[test]
    fn empty_lobby_marker_does_not_create_a_report() {
        let mut analyzer = Analyzer::default();
        analyzer.process_line(&line(
            0,
            "[Behaviour] Entering Room: Ecliptica - Demo Playtest",
        ));
        analyzer.process_line(&line(2, "ECLIPTICA - now in intermission"));
        assert!(analyzer.snapshot_at(i64::MAX).round_report.is_none());
    }

    #[test]
    fn incoming_only_round_still_archives_damage_taken() {
        let mut analyzer = Analyzer::default();
        analyzer.process_line(&line(
            0,
            "[Behaviour] Entering Room: Ecliptica - Demo Playtest",
        ));
        analyzer.process_line(&line(1, "ECLIPTICA - now in intermission"));
        analyzer.process_line(&line(2, "ECLIPTICA - now in stage: Stage_Demo"));
        analyzer.process_line(&line(5, "damage has been taken: 23, from source: Boss"));
        analyzer.process_line(&line(8, "ECLIPTICA - now in intermission"));

        let report = analyzer
            .snapshot_at(timestamp(9))
            .round_report
            .expect("incoming damage should produce a round report");
        assert_eq!(report.total_damage, 0);
        assert_eq!(report.effective_dps, 0.0);
        assert_eq!(report.burst_10s_dps, None);
        assert_eq!(report.damage_taken, 23);
    }

    #[test]
    fn joining_an_active_stage_waits_for_an_explicit_round_boundary() {
        let mut analyzer = Analyzer::default();
        analyzer.process_line(&line(
            0,
            "[Behaviour] Entering Room: Ecliptica - Active Instance",
        ));
        assert_eq!(
            analyzer.snapshot_at(timestamp(0)).phase,
            RoundPhase::Syncing
        );

        analyzer.process_line(&line(1, "ECLIPTICA - now in stage: Stage_Active"));
        let snapshot = analyzer.snapshot_at(i64::MAX);
        assert_eq!(snapshot.phase, RoundPhase::Combat);
        assert!(!snapshot.round_metrics_active);

        // Late-join activity is ignored until the next lobby.
        analyzer.process_line(&line(8, "Dealing 999 STRIKE damage"));
        analyzer.process_line(&line(9, "damage has been taken: 4, from source: Boss"));
        let snapshot = analyzer.snapshot_at(i64::MAX);
        assert!(!snapshot.round_metrics_active);
        assert!(!snapshot.has_damage_data);

        analyzer.process_line(&line(10, "ECLIPTICA - now in intermission"));
        let snapshot = analyzer.snapshot_at(i64::MAX);
        assert_eq!(snapshot.phase, RoundPhase::Lobby);
        assert!(!snapshot.round_metrics_active);
        assert!(snapshot.round_report.is_none());

        analyzer.process_line(&line(12, "ECLIPTICA - now in stage: Stage_Next"));
        analyzer.process_line(&line(15, "Dealing 30 STRIKE damage"));
        let base = Local
            .with_ymd_and_hms(2026, 8, 3, 1, 2, 0)
            .earliest()
            .unwrap()
            .timestamp();
        let snapshot = analyzer.snapshot_at(base + 16);
        assert!(snapshot.round_metrics_active);
        assert_eq!(snapshot.latest_dps, 30);
    }

    #[test]
    fn a_quiet_new_room_remains_syncing_without_an_explicit_phase_signal() {
        let mut analyzer = Analyzer::default();
        analyzer.process_line(&line(
            0,
            "[Behaviour] Entering Room: Ecliptica - Waiting Instance",
        ));
        assert_eq!(
            analyzer.snapshot_at(timestamp(11)).phase,
            RoundPhase::Syncing
        );

        let unknown = analyzer.snapshot_at(timeline_timestamp(120));
        assert_eq!(unknown.phase, RoundPhase::Syncing);
        assert!(!unknown.round_metrics_active);
        assert!(unknown.round_report.is_none());

        // A Stage alone identifies world combat, but cannot prove local
        // participation, so personal round metrics remain disabled.
        analyzer.process_line(&line(30, "ECLIPTICA - now in stage: Stage_First"));
        analyzer.process_line(&line(35, "Dealing 25 STRIKE damage"));
        let playing = analyzer.snapshot_at(timestamp(36));
        assert_eq!(playing.phase, RoundPhase::Combat);
        assert!(!playing.round_metrics_active);
        assert_eq!(playing.latest_dps, 0);
    }

    #[test]
    fn first_boss_exposes_world_combat_without_claiming_local_participation() {
        let mut analyzer = Analyzer::default();
        analyzer.process_line(&line(
            0,
            "[Behaviour] Entering Room: Ecliptica - Waiting Instance",
        ));
        assert_eq!(
            analyzer.snapshot_at(timestamp(12)).phase,
            RoundPhase::Syncing
        );

        analyzer.process_line(&line(
            13,
            "ECLIPTICA - now fighting boss: Maxipuss(Clone) on phase: 1",
        ));
        analyzer.process_line(&line(13, "ownership of Maxipuss transferred to Player One"));
        let combat = analyzer.snapshot_at(timestamp(13));
        assert_eq!(combat.phase, RoundPhase::Combat);
        assert_eq!(combat.boss.as_deref(), Some("Maxipuss"));
        assert_eq!(combat.boss_lock.as_deref(), Some("Player One"));
        assert!(!combat.round_metrics_active);
        assert!(combat.round_report.is_none());
    }

    #[test]
    fn explicit_lobby_still_ignores_training_boss_noise() {
        let mut analyzer = Analyzer::default();
        analyzer.process_line(&line(
            0,
            "[Behaviour] Entering Room: Ecliptica - Waiting Instance",
        ));
        analyzer.process_line(&line(1, "ECLIPTICA - now in lobby"));
        analyzer.process_line(&line(
            20,
            "ECLIPTICA - now fighting boss: Maxipuss(Clone) on phase: 1",
        ));
        analyzer.process_line(&line(20, "ownership of Maxipuss transferred to Player One"));

        let lobby = analyzer.snapshot_at(timestamp(20));
        assert_eq!(lobby.phase, RoundPhase::Lobby);
        assert!(lobby.boss.is_none());
        assert!(lobby.boss_lock.is_none());
    }

    #[test]
    fn missing_intermission_drops_incomplete_round_and_reports_degradation() {
        let mut analyzer = Analyzer::default();
        analyzer.process_line(&line(0, "[Behaviour] Entering Room: Ecliptica"));
        analyzer.process_line(&line(1, "ECLIPTICA - now in intermission"));
        analyzer.process_line(&line(2, "ECLIPTICA - now in stage: Stage_First"));
        analyzer.process_line(&line(5, "Dealing 40 STRIKE damage"));

        analyzer.process_line(&line(10, "ECLIPTICA - now in stage: Stage_Second"));
        let snapshot = analyzer.snapshot_at(timestamp(11));

        assert_eq!(snapshot.phase, RoundPhase::Combat);
        assert!(snapshot.round_report.is_none());
        assert!(!snapshot.has_damage_data);
        assert!(snapshot.data_quality.degraded);
        assert!(
            snapshot
                .data_quality
                .issues
                .iter()
                .any(|issue| issue.code == "intermission_missing")
        );
    }

    #[test]
    fn joining_in_lobby_starts_alive_and_room_changes_clear_everything() {
        let mut analyzer = Analyzer::default();
        analyzer.process_line(&line(
            0,
            "[Behaviour] Entering Room: Ecliptica - First Instance",
        ));
        analyzer.process_line(&line(1, "ECLIPTICA - now in intermission"));
        analyzer.process_line(&line(2, "ECLIPTICA - now in stage: Stage_First"));
        analyzer.process_line(&line(5, "Dealing 40 STRIKE damage"));
        analyzer.process_line(&line(
            6,
            "ECLIPTICA - now fighting boss: Maxipuss(Clone) on phase: 1",
        ));
        analyzer.process_line(&line(7, "ownership of Maxipuss transferred to Old Player"));
        analyzer.process_line(&line(10, "ECLIPTICA - now in intermission"));
        let old = analyzer.snapshot_at(i64::MAX);
        assert!(old.round_report.is_some());
        assert_eq!(old.max_dps, 40);

        analyzer.process_line(&line(15, "[Behaviour] OnLeftRoom"));
        let outside = analyzer.snapshot_at(i64::MAX);
        assert_eq!(outside.phase, RoundPhase::Outside);
        assert!(!outside.in_ecliptica);
        assert_eq!(outside.max_dps, 0);
        assert!(outside.round_report.is_none());
        assert!(outside.boss_lock.is_none());

        analyzer.process_line(&line(
            20,
            "[Behaviour] Entering Room: Ecliptica - New Instance",
        ));
        let syncing = analyzer.snapshot_at(timestamp(20));
        assert_eq!(syncing.phase, RoundPhase::Syncing);
        assert_eq!(syncing.max_dps, 0);
        assert!(syncing.round_report.is_none());

        // World combat is visible, but personal participation is not inferred.
        analyzer.process_line(&line(21, "ECLIPTICA - now in stage: Stage_AlreadyActive"));
        let new_room = analyzer.snapshot_at(i64::MAX);
        assert!(!new_room.round_metrics_active);
        assert_eq!(new_room.max_dps, 0);
    }

    #[test]
    fn a_new_ecliptica_entry_clears_active_state_even_without_leave_event() {
        let mut analyzer = Analyzer::default();
        analyzer.process_line(&line(0, "ECLIPTICA - now in intermission"));
        analyzer.process_line(&line(1, "ECLIPTICA - now in stage: Stage_Old"));
        analyzer.process_line(&line(5, "Dealing 75 STRIKE damage"));
        analyzer.process_line(&line(
            6,
            "ECLIPTICA - now fighting boss: Maxipuss(Clone) on phase: 1",
        ));
        analyzer.process_line(&line(7, "ownership of Maxipuss transferred to Old Player"));
        let old_room = analyzer.snapshot_at(i64::MAX);
        assert_eq!(old_room.boss_lock.as_deref(), Some("Old Player"));
        assert!(old_room.has_damage_data);

        analyzer.process_line(&line(
            20,
            "[Behaviour] Entering Room: Ecliptica - Replacement Instance",
        ));
        let replacement = analyzer.snapshot_at(timestamp(20));
        assert_eq!(replacement.phase, RoundPhase::Syncing);
        assert!(replacement.in_ecliptica);
        assert!(!replacement.round_metrics_active);
        assert!(!replacement.has_damage_data);
        assert_eq!(replacement.max_dps, 0);
        assert!(replacement.round_report.is_none());
        assert!(replacement.boss.is_none());
        assert!(replacement.boss_lock.is_none());
        assert!(!replacement.boss_active);
    }

    #[test]
    fn max_dps_keeps_the_visit_peak_across_rounds_and_resets_on_leave() {
        let mut analyzer = Analyzer::default();
        analyzer.process_line(&line(
            0,
            "[Behaviour] Entering Room: Ecliptica - Demo Playtest",
        ));
        analyzer.process_line(&line(1, "ECLIPTICA - now in intermission"));
        analyzer.process_line(&line(2, "ECLIPTICA - now in stage: Stage_Demo"));
        analyzer.process_line(&line(5, "Dealing 30 STRIKE damage"));
        analyzer.process_line(&line(5, "Dealing 12 NON-STRIKE damage"));
        let base = Local
            .with_ymd_and_hms(2026, 8, 3, 1, 2, 0)
            .earliest()
            .unwrap()
            .timestamp();
        // Simulate startup recovery: the whole round is parsed without an
        // intermediate snapshot before the intermission line arrives.
        analyzer.process_line(&line(7, "ECLIPTICA - now in intermission"));
        assert_eq!(analyzer.snapshot_at(base + 8).max_dps_text(), "42");
        analyzer.process_line(&line(9, "ECLIPTICA - now in stage: 2"));
        analyzer.process_line(&line(10, "Dealing 60 STRIKE damage"));
        assert_eq!(analyzer.snapshot_at(base + 11).max_dps, 60);

        analyzer.process_line(&line(12, "ECLIPTICA - now in intermission"));
        analyzer.process_line(&line(14, "ECLIPTICA - now in stage: 3"));
        analyzer.process_line(&line(15, "Dealing 25 STRIKE damage"));
        analyzer.process_line(&line(17, "ECLIPTICA - now in intermission"));
        let snapshot = analyzer.snapshot_at(base + 18);
        assert_eq!(snapshot.max_dps, 60);
        assert_eq!(snapshot.round_report.unwrap().max_dps, 25);

        analyzer.process_line(&line(20, "[Behaviour] OnLeftRoom"));
        assert_eq!(analyzer.snapshot_at(base + 21).max_dps_text(), "-");
    }

    #[test]
    fn only_current_boss_ownership_updates_lock_and_intermission_clears_it() {
        let mut analyzer = Analyzer::default();
        analyzer.process_line(&line(
            0,
            "ECLIPTICA - now fighting boss: Maxipuss(Clone) on phase: 0.1",
        ));
        analyzer.process_line(&line(1, "ownership of Fly transferred to Nobody"));
        assert_eq!(analyzer.snapshot_at(i64::MAX).boss_lock, None);
        analyzer.process_line(&line(2, "ownership of Maxipuss transferred to Player One"));
        assert_eq!(
            analyzer.snapshot_at(i64::MAX).boss_lock.as_deref(),
            Some("Player One")
        );
        analyzer.process_line(&line(3, "ECLIPTICA - now in intermission"));
        assert!(!analyzer.snapshot_at(i64::MAX).boss_active);
    }

    #[test]
    fn names_use_unicode_nfkc_casefold_like_comparison() {
        assert_eq!(normalized_name("  Ａlice "), normalized_name("alice"));
    }

    #[test]
    fn boss_death_and_lobby_clear_lock_and_statistics() {
        let mut analyzer = Analyzer::default();
        analyzer.process_line(&line(
            0,
            "ECLIPTICA - now fighting boss: JimBringerPhase3(Clone) on phase: 1",
        ));
        analyzer.process_line(&line(
            1,
            "ownership of JimBringerPhase3 transferred to Alice",
        ));
        analyzer.process_line(&line(
            2,
            "Boss JimBringerPhase3 dead, personal damage dealt:",
        ));
        assert_eq!(analyzer.snapshot_at(i64::MAX).boss_lock, None);
        analyzer.process_line(&line(3, "ECLIPTICA - now in lobby"));
        let snapshot = analyzer.snapshot_at(i64::MAX);
        assert_eq!(snapshot.latest_dps, 0);
        assert!(!snapshot.boss_active);
    }
}
