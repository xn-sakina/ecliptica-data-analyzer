//! Bayesian model averaging over historical and updated game pacing.
//! Offline priors are reproducibly fitted by scripts/train-step-model.py.
//! Runtime learning uses only transitions already observed in this room.

use serde::Deserialize;
use std::sync::OnceLock;

const TRAINED_DIM: usize = 6;
// A shared room offset lets observations calibrate overall progression before
// every individual phase band has enough samples. It has a zero-mean prior.
const DIM: usize = TRAINED_DIM + 1;
const MAX_TRANSITIONS: u32 = 64;
const RIDGE: [f64; DIM] = [5.0, 5.0, 5.0, 5.0, 5.0, 1.0, 16.0];

#[derive(Clone, Copy)]
pub(super) struct ProgressSample {
    pub phase: f64,
    pub delta: f64,
    pub combat_seconds: Option<f64>,
}

#[derive(Clone, Deserialize)]
struct Prior {
    coefficients: [f64; TRAINED_DIM],
    combat_seconds_range: [f64; 2],
}
#[derive(Deserialize)]
struct PriorFile {
    models: Vec<Prior>,
}

fn priors() -> &'static [Prior] {
    static PRIORS: OnceLock<Vec<Prior>> = OnceLock::new();
    PRIORS.get_or_init(|| {
        serde_json::from_str::<PriorFile>(include_str!("../../resources/rules/step-priors.json"))
            .map(|file| file.models)
            .unwrap_or_default()
    })
}

fn features(phase: f64, combat: f64) -> [f64; DIM] {
    let mut x = [0.0; DIM];
    x[((phase.clamp(0.0, 1.0) * 5.0) as usize).min(4)] = 1.0;
    x[5] = (combat.clamp(30.0, 1800.0) / 420.0).ln();
    x[6] = 1.0;
    x
}

fn predict(coefficients: &[f64], phase: f64, combat: f64) -> f64 {
    features(phase, combat)
        .iter()
        .zip(coefficients)
        .map(|(x, b)| x * b)
        .sum::<f64>()
        .clamp(0.015, 0.16)
}

fn solve(mut a: [[f64; DIM]; DIM], mut b: [f64; DIM]) -> Option<[f64; DIM]> {
    for col in 0..DIM {
        let pivot = (col..DIM).max_by(|&i, &j| a[i][col].abs().total_cmp(&a[j][col].abs()))?;
        a.swap(pivot, col);
        b.swap(pivot, col);
        let divisor = a[col][col];
        if !divisor.is_finite() || divisor.abs() < 1e-12 {
            return None;
        }
        for k in col..DIM {
            a[col][k] /= divisor;
        }
        b[col] /= divisor;
        for row in 0..DIM {
            if row == col {
                continue;
            }
            let factor = a[row][col];
            for k in col..DIM {
                a[row][k] -= factor * a[col][k];
            }
            b[row] -= factor * b[col];
        }
    }
    // The game rule is monotonic in combat duration. Sparse/noisy room data
    // must not learn that waiting longer increases progression.
    b[5] = b[5].clamp(-0.2, 0.0);
    b.iter().all(|v| v.is_finite()).then_some(b)
}

struct Posterior {
    coefficients: [f64; DIM],
    precision: [[f64; DIM]; DIM],
    information: [f64; DIM],
    log_evidence: f64,
    prior_time_coefficient: f64,
    time_prior_precision: f64,
    combat_seconds_range: [f64; 2],
}

impl Posterior {
    fn new(prior: &Prior) -> Self {
        let mut precision = [[0.0; DIM]; DIM];
        let mut information = [0.0; DIM];
        let mut coefficients = [0.0; DIM];
        coefficients[..TRAINED_DIM].copy_from_slice(&prior.coefficients);
        for i in 0..DIM {
            precision[i][i] = RIDGE[i];
            information[i] = RIDGE[i] * coefficients[i];
        }
        Self {
            coefficients,
            precision,
            information,
            log_evidence: 0.0,
            prior_time_coefficient: prior.coefficients[5],
            time_prior_precision: RIDGE[5],
            combat_seconds_range: prior.combat_seconds_range,
        }
    }

