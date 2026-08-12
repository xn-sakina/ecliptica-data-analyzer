//! Central definition of the external log protocol consumed by the analyzer.
//!
//! Game updates are allowed to break individual signals: parsing a line never
//! panics and returns a diagnostic for recognizable-but-malformed input.

use chrono::{Local, NaiveDateTime, TimeZone};
use regex::Regex;

pub const TIMESTAMP_PATTERN: &str = r"^(?P<timestamp>\d{4}\.\d{2}\.\d{2} \d{2}:\d{2}:\d{2})";
pub const DAMAGE_PATTERN: &str = r"Dealing\s+(?P<amount>\d+)\s+(?:STRIKE|NON-STRIKE)\s+damage";
pub const DAMAGE_TAKEN_PATTERN: &str = r"damage has been taken:\s*(?P<amount>\d+)";
pub const BOSS_PATTERN: &str =
    r"ECLIPTICA - now fighting boss:\s*(?P<name>.+?)\(Clone\)\s+on phase:";
pub const BOSS_DEFEATED_PATTERN: &str = r"Boss\s+(?P<name>.+?)\s+dead, personal damage dealt:";
pub const OWNERSHIP_PATTERN: &str =
    r"ownership of\s+(?P<object>.+?)\s+transferred to\s+(?P<player>.+?)\s*$";
pub const STAGE_DETAILS_PATTERN: &str =
    r"ECLIPTICA - now in stage:\s*.+?\s+on phase:\s*(?P<phase>[0-9]+(?:\.[0-9]+)?)\s+as class:";

pub const ENTER_ECLIPTICA_MARKER: &str = "[Behaviour] Entering Room: Ecliptica";
pub const ENTER_ROOM_MARKER: &str = "[Behaviour] Entering Room:";
pub const LEAVE_ROOM_MARKER: &str = "[Behaviour] OnLeftRoom";
pub const STAGE_MARKER: &str = "ECLIPTICA - now in stage:";
pub const INTERMISSION_MARKERS: [&str; 2] = [
    "ECLIPTICA - now in intermission",
    "ECLIPTICA - now in lobby",
];
pub const BOSS_MARKER: &str = "ECLIPTICA - now fighting boss:";
pub const DAMAGE_MARKER: &str = "Dealing";
pub const DAMAGE_SUFFIX_MARKER: &str = "damage";
pub const DAMAGE_TAKEN_MARKER: &str = "damage has been taken:";
pub const OWNERSHIP_MARKER: &str = "ownership of";
pub const OWNERSHIP_TRANSFER_MARKER: &str = "transferred to";
pub const BOSS_DEFEATED_PREFIX: &str = "Boss ";
pub const BOSS_DEFEATED_SUFFIX: &str = "dead, personal damage dealt:";

/// Inventory used by diagnostics and by developers reviewing compatibility.
/// All strings that recognize game/VRChat log input must live in this module.
pub const LOG_PATTERN_INVENTORY: [(&str, &str); 14] = [
    ("timestamp", TIMESTAMP_PATTERN),
    ("enter_ecliptica", ENTER_ECLIPTICA_MARKER),
    ("enter_room", ENTER_ROOM_MARKER),
    ("leave_room", LEAVE_ROOM_MARKER),
    ("stage", STAGE_MARKER),
    ("stage_details", STAGE_DETAILS_PATTERN),
    (
        "intermission",
        "ECLIPTICA - now in intermission | now in lobby",
    ),
    ("damage", DAMAGE_PATTERN),
    ("damage_marker", "Dealing ... damage"),
    ("damage_taken", DAMAGE_TAKEN_PATTERN),
    ("boss", BOSS_PATTERN),
    ("boss_defeated", BOSS_DEFEATED_PATTERN),
    ("ownership", OWNERSHIP_PATTERN),
    ("ownership_marker", "ownership of ... transferred to ..."),
];

