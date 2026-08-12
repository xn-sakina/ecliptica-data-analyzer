use std::{
    env,
    fs::{self, File},
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    sync::atomic::Ordering,
    thread,
    time::{Duration, Instant},
};

use chrono::Utc;

use crate::{
    analysis::{Analyzer, DataStatus},
    runtime::{EventLevel, SharedState},
};

const MIN_BACKOFF: Duration = Duration::from_millis(250);
const MAX_BACKOFF: Duration = Duration::from_secs(8);
const POLL_INTERVAL: Duration = Duration::from_millis(250);

pub fn spawn(shared: SharedState) -> thread::JoinHandle<()> {
    thread::Builder::new()
        .name("ecliptica-log-reader".to_owned())
        .spawn(move || run(shared))
        .expect("failed to start log reader")
}

fn run(shared: SharedState) {
    let mut analyzer = Analyzer::default();
    let mut current: Option<PathBuf> = None;
    let mut current_id: Option<file_id::FileId> = None;
    let mut offset = 0_u64;
    let mut pending = Vec::new();
    let mut last_growth: Option<Instant> = None;
    let mut backoff = MIN_BACKOFF;
    let mut next_discovery = Instant::now();
    let mut last_status = DataStatus::Searching;

    while !shared.shutdown.load(Ordering::Relaxed) {
        let override_path = shared.config.read().value.log_path_override.clone();
        if override_path.is_none() && Instant::now() >= next_discovery {
            match discover_latest_log() {
                Ok(found) if found != current => {
                    switch_file(
                        &shared,
                        &mut analyzer,
                        &mut current,
                        &mut current_id,
                        &mut offset,
                        &mut pending,
                        found,
                    );
                }
                Ok(_) => {}
                Err(error) => {
                    set_status(&shared, &mut last_status, DataStatus::Error);
                    shared.event(
                        EventLevel::Error,
                        format!(
                            "{}: {error:#}",
                            shared.text(crate::i18n::text::LOG_DISCOVERY_FAILED)
                        ),
                    );
                }
            }
            next_discovery = Instant::now() + Duration::from_secs(1);
        } else if let Some(path) = override_path {
            if current.as_ref() != Some(&path) {
                switch_file(
                    &shared,
                    &mut analyzer,
                    &mut current,
                    &mut current_id,
                    &mut offset,
                    &mut pending,
                    Some(path),
                );
            }
        }

        let Some(path) = current.clone() else {
            set_status(&shared, &mut last_status, DataStatus::Searching);
            publish(&shared, &mut analyzer, DataStatus::Searching, None);
            sleep_interruptible(&shared, backoff);
            backoff = (backoff * 2).min(MAX_BACKOFF);
            continue;
        };

        match file_id::get_file_id(&path) {
            Ok(next_id) => {
                if current_id
                    .replace(next_id)
                    .is_some_and(|old| old != next_id)
                {
                    analyzer.reset_all();
                    offset = 0;
                    pending.clear();
                    last_growth = None;
                    shared.event(
                        EventLevel::Warning,
                        shared.text(crate::i18n::text::LOG_REPLACED),
                    );
                }
            }
            Err(error) => {
                shared.event(
                    EventLevel::Error,
                    format!("{}: {error}", shared.text(crate::i18n::text::LOG_ID_FAILED)),
                );
            }
        }

        match read_increment(&path, &mut offset) {
            Ok(ReadResult::Truncated(bytes)) => {
                analyzer.reset_all();
                pending.clear();
                consume_lines(&mut analyzer, &mut pending, bytes);
                last_growth = Some(Instant::now());
                shared.event(
                    EventLevel::Warning,
                    shared.text(crate::i18n::text::LOG_TRUNCATED),
                );
            }
            Ok(ReadResult::Data(bytes)) => {
                if !bytes.is_empty() {
                    consume_lines(&mut analyzer, &mut pending, bytes);
                    last_growth = Some(Instant::now());
                }
                backoff = MIN_BACKOFF;
            }
            Err(error) => {
                analyzer.reset_all();
                current = None;
                current_id = None;
                offset = 0;
                pending.clear();
                last_growth = None;
                shared.event(
                    EventLevel::Error,
                    format!(
                        "{}: {error:#}",
                        shared.text(crate::i18n::text::LOG_READ_FAILED)
                    ),
                );
                set_status(&shared, &mut last_status, DataStatus::Error);
                publish(&shared, &mut analyzer, last_status, None);
                continue;
            }
        }

        let stale_after = Duration::from_secs(shared.config.read().value.stale_after_seconds);
        let status = match last_growth {
            Some(last) if last.elapsed() <= stale_after => DataStatus::Live,
            Some(_) => DataStatus::Stale,
            None => DataStatus::Recovering,
        };
        set_status(&shared, &mut last_status, status);
        publish(&shared, &mut analyzer, status, current.as_deref());
        sleep_interruptible(&shared, POLL_INTERVAL);
    }
}