    fn observe(&mut self, sample: ProgressSample, inferred_duration: f64) {
        let combat = sample.combat_seconds.unwrap_or(inferred_duration);
        let residual = sample.delta - predict(&self.coefficients, sample.phase, combat);
        // Score BEFORE updating. A flexible model cannot win by evaluating
        // an observation it has already fitted. Student-t evidence tolerates
        // an isolated special/slow stage without permanently locking a regime.
        let sigma: f64 = if sample.combat_seconds.is_some() {
            0.015
        } else {
            0.03
        };
        self.log_evidence =
            0.95 * self.log_evidence - 2.0 * (1.0 + residual.powi(2) / (3.0 * sigma.powi(2))).ln();
        // Repeated fights outside this prior's measured pace range provide
        // evidence that its time response needs room-specific recalibration.
        // Temper only the prior, preserving all accumulated observations and
        // phase-band precision. In-range play keeps the established behavior.
        if let Some(seconds) = sample.combat_seconds {
            let [minimum, maximum] = self.combat_seconds_range;
            if seconds < minimum || seconds > maximum {
                let next_precision = (self.time_prior_precision * 0.8).max(0.25);
                let reduction = self.time_prior_precision - next_precision;
                self.precision[5][5] -= reduction;
                self.information[5] -= reduction * self.prior_time_coefficient;
                self.time_prior_precision = next_precision;
            }
        }
        let x = features(sample.phase, combat);
        let reliability = if sample.combat_seconds.is_some() {
            1.0
        } else {
            0.25
        };
        let weight = reliability / (1.0 + (residual / 0.03).powi(2));
        for row in 0..DIM {
            self.information[row] += weight * x[row] * sample.delta;
            for col in 0..DIM {
                self.precision[row][col] += weight * x[row] * x[col];
            }
        }
        if let Some(coefficients) = solve(self.precision, self.information) {
            self.coefficients = coefficients;
        }
    }
}

pub(super) struct ProgressEnsemble {
    models: Vec<Posterior>,
    recent_combat: Vec<f64>,
}

impl ProgressEnsemble {
    pub fn fit(samples: &[ProgressSample]) -> Option<Self> {
        Self::fit_with_priors(samples, priors())
    }

    fn fit_with_priors(samples: &[ProgressSample], model_priors: &[Prior]) -> Option<Self> {
        let mut known: Vec<f64> = Vec::new();
        let mut models: Vec<_> = model_priors.iter().map(Posterior::new).collect();
        if models.is_empty() {
            return None;
        }
        for sample in samples {
            let inferred = median(&known).unwrap_or(420.0);
            for model in &mut models {
                model.observe(*sample, inferred);
            }
            if let Some(seconds) = sample.combat_seconds {
                known.push(seconds);
            }
        }
        if known.is_empty() {
            return None;
        } // Keep the established phase-only fallback.
        Some(Self {
            models,
            recent_combat: known
                .into_iter()
                .rev()
                .take(5)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect(),
        })
    }

    fn pace_scenarios(&self, completed_combat: Option<f64>) -> [f64; 3] {
        let mut recent = self.recent_combat.clone();
        if let Some(seconds) = completed_combat {
            recent.push(seconds);
        }
        let mut sorted = recent.clone();
        sorted.sort_by(f64::total_cmp);
        // Predict pace from a recency-weighted geometric mean: one old long
        // fight must not freeze every subsequent forecast at that duration.
        let mut weighted_log = 0.0;
        let mut total_weight = 0.0;
        for (age, seconds) in recent.iter().rev().take(5).enumerate() {
            let weight = 0.5_f64.powi(age as i32);
            weighted_log += weight * seconds.ln();
            total_weight += weight;
        }
        let central = (weighted_log / total_weight).exp();
        [
            sorted[(sorted.len() - 1) / 4].min(central),
            central,
            sorted[(sorted.len() - 1) * 3 / 4].max(central),
        ]
    }

    pub fn remaining(&self, phase: f64, completed_combat: Option<f64>, elapsed: f64) -> u32 {
        let max_score = self
            .models
            .iter()
            .map(|m| m.log_evidence)
            .fold(f64::NEG_INFINITY, f64::max);
        let mut outcomes = Vec::new();
        for model in &self.models {
            let model_weight = (model.log_evidence - max_score).exp().max(0.0001);
            for (duration, pace_weight) in self
                .pace_scenarios(completed_combat)
                .into_iter()
                .zip([0.2, 0.6, 0.2])
            {
                let mut projected = phase;
                let mut transitions = 0;
                while projected < 1.0 && transitions < MAX_TRANSITIONS {
                    let combat = if transitions == 0 {
                        completed_combat.unwrap_or(duration.max(elapsed))
                    } else {
                        duration
                    };
                    projected =
                        (projected + predict(&model.coefficients, projected, combat)).min(1.0);
                    transitions += 1;
                }
                outcomes.push((transitions.saturating_sub(1), model_weight * pace_weight));
            }
        }
        weighted_median(&mut outcomes)
    }
}

