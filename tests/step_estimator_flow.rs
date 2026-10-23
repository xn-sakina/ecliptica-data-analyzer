use ecliptica_data_analyzer::{
    analysis::Analyzer,
    log_protocol::{LogParser, ParsedEvent},
};

use chrono::{Duration, Local, NaiveDate, TimeZone};

fn feed_synthetic(
    analyzer: &mut Analyzer,
    elapsed: i64,
    message: &str,
) -> ecliptica_data_analyzer::analysis::GameSnapshot {
    let time = NaiveDate::from_ymd_opt(2026, 10, 23)
        .unwrap()
        .and_hms_opt(0, 0, 0)
        .unwrap()
        + Duration::seconds(elapsed);
    let line = format!(
        "{} Debug      -  {message}",
        time.format("%Y.%m.%d %H:%M:%S")
    );
    analyzer.process_line(&line);
    analyzer.snapshot_at(
        Local
            .from_local_datetime(&time)
            .earliest()
            .unwrap()
            .timestamp(),
    )
}

const ROUTES: [&str; 2] = [
    include_str!("fixtures/october_steps_a.txt"),
    include_str!("fixtures/october_steps_b.txt"),
];

#[test]
fn october_routes_keep_the_extra_late_stage_and_final_boundary() {
    let parser = LogParser::default();
    for (route, fixture) in ROUTES.iter().enumerate() {
        let mut analyzer = Analyzer::default();
        for line in fixture.lines() {
            analyzer.process_line(line);
            if let Some(ParsedEvent::Stage {
                second,
                phase: Some(phase),
            }) = parser.parse(line).event
            {
                let snapshot = analyzer.snapshot_at(second);
                if snapshot.has_step_estimate {
                    eprintln!(
                        "route={route} phase={phase:.7} current={} remaining={}",
                        snapshot.current_step, snapshot.until_boss_step
                    );
                    assert!(snapshot.current_step >= 3);
                }
                let expected = match (route, phase) {
                    (0, p) if (p - 0.8528067).abs() < 1e-7 => Some(1),
                    (0, p) if (p - 0.9279246).abs() < 1e-7 => Some(0),
                    (1, p) if (p - 0.9054043).abs() < 1e-7 => Some(1),
                    (1, p) if (p - 0.9503832).abs() < 1e-7 => Some(0),
                    _ => None,
                };
                if let Some(expected) = expected {
                    assert!(snapshot.has_step_estimate, "route {route} at {phase}");
                    assert_eq!(
                        snapshot.until_boss_step, expected,
                        "route {route} at {phase}"
                    );
                }
            }
        }
    }
}

#[test]
fn october_replay_records_coverage_and_suffix_error_for_late_joins() {
    let parser = LogParser::default();
    let mut eligible = 0;
    let mut shown = 0;
    let mut exact = 0;
    let mut within_one = 0;
    for fixture in ROUTES {
        let lines: Vec<_> = fixture.lines().collect();
        let starts: Vec<_> = lines
            .iter()
            .enumerate()
            .filter_map(|(i, line)| {
                matches!(parser.parse(line).event, Some(ParsedEvent::Stage { .. })).then_some(i)
            })
            .collect();
        for join in 0..starts.len() - 2 {
            let mut analyzer = Analyzer::default();
            let mut ordinal = join;
            for line in &lines[starts[join]..] {
                analyzer.process_line(line);
                if let Some(ParsedEvent::Stage {
                    second,
                    phase: Some(phase),
                }) = parser.parse(line).event
                {
                    let snapshot = analyzer.snapshot_at(second);
                    let expected = starts.len().saturating_sub(ordinal + 2) as u32;
                    if phase < 1.0 && ordinal >= join + 2 {
                        eligible += 1;
                        if snapshot.has_step_estimate {
                            shown += 1;
                            let error = snapshot.until_boss_step.abs_diff(expected);
                            if expected > 0 {
                                assert!(
                                    snapshot.until_boss_step > 0,
                                    "must not announce the final Boss early at phase {phase}"
                                );
                            }
                            exact += usize::from(error == 0);
                            within_one += usize::from(error <= 1);
                        }
                    }
                    ordinal += 1;
                }
            }
        }
    }
    eprintln!(
        "October replay: shown={shown}/{eligible}, exact={exact}/{shown}, within_one={within_one}/{shown}"
    );
    assert_eq!(
        shown, eligible,
        "keep the original display timing for these routes"
    );
    assert!(
        exact * 100 >= shown * 50,
        "shown predictions must retain the audited accuracy"
    );
    assert!(
        within_one * 100 >= shown * 80,
        "at least 80% of shown suffix predictions must be within one round"
    );
}