fn switch_file(
    shared: &SharedState,
    analyzer: &mut Analyzer,
    current: &mut Option<PathBuf>,
    current_id: &mut Option<file_id::FileId>,
    offset: &mut u64,
    pending: &mut Vec<u8>,
    next: Option<PathBuf>,
) {
    analyzer.reset_all();
    *offset = 0;
    pending.clear();
    *current = next;
    *current_id = None;
    if let Some(path) = current {
        shared.event(
            EventLevel::Info,
            format!(
                "{}: {}",
                shared.text(crate::i18n::text::LOG_FOUND),
                path.display()
            ),
        );
    }
}

fn publish(shared: &SharedState, analyzer: &mut Analyzer, status: DataStatus, path: Option<&Path>) {
    for diagnostic in analyzer.take_protocol_diagnostics() {
        let language = shared.config.read().value.language;
        let detail = match diagnostic.code {
            "stage_details" => crate::i18n::text::DIAGNOSTIC_STAGE_DETAILS
                .get(language)
                .to_owned(),
            "boss" => crate::i18n::text::DIAGNOSTIC_BOSS.get(language).to_owned(),
            "boss_defeated" => crate::i18n::text::DIAGNOSTIC_BOSS_DEFEATED
                .get(language)
                .to_owned(),
            "ownership" => crate::i18n::text::DIAGNOSTIC_OWNERSHIP
                .get(language)
                .to_owned(),
            "intermission_missing" => crate::i18n::text::DIAGNOSTIC_INTERMISSION_MISSING
                .get(language)
                .to_owned(),
            "combat_metrics_missing" => crate::i18n::text::DIAGNOSTIC_COMBAT_METRICS_MISSING
                .get(language)
                .to_owned(),
            "room_phase_missing" => crate::i18n::text::DIAGNOSTIC_ROOM_PHASE_MISSING
                .get(language)
                .to_owned(),
            "timestamp" => crate::i18n::format_pattern(
                crate::i18n::text::DIAGNOSTIC_TIMESTAMP,
                language,
                &[("code", diagnostic.code.to_owned())],
            ),
            code => crate::i18n::format_pattern(
                crate::i18n::text::DIAGNOSTIC_VALUE,
                language,
                &[("code", code.to_owned())],
            ),
        };
        shared.event(
            EventLevel::Warning,
            format!(
                "{} [{}]: {}. {}",
                shared.text(crate::i18n::text::LOG_PROTOCOL_DEGRADED),
                diagnostic.code,
                detail,
                shared.text(crate::i18n::text::LOG_PROTOCOL_DEGRADED_SUFFIX)
            ),
        );
    }
    let mut snapshot = analyzer.snapshot_at(Utc::now().timestamp());
    snapshot.status = status;
    snapshot.source = path.map(|value| value.display().to_string());
    shared.apply_wasd_metric(&mut snapshot);
    *shared.snapshot.write() = snapshot;
}

fn set_status(shared: &SharedState, last: &mut DataStatus, next: DataStatus) {
    if *last == next {
        return;
    }
    *last = next;
    let (level, message) = match next {
        DataStatus::Live => (EventLevel::Info, shared.text(crate::i18n::text::LOG_LIVE)),
        // A quiet log is normal while choosing cards, walking between enemy
        // groups, or waiting in the lobby. Keep the safety state (and pause
        // OSC), but do not turn this routine idle period into an overlay alert.
        DataStatus::Stale => (EventLevel::Info, shared.text(crate::i18n::text::LOG_STALE)),
        DataStatus::Searching => (
            EventLevel::Warning,
            shared.text(crate::i18n::text::LOG_SEARCHING),
        ),
        DataStatus::Recovering => (
            EventLevel::Info,
            shared.text(crate::i18n::text::LOG_RECOVERING),
        ),
        DataStatus::Error => (EventLevel::Error, shared.text(crate::i18n::text::LOG_ERROR)),
    };
    shared.event(level, message);
}