fn median(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    Some(sorted[sorted.len() / 2])
}
fn weighted_median(values: &mut [(u32, f64)]) -> u32 {
    values.sort_by_key(|v| v.0);
    let half = values.iter().map(|v| v.1).sum::<f64>() / 2.0;
    let mut sum = 0.0;
    for (value, weight) in values.iter() {
        sum += weight;
        if sum > half {
            return *value;
        }
    }
    values.last().map_or(0, |v| v.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shipped_priors_are_valid_and_longer_fights_never_increase_progress() {
        assert_eq!(priors().len(), 2);
        for prior in priors() {
            for phase in [0.0, 0.3, 0.5, 0.7, 0.9] {
                assert!(
                    predict(&prior.coefficients, phase, 900.0)
                        <= predict(&prior.coefficients, phase, 300.0)
                );
            }
        }
    }
    #[test]
    fn posterior_scores_unseen_samples_and_can_select_the_updated_regime() {
        let prior = &priors()[1];
        let samples: Vec<_> = [
            (0.4, 420.0),
            (0.55, 600.0),
            (0.65, 600.0),
            (0.75, 420.0),
            (0.85, 600.0),
        ]
        .into_iter()
        .map(|(phase, t)| ProgressSample {
            phase,
            delta: predict(&prior.coefficients, phase, t),
            combat_seconds: Some(t),
        })
        .collect();
        let ensemble = ProgressEnsemble::fit(&samples).unwrap();
        assert!(ensemble.models[1].log_evidence > ensemble.models[0].log_evidence);
        assert!(
            ensemble.remaining(0.7, Some(900.0), 0.0) >= ensemble.remaining(0.7, Some(300.0), 0.0)
        );
    }
    #[test]
    fn no_complete_timing_uses_the_original_fallback() {
        let sample = ProgressSample {
            phase: 0.5,
            delta: 0.1,
            combat_seconds: None,
        };
        assert!(ProgressEnsemble::fit(&[sample, sample]).is_none());
    }
    #[test]
    fn completed_and_live_long_fights_change_forecasts_without_requiring_more_samples() {
        let prior = &priors()[1];
        let samples: Vec<_> = [(0.3, 420.0), (0.4, 600.0), (0.55, 300.0)]
            .into_iter()
            .map(|(phase, t)| ProgressSample {
                phase,
                delta: predict(&prior.coefficients, phase, t),
                combat_seconds: Some(t),
            })
            .collect();
        let ensemble = ProgressEnsemble::fit(&samples).unwrap();
        let mut completed_changes = 0;
        let mut live_changes = 0;
        for phase in [0.3, 0.45, 0.6, 0.7, 0.8, 0.9] {
            let short = ensemble.remaining(phase, Some(300.0), 0.0);
            let long = ensemble.remaining(phase, Some(900.0), 0.0);
            assert!(long >= short);
            completed_changes += usize::from(long > short);
            let early = ensemble.remaining(phase, None, 100.0);
            let ongoing = ensemble.remaining(phase, None, 900.0);
            assert!(ongoing >= early);
            live_changes += usize::from(ongoing > early);
        }
        assert!(completed_changes > 0);
        assert!(live_changes > 0);
    }

    #[test]
    fn repeated_unseen_combat_pace_tempers_only_the_time_prior() {
        let prior = &priors()[1];
        for (combat, expected_precision) in [(480.0, 1.0), (780.0, 0.25)] {
            let mut posterior = Posterior::new(prior);
            for phase in [0.0, 0.06, 0.12, 0.18, 0.25, 0.33, 0.42, 0.51] {
                posterior.observe(
                    ProgressSample {
                        phase,
                        delta: 0.06,
                        combat_seconds: Some(combat),
                    },
                    combat,
                );
            }
            assert_eq!(posterior.time_prior_precision, expected_precision);
            for band in 0..5 {
                assert!(posterior.precision[band][band] >= RIDGE[band]);
            }
            assert!(
                posterior.precision[5][5] > expected_precision,
                "tempering must preserve the precision contributed by real observations"
            );
        }
    }

    #[test]
    fn shared_room_calibration_learns_both_directions_without_rewriting_unseen_band_priors() {
        let prior = &priors()[1];
        for direction in [-1.0, 1.0] {
            let mut posterior = Posterior::new(prior);
            assert_eq!(posterior.coefficients[6], 0.0);
            for phase in [0.25, 0.45, 0.65, 0.85] {
                posterior.observe(
                    ProgressSample {
                        phase,
                        delta: predict(&prior.coefficients, phase, 420.0) + direction * 0.01,
                        combat_seconds: Some(420.0),
                    },
                    420.0,
                );
            }
            assert!(direction * posterior.coefficients[6] > 0.0);
            assert_eq!(posterior.coefficients[0], prior.coefficients[0]);
            let correction = predict(&posterior.coefficients, 0.1, 420.0)
                - predict(&prior.coefficients, 0.1, 420.0);
            assert!(direction * correction > 0.0 && correction.abs() < 0.01);
            assert!(posterior.coefficients[5] <= 0.0);
        }
    }

    #[derive(Deserialize)]
    struct Corpus {
        runs: Vec<Run>,
    }
    #[derive(Deserialize)]
    struct Run {
        id: String,
        reaches_final: bool,
        stages: Vec<Stage>,
    }
    #[derive(Deserialize)]
    struct Stage {
        phase: f64,
        second: i64,
        end: Option<i64>,
    }
    #[derive(Deserialize)]
    struct Validation {
        folds: Vec<Fold>,
    }
    #[derive(Deserialize)]
    struct Fold {
        run_id: String,
        models: Vec<Prior>,
    }

    #[test]
    fn entire_run_holdout_compares_stage_and_report_forecasts_without_leakage() {
        let corpus: Corpus =
            serde_json::from_str(include_str!("../../resources/rules/step-training.json")).unwrap();
        let validation: Validation =
            serde_json::from_str(include_str!("../../resources/rules/step-validation.json"))
                .unwrap();
        let mut count = 0;
        let mut original_error = 0;
        let mut stage_error = 0;
        let mut report_error = 0;
        let mut original_early = 0;
        let mut new_early = 0;
        for run in corpus.runs.iter().filter(|run| run.reaches_final) {
            let fold = validation
                .folds
                .iter()
                .find(|fold| fold.run_id == run.id)
                .unwrap();
            for join in 0..run.stages.len() - 2 {
                for index in join + 2..run.stages.len() - 1 {
                    let mut samples = Vec::new();
                    let mut cycles = Vec::new();
                    for pair in run.stages[join..=index].windows(2) {
                        let delta = pair[1].phase - pair[0].phase;
                        let cycle = pair[1].second - pair[0].second;
                        if (0.025..=0.16).contains(&delta) && (180..=1800).contains(&cycle) {
                            samples.push(ProgressSample {
                                phase: pair[0].phase,
                                delta,
                                combat_seconds: pair[0]
                                    .end
                                    .map(|end| (end - pair[0].second) as f64),
                            });
                            cycles.push(cycle as f64);
                        }
                    }
                    if samples.len() < 2 {
                        continue;
                    }
                    let mean = samples
                        .iter()
                        .map(|s| s.delta - 0.05 * s.phase)
                        .sum::<f64>()
                        / samples.len() as f64;
                    let spread = (samples
                        .iter()
                        .map(|s| (s.delta - 0.05 * s.phase - mean).powi(2))
                        .sum::<f64>()
                        / samples.len() as f64)
                        .sqrt();
                    if spread > 0.035 {
                        continue;
                    }
                    let mean_cycle = cycles.iter().sum::<f64>() / cycles.len() as f64;
                    let prior = super::super::intercept_for_transitions(
                        (6900.0 / mean_cycle).round().clamp(8.0, 16.0) as u32,
                    );
                    let weight = super::super::step_time_prior_weight(mean, prior);
                    let old_intercept =
                        (mean * (1.0 - weight) + prior * weight).clamp(0.035, 0.095);
                    let stage = &run.stages[index];
                    let expected = (run.stages.len() - index - 2) as u32;
                    let baseline =
                        super::super::transitions_until_final(stage.phase, old_intercept)
                            .saturating_sub(1);
                    let Some(ensemble) = ProgressEnsemble::fit_with_priors(&samples, &fold.models)
                    else {
                        continue;
                    };
                    let at_stage = ensemble.remaining(stage.phase, None, 0.0);
                    let at_report = ensemble.remaining(
                        stage.phase,
                        stage.end.map(|end| (end - stage.second) as f64),
                        0.0,
                    );
                    count += 1;
                    original_error += baseline.abs_diff(expected);
                    stage_error += at_stage.abs_diff(expected);
                    report_error += at_report.abs_diff(expected);
                    original_early += usize::from(baseline == 0 && expected > 0);
                    new_early += usize::from(at_report == 0 && expected > 0);
                }
            }
        }
        eprintln!(
            "Held-out runs: n={count}, old_abs_error={original_error}, new_stage_abs_error={stage_error}, new_report_abs_error={report_error}, old_early_final={original_early}, new_early_final={new_early}"
        );
        assert!(count > 100);
        assert!(
            stage_error <= original_error,
            "the held-out stage forecast must improve the original baseline"
        );
        assert!(
            report_error <= original_error,
            "the held-out report forecast must improve the original baseline"
        );
        assert!(
            new_early <= original_early,
            "do not increase premature final-Boss announcements"
        );
    }
}