#[test]
fn historical_near_final_boundary_is_preserved_with_real_combat_durations() {
    let parser = LogParser::default();
    let mut analyzer = Analyzer::default();
    let mut phase = 0.0_f64;
    let mut checked = 0;
    for line in include_str!("fixtures/september_steps.txt").lines() {
        analyzer.process_line(line);
        let event = parser.parse(line).event;
        if let Some(ParsedEvent::Stage {
            phase: Some(value), ..
        }) = event
        {
            phase = value;
        }
        let second = match event {
            Some(ParsedEvent::Stage { second, .. })
            | Some(ParsedEvent::Intermission { second }) => second,
            _ => continue,
        };
        let expected = if (phase - 0.887341).abs() < 1e-7 {
            Some((12, 1))
        } else if (phase - 0.9994387).abs() < 1e-7 {
            Some((13, 0))
        } else {
            None
        };
        if let Some((current, remaining)) = expected {
            let snapshot = analyzer.snapshot_at(second);
            assert!(snapshot.has_step_estimate);
            assert_eq!(
                (snapshot.current_step, snapshot.until_boss_step),
                (current, remaining)
            );
            checked += 1;
        }
    }
    assert_eq!(checked, 4);
}

#[test]
fn four_hour_sixteen_stage_runs_keep_early_estimates_and_the_final_boundary() {
    // Independent hypothetical observations, not new training data or a claim
    // that real games always finish in sixteen stages after four hours.
    let phases = [
        0.0, 0.052, 0.109, 0.164, 0.219, 0.299, 0.378, 0.457, 0.535, 0.611, 0.677, 0.742, 0.807,
        0.872, 0.938, 1.0,
    ];
    // Both total four hours: distinguish longer normal fights from shop/final
    // Boss time instead of turning total elapsed duration into a round count.
    for (combat, shop, final_combat) in [(540, 300, 1800), (780, 120, 900)] {
        let mut analyzer = Analyzer::default();
        feed_synthetic(
            &mut analyzer,
            0,
            "[Behaviour] Entering Room: Ecliptica - Fixture",
        );
        for (index, phase) in phases.into_iter().enumerate() {
            let start = 60 + index as i64 * (combat + shop);
            let stage = if phase == 1.0 {
                "Stage_Gilgamesh"
            } else {
                "Stage_Hypothetical"
            };
            let snapshot = feed_synthetic(
                &mut analyzer,
                start,
                &format!("ECLIPTICA - now in stage: {stage} on phase: {phase} as class: Twinmage"),
            );
            assert_eq!(
                snapshot.has_step_estimate,
                index >= 2,
                "visibility changed in round {} with {combat}s combat",
                index + 1
            );
            if snapshot.has_step_estimate {
                assert_eq!(snapshot.current_step, index as u32 + 1);
            }
            feed_synthetic(&mut analyzer, start + 1, "Dealing 100 STRIKE damage");
            if phase < 1.0 {
                let report = feed_synthetic(
                    &mut analyzer,
                    start + combat,
                    "ECLIPTICA - now in intermission",
                );
                if index == 13 || index == 14 {
                    eprintln!(
                        "4h/16 stages: combat={combat}, round={}, remaining={}",
                        index + 1,
                        report.until_boss_step
                    );
                    assert!(report.has_step_estimate);
                    assert_eq!(
                        report.until_boss_step,
                        (14 - index) as u32,
                        "final boundary with {combat}s combat"
                    );
                }
            } else {
                feed_synthetic(
                    &mut analyzer,
                    start + 2,
                    "ECLIPTICA - now fighting boss: GilgameshPhase3(Clone) on phase: 1",
                );
                let completed = feed_synthetic(
                    &mut analyzer,
                    start + final_combat,
                    "Boss GilgameshPhase3 dead, personal damage dealt:",
                );
                assert_eq!(start + final_combat - 60, 4 * 60 * 60);
                assert!(!completed.has_step_estimate);
                assert_eq!(
                    completed.round_report.unwrap().duration_seconds,
                    final_combat as u64
                );
            }
        }
    }
}

