use chrono::{Local, NaiveDateTime, TimeZone};
use ecliptica_data_analyzer::{
    analysis::{Analyzer, RoundPhase},
    config::AppConfig,
    log_protocol::{FINAL_BOSS_NAMES, LogParser, ParsedEvent},
    osc::{render_message, selected_template},
};

fn second(line: &str) -> i64 {
    let time = NaiveDateTime::parse_from_str(&line[..19], "%Y.%m.%d %H:%M:%S").unwrap();
    Local
        .from_local_datetime(&time)
        .earliest()
        .unwrap()
        .timestamp()
}

#[test]
fn october_log_switches_to_report_in_gathering_scene_and_preserves_it_in_lobby() {
    // Protocol lines from the October log, with player names anonymized.
    // Includes lobby training damage, all three forms, duplicate Boss markers,
    // delayed phase-two death, summoned BabyGilgamesh ownership, and the late lobby.
    let fixture = include_str!("fixtures/gilgamesh_round.txt");
    let parser = LogParser::default();
    let mut analyzer = Analyzer::default();
    let config = AppConfig {
        message_template: "COMBAT".to_owned(),
        round_report_template: "REPORT {{round_total_damage}}".to_owned(),
        ..AppConfig::default()
    };
    let mut completed = None;
    let mut damage_records = 0;
    let mut taken_records = 0;
    let mut boss_records = 0;
    let mut last_second = 0;
    for line in fixture.lines() {
        let parsed = parser.parse(line);
        assert!(
            parsed.diagnostic.is_none(),
            "{line}: {:?}",
            parsed.diagnostic
        );
        analyzer.process_line(line);
        last_second = second(line);
        let snapshot = analyzer.snapshot_at(last_second);
        match parsed
            .event
            .expect("every fixture line is a recognized protocol event")
        {
            ParsedEvent::Boss { name, .. } => {
                boss_records += 1;
                assert_eq!(snapshot.phase, RoundPhase::Combat);
                assert_eq!(snapshot.boss.as_deref(), Some(name.as_str()));
                assert!(snapshot.round_report.is_none());
                assert_eq!(selected_template(&config, &snapshot), "COMBAT");
            }
            ParsedEvent::Damage { .. } => damage_records += 1,
            ParsedEvent::DamageTaken { .. } => taken_records += 1,
            ParsedEvent::Ownership { object, player, .. } => {
                if snapshot.boss.as_deref() == Some(object.as_str()) {
                    assert_eq!(snapshot.boss_lock.as_deref(), Some(player.as_str()));
                }
                if object == "BabyGilgamesh" {
                    assert_eq!(snapshot.boss.as_deref(), Some("GilgameshPhase3"));
                    assert_eq!(snapshot.boss_lock.as_deref(), Some("Fixture Player"));
                }
            }
            ParsedEvent::BossDefeated { name, .. } if name == "GilgameshPhase3" => {
                assert_eq!(snapshot.phase, RoundPhase::Lobby);
                assert!(!snapshot.round_metrics_active);
                assert!(!snapshot.boss_active);
                assert!(snapshot.boss_lock.is_none());
                assert!(!snapshot.has_step_estimate);
                let report = snapshot
                    .round_report
                    .as_ref()
                    .expect("immediate final report");
                assert!(report.has_duration_data);
                assert_eq!(report.duration_seconds, 23 * 60 + 4);
                assert_eq!(report.total_damage, 198_976);
                assert_eq!(report.damage_taken, 605);
                assert_eq!(
                    render_message(selected_template(&config, &snapshot), &snapshot).unwrap(),
                    "REPORT 198976"
                );
                completed = Some(serde_json::to_value(report).unwrap());
            }
            ParsedEvent::BossDefeated { .. } => {
                assert_eq!(snapshot.phase, RoundPhase::Combat);
                assert!(snapshot.round_report.is_none());
            }
            _ => {}
        }
        assert!(
            !snapshot.data_quality.degraded,
            "{:?}",
            snapshot.data_quality
        );
        if let Some(report) = &completed {
            assert!(snapshot.boss_lock.is_none());
            assert!(!snapshot.boss_active);
            assert!(!snapshot.has_step_estimate);
            assert_eq!(
                serde_json::to_value(&snapshot.round_report).unwrap(),
                *report
            );
            assert_eq!(
                selected_template(&config, &snapshot),
                config.round_report_template
            );
        }
    }
    assert!(damage_records > 100);
    assert!(taken_records > 10);
    assert_eq!(boss_records, 4);
    assert!(completed.is_some());
    assert_eq!(analyzer.snapshot_at(last_second).phase, RoundPhase::Lobby);
}

#[test]
fn terminal_deaths_only_finish_the_active_final_boss_and_never_the_next_run() {
    for boss in FINAL_BOSS_NAMES {
        let mut analyzer = Analyzer::default();
        let mut feed = |second, text: &str| {
            analyzer.process_line(&format!("2026.10.23 03:00:{second:02} Debug - {text}"));
        };
        feed(0, "[Behaviour] Entering Room: Ecliptica");
        feed(1, "ECLIPTICA - now in lobby");
        feed(
            2,
            "ECLIPTICA - now in stage: Stage_Final on phase: 1 as class: Spellhammer",
        );
        feed(
            3,
            &format!("ECLIPTICA - now fighting boss: {boss}(Clone) on phase: 1"),
        );
        feed(4, "Dealing 100 STRIKE damage");
        feed(5, &format!("Boss {boss} dead, personal damage dealt:"));
        let completed = analyzer.snapshot_at(second("2026.10.23 03:00:05"));
        assert_eq!(completed.phase, RoundPhase::Lobby);
        assert_eq!(completed.round_report.unwrap().total_damage, 100);

        for text in [
            format!("Boss {boss} dead, personal damage dealt:"),
            format!("ownership of {boss} transferred to Other Player"),
            format!("ECLIPTICA - now fighting boss: {boss}(Clone) on phase: 1"),
            "Dealing 999 STRIKE damage".to_owned(),
            "ECLIPTICA - now in lobby".to_owned(),
        ] {
            analyzer.process_line(&format!("2026.10.23 03:00:10 Debug - {text}"));
        }
        let lobby = analyzer.snapshot_at(second("2026.10.23 03:00:10"));
        assert_eq!(lobby.phase, RoundPhase::Lobby);
        assert_eq!(lobby.round_report.unwrap().total_damage, 100);
        assert!(!lobby.has_step_estimate);

        for text in [
            "ECLIPTICA - now in stage: Stage_Hall of Beginnings on phase: 0 as class: Spellhammer"
                .to_owned(),
            "ECLIPTICA - now fighting boss: Maxipuss(Clone) on phase: 0".to_owned(),
            format!("Boss {boss} dead, personal damage dealt:"),
            "Dealing 118 STRIKE damage".to_owned(),
        ] {
            analyzer.process_line(&format!("2026.10.23 03:00:20 Debug - {text}"));
        }
        let next = analyzer.snapshot_at(second("2026.10.23 03:00:21"));
        assert_eq!(next.phase, RoundPhase::Combat);
        assert!(next.round_metrics_active);
        assert!(next.round_report.is_none());
        assert_eq!(next.boss.as_deref(), Some("Maxipuss"));
        assert_eq!(next.round_total_damage, 118);
    }
}