enum ReadResult {
    Data(Vec<u8>),
    Truncated(Vec<u8>),
}

fn read_increment(path: &Path, offset: &mut u64) -> anyhow::Result<ReadResult> {
    let mut file = File::open(path)?;
    let length = file.metadata()?.len();
    let truncated = length < *offset;
    if truncated {
        *offset = 0;
    }
    file.seek(SeekFrom::Start(*offset))?;
    let mut bytes = Vec::with_capacity(length.saturating_sub(*offset) as usize);
    file.read_to_end(&mut bytes)?;
    *offset += bytes.len() as u64;
    drop(file);
    Ok(if truncated {
        ReadResult::Truncated(bytes)
    } else {
        ReadResult::Data(bytes)
    })
}

fn consume_lines(analyzer: &mut Analyzer, pending: &mut Vec<u8>, bytes: Vec<u8>) {
    pending.extend(bytes);
    let Some(last_newline) = pending.iter().rposition(|byte| *byte == b'\n') else {
        return;
    };
    let complete: Vec<u8> = pending.drain(..=last_newline).collect();
    for raw in complete.split(|byte| *byte == b'\n') {
        if raw.is_empty() {
            continue;
        }
        let line = String::from_utf8_lossy(raw);
        analyzer.process_line(line.trim_end_matches('\r'));
    }
}

pub fn discover_latest_log() -> anyhow::Result<Option<PathBuf>> {
    if let Some(path) = env::var_os("ECLIPTICA_LOG_PATH") {
        return Ok(Some(PathBuf::from(path)));
    }
    let directory = default_vrchat_log_dir()?;
    if !directory.is_dir() {
        return Ok(None);
    }
    let latest = fs::read_dir(directory)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("output_log_") && name.ends_with(".txt"))
        })
        .max_by_key(|path| path.file_name().map(|name| name.to_os_string()));
    Ok(latest)
}

fn default_vrchat_log_dir() -> anyhow::Result<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        let profile =
            env::var_os("USERPROFILE").ok_or_else(|| anyhow::anyhow!("USERPROFILE 未设置"))?;
        Ok(PathBuf::from(profile).join("AppData/LocalLow/VRChat/VRChat"))
    }
    #[cfg(target_os = "macos")]
    {
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("data/output_log_2026-08-02_23-32-00.txt");
        Ok(fixture.parent().unwrap().to_path_buf())
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        Ok(PathBuf::from("."))
    }
}

fn sleep_interruptible(shared: &SharedState, duration: Duration) {
    let until = Instant::now() + duration;
    while Instant::now() < until && !shared.shutdown.load(Ordering::Relaxed) {
        thread::sleep(
            Duration::from_millis(50).min(until.saturating_duration_since(Instant::now())),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn retains_half_line_and_half_utf8_until_newline() {
        let mut analyzer = Analyzer::default();
        let mut pending = Vec::new();
        let text = "2026.08.03 01:02:00 Debug - ECLIPTICA - now fighting boss: Maxipuss(Clone) on phase: 1\n2026.08.03 01:02:01 Debug - ownership of Maxipuss transferred to 小白\n";
        let bytes = text.as_bytes();
        let split = bytes.iter().position(|byte| *byte >= 0x80).unwrap() + 1;
        consume_lines(&mut analyzer, &mut pending, bytes[..split].to_vec());
        assert!(!pending.is_empty());
        consume_lines(&mut analyzer, &mut pending, bytes[split..].to_vec());
        assert_eq!(
            analyzer.snapshot_at(i64::MAX).boss_lock.as_deref(),
            Some("小白")
        );
    }

    #[test]
    fn detects_truncation_without_holding_file_open() {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        writeln!(file, "hello").unwrap();
        let path = file.path().to_owned();
        let mut offset = 0;
        assert!(matches!(
            read_increment(&path, &mut offset).unwrap(),
            ReadResult::Data(_)
        ));
        file.as_file_mut().set_len(0).unwrap();
        assert!(matches!(
            read_increment(&path, &mut offset).unwrap(),
            ReadResult::Truncated(_)
        ));
    }
}