#[derive(Debug, Clone, PartialEq)]
pub enum ParsedEvent {
    EnterEcliptica { second: i64 },
    LeaveRoom { second: i64 },
    Stage { second: i64, phase: Option<f64> },
    Intermission { second: i64 },
    Boss { second: i64, name: String },
    BossDefeated { second: i64, name: String },
    Ownership { object: String, player: String },
    Damage { second: i64, amount: u64 },
    DamageTaken { second: i64, amount: u64 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtocolDiagnostic {
    pub code: &'static str,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedLine {
    pub event: Option<ParsedEvent>,
    pub diagnostic: Option<ProtocolDiagnostic>,
}

impl ParsedLine {
    fn ignored() -> Self {
        Self {
            event: None,
            diagnostic: None,
        }
    }

    fn event(event: ParsedEvent) -> Self {
        Self {
            event: Some(event),
            diagnostic: None,
        }
    }

    fn malformed(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            event: None,
            diagnostic: Some(ProtocolDiagnostic {
                code,
                message: message.into(),
            }),
        }
    }
}

pub struct LogParser {
    timestamp: Option<Regex>,
    damage: Option<Regex>,
    damage_taken: Option<Regex>,
    boss: Option<Regex>,
    boss_defeated: Option<Regex>,
    ownership: Option<Regex>,
    stage: Option<Regex>,
}

impl Default for LogParser {
    fn default() -> Self {
        // Invalid built-in patterns degrade into diagnostics instead of taking
        // the log-reader thread (and, in release builds, the process) down.
        Self {
            timestamp: Regex::new(TIMESTAMP_PATTERN).ok(),
            damage: Regex::new(DAMAGE_PATTERN).ok(),
            damage_taken: Regex::new(DAMAGE_TAKEN_PATTERN).ok(),
            boss: Regex::new(BOSS_PATTERN).ok(),
            boss_defeated: Regex::new(BOSS_DEFEATED_PATTERN).ok(),
            ownership: Regex::new(OWNERSHIP_PATTERN).ok(),
            stage: Regex::new(STAGE_DETAILS_PATTERN).ok(),
        }
    }
}

impl LogParser {
    pub fn parse(&self, line: &str) -> ParsedLine {
        if line.contains(ENTER_ECLIPTICA_MARKER) {
            return self.timed(line, "enter_ecliptica", |second| {
                ParsedEvent::EnterEcliptica { second }
            });
        }
        // Entering another room is an authoritative fallback when a leave line
        // disappeared. It prevents stale Ecliptica state from leaking forward.
        if line.contains(ENTER_ROOM_MARKER) {
            return self.timed(line, "room_transition", |second| ParsedEvent::LeaveRoom {
                second,
            });
        }
        if line.contains(LEAVE_ROOM_MARKER) {
            return self.timed(line, "leave_room", |second| ParsedEvent::LeaveRoom {
                second,
            });
        }
        if INTERMISSION_MARKERS
            .iter()
            .any(|marker| line.contains(marker))
        {
            return self.timed(line, "intermission", |second| ParsedEvent::Intermission {
                second,
            });
        }
        if line.contains(STAGE_MARKER) {
            let Some(second) = self.parse_second(line) else {
                return Self::timestamp_failure("stage");
            };
            let phase = self
                .stage
                .as_ref()
                .and_then(|pattern| pattern.captures(line))
                .and_then(|capture| capture.name("phase"))
                .and_then(|value| value.as_str().parse().ok());
            let diagnostic = phase.is_none().then(|| ProtocolDiagnostic {
                code: "stage_details",
                message: "阶段日志仍可识别，但 phase/class 详情格式已变化；回合状态保留，阶段估算降级为未知"
                    .to_owned(),
            });
            return ParsedLine {
                event: Some(ParsedEvent::Stage { second, phase }),
                diagnostic,
            };
        }
        if line.contains(DAMAGE_TAKEN_MARKER) {
            return self.timed_capture_u64(
                line,
                "damage_taken",
                self.damage_taken.as_ref(),
                |second, amount| ParsedEvent::DamageTaken { second, amount },
            );
        }
        if line.contains(BOSS_MARKER) {
            let Some(second) = self.parse_second(line) else {
                return Self::timestamp_failure("boss");
            };
            let Some(name) = capture_text(self.boss.as_ref(), line, "name") else {
                return ParsedLine::malformed(
                    "boss",
                    "Boss 日志格式已变化，Boss/锁定信息已降级为空",
                );
            };
            return ParsedLine::event(ParsedEvent::Boss { second, name });
        }
        if line.contains(BOSS_DEFEATED_PREFIX) && line.contains(BOSS_DEFEATED_SUFFIX) {
            let Some(second) = self.parse_second(line) else {
                return Self::timestamp_failure("boss_defeated");
            };
            return capture_text(self.boss_defeated.as_ref(), line, "name").map_or_else(
                || {
                    ParsedLine::malformed(
                        "boss_defeated",
                        "Boss 击败日志格式已变化，锁定状态会等待后续事件清理",
                    )
                },
                |name| ParsedLine::event(ParsedEvent::BossDefeated { second, name }),
            );
        }
        if line.contains(DAMAGE_MARKER) && line.contains(DAMAGE_SUFFIX_MARKER) {
            return self.timed_capture_u64(
                line,
                "damage",
                self.damage.as_ref(),
                |second, amount| ParsedEvent::Damage { second, amount },
            );
        }
        if line.contains(OWNERSHIP_MARKER) && line.contains(OWNERSHIP_TRANSFER_MARKER) {
            let object = capture_text(self.ownership.as_ref(), line, "object");
            let player = capture_text(self.ownership.as_ref(), line, "player");
            return match (object, player) {
                (Some(object), Some(player)) => {
                    ParsedLine::event(ParsedEvent::Ownership { object, player })
                }
                _ => {
                    ParsedLine::malformed("ownership", "所有权日志格式已变化，Boss Lock 已降级为空")
                }
            };
        }
        ParsedLine::ignored()
    }

