use chrono::{Local, TimeZone};
use ecliptica_data_analyzer::analysis::Analyzer;

#[test]
fn recovers_a_representative_round_from_fixture() {
    let fixture = std::fs::read_to_string(format!(
        "{}/tests/fixtures/ecliptica_round.txt",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap();
    let lines: Vec<_> = fixture.lines().collect();
    let mut analyzer = Analyzer::default();

    for line in &lines[..5] {
        analyzer.process_line(line);
    }
    let after_damage = Local
        .with_ymd_and_hms(2026, 8, 3, 1, 2, 6)
        .earliest()
        .unwrap()
        .timestamp();
    let snapshot = analyzer.snapshot_at(after_damage);
    assert_eq!(snapshot.latest_dps, 42);
    assert_eq!(snapshot.average_dps, 1.4);

    for line in &lines[5..8] {
        analyzer.process_line(line);
    }
    let snapshot = analyzer.snapshot_at(after_damage + 2);
    assert_eq!(snapshot.boss_lock.as_deref(), Some("Alice"));

    for line in &lines[8..] {
        analyzer.process_line(line);
    }
    let snapshot = analyzer.snapshot_at(after_damage + 4);
    assert_eq!(snapshot.boss_lock, None);
    assert_eq!(snapshot.latest_dps, 0);
    assert!(!snapshot.boss_active);
}