fn replay_hypothetical_route(phases: &[f64], combat: i64, shop: i64, exact_boundary: bool) {
    let mut analyzer = Analyzer::default();
    feed_synthetic(
        &mut analyzer,
        0,
        "[Behaviour] Entering Room: Ecliptica - Fixture",
    );
    for (index, &phase) in phases.iter().enumerate() {
        let start = 60 + index as i64 * (combat + shop);
        let snapshot = feed_synthetic(
            &mut analyzer,
            start,
            &format!(
                "ECLIPTICA - now in stage: Stage_Hypothetical on phase: {phase} as class: Twinmage"
            ),
        );
        assert_eq!(
            snapshot.has_step_estimate,
            index >= 2,
            "{} stages, stage {} at {combat}s combat",
            phases.len(),
            index + 1
        );
        if snapshot.has_step_estimate {
            assert_eq!(snapshot.current_step, index as u32 + 1);
        }
        feed_synthetic(&mut analyzer, start + 1, "Dealing 100 STRIKE damage");
        if phase < 1.0 {
            let snapshot = feed_synthetic(
                &mut analyzer,
                start + combat,
                "ECLIPTICA - now in intermission",
            );
            let expected = phases.len() - index - 2;
            if index >= 2 {
                assert!(snapshot.has_step_estimate);
                assert_eq!(snapshot.current_step, index as u32 + 1);
            }
            if index >= phases.len() - 3 {
                eprintln!(
                    "{} stages, combat={combat}, round={}, remaining={} (actual={expected})",
                    phases.len(),
                    index + 1,
                    snapshot.until_boss_step
                );
                if exact_boundary {
                    assert_eq!(snapshot.until_boss_step, expected as u32);
                }
            }
            // A long run must remain possible after the sixteenth round.
            if index == 15 && phases.len() > 17 {
                assert!(snapshot.until_boss_step > 0);
            }
        } else {
            assert_eq!(snapshot.current_step, phases.len() as u32);
            feed_synthetic(
                &mut analyzer,
                start + 2,
                "ECLIPTICA - now fighting boss: GilgameshPhase3(Clone) on phase: 1",
            );
            let ended = feed_synthetic(
                &mut analyzer,
                start + 900,
                "Boss GilgameshPhase3 dead, personal damage dealt:",
            );
            assert!(!ended.has_step_estimate);
            assert!(ended.round_report.is_some());
        }
    }
}

#[test]
fn fast_eleven_stage_runs_keep_the_original_display_timing_and_final_boundary() {
    // Hypothetical fast progression; not a claim about the game's minimum.
    let phases = [
        0.0, 0.078, 0.158, 0.242, 0.340, 0.442, 0.550, 0.662, 0.776, 0.892, 1.0,
    ];
    for combat in [180, 300] {
        replay_hypothetical_route(&phases, combat, 60, true);
    }
}

#[test]
fn slow_eighteen_stage_runs_do_not_treat_sixteen_as_the_limit() {
    // Extend the existing slow phase-only regression with real combat/lobby
    // boundaries, so the online duration model is exercised too.
    let phases = [
        0.0,
        0.042058558,
        0.086220045,
        0.132589605,
        0.181277644,
        0.232400085,
        0.286078647,
        0.342441138,
        0.401621754,
        0.4637614,
        0.529008028,
        0.597516988,
        0.669451396,
        0.744982524,
        0.824290209,
        0.907563277,
        0.995,
        1.0,
    ];
    for combat in [900, 1200] {
        replay_hypothetical_route(&phases, combat, 120, true);
    }
}

#[test]
fn twenty_six_stages_keep_exact_observed_count_beyond_the_learning_window() {
    let phases: Vec<_> = (0..=25).map(|index| index as f64 / 25.0).collect();
    // This checks capacity/counting rather than pretending arbitrary unseen
    // future progression is predictable to an exact integer.
    replay_hypothetical_route(&phases, 1200, 120, false);
}