    fn timed(
        &self,
        line: &str,
        code: &'static str,
        event: impl FnOnce(i64) -> ParsedEvent,
    ) -> ParsedLine {
        self.parse_second(line)
            .map(event)
            .map_or_else(|| Self::timestamp_failure(code), ParsedLine::event)
    }

    fn timed_capture_u64(
        &self,
        line: &str,
        code: &'static str,
        pattern: Option<&Regex>,
        event: impl FnOnce(i64, u64) -> ParsedEvent,
    ) -> ParsedLine {
        let Some(second) = self.parse_second(line) else {
            return Self::timestamp_failure(code);
        };
        let amount = pattern
            .and_then(|pattern| pattern.captures(line))
            .and_then(|capture| capture.name("amount"))
            .and_then(|value| value.as_str().parse::<u64>().ok());
        amount.map(|amount| event(second, amount)).map_or_else(
            || {
                ParsedLine::malformed(
                    code,
                    format!("{code} 日志格式或数值已变化，本条数据按缺失处理"),
                )
            },
            ParsedLine::event,
        )
    }

    fn parse_second(&self, line: &str) -> Option<i64> {
        let timestamp = self
            .timestamp
            .as_ref()?
            .captures(line)?
            .name("timestamp")?
            .as_str();
        let parsed = NaiveDateTime::parse_from_str(timestamp, "%Y.%m.%d %H:%M:%S").ok()?;
        Local
            .from_local_datetime(&parsed)
            .earliest()
            .map(|value| value.timestamp())
    }

    fn timestamp_failure(signal: &'static str) -> ParsedLine {
        ParsedLine::malformed(
            "timestamp",
            format!("{signal} 日志已出现，但时间戳格式无法识别；本条数据已安全跳过"),
        )
    }
}

fn capture_text(pattern: Option<&Regex>, line: &str, name: &str) -> Option<String> {
    pattern?
        .captures(line)?
        .name(name)
        .map(|value| value.as_str().trim().to_owned())
        .filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn malformed_known_lines_are_diagnostics_not_panics() {
        let parser = LogParser::default();
        let malformed = [
            "future timestamp Dealing lots STRIKE damage",
            "2026.08.12 12:00:00 Debug - damage has been taken: many",
            "2026.08.12 12:00:00 Debug - ECLIPTICA - now fighting boss: changed",
            "2026.08.12 12:00:00 Debug - ownership of Boss transferred to",
        ];
        for line in malformed {
            let parsed = parser.parse(line);
            assert!(parsed.event.is_none());
            assert!(parsed.diagnostic.is_some());
        }
    }

    #[test]
    fn entering_another_room_closes_stale_ecliptica_state() {
        let parsed = LogParser::default()
            .parse("2026.08.12 12:00:00 Debug - [Behaviour] Entering Room: Another World");
        assert!(matches!(parsed.event, Some(ParsedEvent::LeaveRoom { .. })));
    }

    #[test]
    fn boss_defeat_keeps_its_timestamp_for_round_boundaries() {
        let parsed = LogParser::default().parse(
            "2026.08.12 07:49:28 Debug - Boss JimBringerPhase3 dead, personal damage dealt:",
        );
        assert!(matches!(
            parsed.event,
            Some(ParsedEvent::BossDefeated { name, .. }) if name == "JimBringerPhase3"
        ));
    }
}
