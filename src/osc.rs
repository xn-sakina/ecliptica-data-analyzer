use std::{
    collections::VecDeque,
    net::UdpSocket,
    sync::atomic::Ordering,
    thread,
    time::{Duration, Instant},
};

use handlebars::Handlebars;
use rosc::{OscMessage, OscPacket, OscType, encoder};

use crate::{
    analysis::{DataStatus, GameSnapshot, RoundPhase, normalized_name},
    config::AppConfig,
    runtime::{EventLevel, SharedState},
};

pub fn spawn(shared: SharedState) -> thread::JoinHandle<()> {
    thread::Builder::new()
        .name("ecliptica-osc-sender".to_owned())
        .spawn(move || run(shared))
        .expect("failed to start OSC sender")
}

fn run(shared: SharedState) {
    let socket = match UdpSocket::bind("127.0.0.1:0") {
        Ok(socket) => socket,
        Err(error) => {
            shared.event(
                EventLevel::Error,
                format!(
                    "{}: {error}",
                    shared.text(crate::i18n::text::OSC_INIT_FAILED)
                ),
            );
            return;
        }
    };
    let mut schedule = SendSchedule::new(Instant::now());
    let mut queue = SendQueue::default();
    let mut rate_limiter = ChatboxRateLimiter::default();
    let mut published = PublishedChatboxState::default();
    let mut last_error = String::new();

    while !shared.shutdown.load(Ordering::Relaxed) {
        let (config, revision) = {
            let live = shared.config.read();
            (live.value.clone(), live.revision)
        };
        let snapshot = shared.snapshot.read().clone();
        let interval = config.send_interval.duration();
        let now = Instant::now();
        let context = broadcast_context(&snapshot);
        let config_changed = schedule.observe_config(revision, interval, now);
        let context_changed = schedule.observe_broadcast_context(context, now);
        let wasd_changed = schedule.observe_no_wasd_condition(snapshot.no_wasd_for_10s, now);

        if config_changed || context_changed || wasd_changed {
            // Anything already pending was rendered for an older presentation
            // state. A one-slot coalescing queue intentionally discards it.
            queue.clear();
        }

        let eligible = config.osc_enabled
            && snapshot.status == DataStatus::Live
            && snapshot.in_ecliptica
            && context.is_some();
        if !eligible {
            queue.clear();
            thread::sleep(Duration::from_millis(50));
            continue;
        }

        if let Some(priority) =
            edge_priority(context_changed, wasd_changed, snapshot.no_wasd_for_10s)
        {
            queue.enqueue(context.unwrap(), priority);
        } else if schedule.is_due(now) {
            queue.enqueue(context.unwrap(), SendPriority::Regular);
        }

        if let Some(pending) = queue.pending() {
            if !rate_limiter.can_send(pending.priority, now) {
                thread::sleep(Duration::from_millis(50));
                continue;
            }
            // Render at dispatch time, not enqueue time. If the room changed
            // while this request was waiting for a rate-limit slot, discard
            // the obsolete request instead of ever sending stale text.
            let (latest_config, latest_revision) = {
                let live = shared.config.read();
                (live.value.clone(), live.revision)
            };
            let latest_snapshot = shared.snapshot.read().clone();
            let latest_context = broadcast_context(&latest_snapshot);
            if latest_revision != revision
                || latest_context != Some(pending.context)
                || !latest_config.osc_enabled
                || latest_snapshot.status != DataStatus::Live
                || !latest_snapshot.in_ecliptica
            {
                queue.clear();
                thread::sleep(Duration::from_millis(50));
                continue;
            }

            let update =
                render_configured_message(&latest_config, &latest_snapshot).map(|message| {
                    published.next_update(message, pending.context, latest_config.language)
                });
            let result = update.and_then(|update| {
                send_chatbox_update(&socket, &latest_config.osc_address, &update)
                    .map(|outcome| (update, outcome))
            });
            match result {
                Ok((update, outcome)) => {
                    published.complete(&update);
                    if outcome.sent_packet() {
                        rate_limiter.record_send(Instant::now());
                    }
                    if pending.priority == SendPriority::StateChange {
                        tracing::info!(
                            context = ?pending.context,
                            "{}",
                            shared.text(crate::i18n::text::OSC_STATE_PACKET_SUBMITTED)
                        );
                    }
                    queue.clear();
                    schedule.complete_cycle(latest_config.send_interval.duration(), Instant::now());
                    last_error.clear();
                }
                Err(error) => {
                    let message = format!(
                        "{}: {error:#}",
                        shared.text(crate::i18n::text::OSC_SEND_FAILED)
                    );
                    if message != last_error {
                        shared.event(EventLevel::Error, message.clone());
                        last_error = message;
                    }
                }
            }
        }
        thread::sleep(Duration::from_millis(50));
    }
}

#[cfg(test)]
fn broadcast_context_ready(snapshot: &GameSnapshot) -> bool {
    broadcast_context(snapshot).is_some()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BroadcastContext {
    Combat(u64),
    RoundReport(u64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SendPriority {
    Regular,
    StateChange,
}

fn edge_priority(
    context_changed: bool,
    wasd_changed: bool,
    no_wasd_for_10s: bool,
) -> Option<SendPriority> {
    if context_changed || (wasd_changed && !no_wasd_for_10s) {
        Some(SendPriority::StateChange)
    } else if wasd_changed {
        // Becoming idle may wait in the coalescing queue like a regular
        // refresh. It must not consume the reserved transition slot that a
        // subsequent WASD press needs to remove stale idle text.
        Some(SendPriority::Regular)
    } else {
        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PendingSend {
    context: BroadcastContext,
    priority: SendPriority,
}

/// A latest-value queue: pending work is a request to render the newest
/// snapshot, never a frozen message string. Its capacity is deliberately one,
/// so normal updates coalesce and state transitions can erase obsolete work.
#[derive(Debug, Default)]
struct SendQueue {
    pending: Option<PendingSend>,
}

impl SendQueue {
    fn enqueue(&mut self, context: BroadcastContext, priority: SendPriority) {
        let priority = match self.pending {
            Some(pending)
                if pending.context == context && pending.priority == SendPriority::StateChange =>
            {
                SendPriority::StateChange
            }
            _ => priority,
        };
        self.pending = Some(PendingSend { context, priority });
    }

    fn clear(&mut self) {
        self.pending = None;
    }

    fn pending(&self) -> Option<PendingSend> {
        self.pending
    }
}

const CHATBOX_RATE_WINDOW: Duration = Duration::from_secs(5);
const REGULAR_SEND_LIMIT: usize = 4;
const ABSOLUTE_SEND_LIMIT: usize = 5;

/// Mirrors VRChat's five-messages-per-five-seconds limiter locally. Regular
/// traffic may use only four slots; the fifth stays reserved for a phase or
/// alert-state transition, preventing packets from accumulating in VRChat.
#[derive(Debug, Default)]
struct ChatboxRateLimiter {
    sent_at: VecDeque<Instant>,
}

impl ChatboxRateLimiter {
    fn can_send(&mut self, priority: SendPriority, now: Instant) -> bool {
        self.sent_at
            .retain(|sent| now.saturating_duration_since(*sent) < CHATBOX_RATE_WINDOW);
        let limit = match priority {
            SendPriority::Regular => REGULAR_SEND_LIMIT,
            SendPriority::StateChange => ABSOLUTE_SEND_LIMIT,
        };
        self.sent_at.len() < limit
    }

    fn record_send(&mut self, now: Instant) {
        self.sent_at.push_back(now);
    }
}

#[derive(Debug, Default)]
struct PublishedChatboxState {
    /// Once this process has sent visible text, only another non-empty message
    /// can deterministically replace it in VRChat.
    remote_may_have_visible_content: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ChatboxUpdate {
    Message(String),
    SkipEmpty,
}

impl PublishedChatboxState {
    fn next_update(
        &self,
        message: String,
        context: BroadcastContext,
        language: crate::i18n::Language,
    ) -> ChatboxUpdate {
        if !message.trim().is_empty() {
            ChatboxUpdate::Message(message)
        } else if self.remote_may_have_visible_content {
            // VRChat does not use an empty OSC message to replace persistent
            // Chatbox content. Send an explicit state-appropriate placeholder
            // until the selected template produces real data.
            let replacement = match context {
                BroadcastContext::Combat(_) => {
                    crate::i18n::text::EMPTY_COMBAT_REPLACEMENT.get(language)
                }
                BroadcastContext::RoundReport(_) => {
                    crate::i18n::text::EMPTY_REPORT_REPLACEMENT.get(language)
                }
            };
            ChatboxUpdate::Message(replacement.to_owned())
        } else {
            ChatboxUpdate::SkipEmpty
        }
    }

    fn complete(&mut self, update: &ChatboxUpdate) {
        if matches!(update, ChatboxUpdate::Message(_)) {
            self.remote_may_have_visible_content = true;
        }
    }
}

fn broadcast_context(snapshot: &GameSnapshot) -> Option<BroadcastContext> {
    if snapshot.phase == RoundPhase::Combat {
        Some(BroadcastContext::Combat(snapshot.combat_round_epoch))
    } else if snapshot.round_report.is_some() {
        Some(BroadcastContext::RoundReport(snapshot.combat_round_epoch))
    } else {
        None
    }
}

pub fn selected_template<'a>(config: &'a AppConfig, snapshot: &GameSnapshot) -> &'a str {
    if snapshot.phase == RoundPhase::Combat {
        &config.message_template
    } else if snapshot.round_report.is_some() {
        &config.round_report_template
    } else {
        &config.message_template
    }
}

pub fn render_configured_message(
    config: &AppConfig,
    snapshot: &GameSnapshot,
) -> anyhow::Result<String> {
    render_message_with_display_name(
        selected_template(config, snapshot),
        snapshot,
        &config.display_name,
    )
}

struct SendSchedule {
    observed_revision: u64,
    next_send: Instant,
    last_broadcast_context: Option<BroadcastContext>,
    last_no_wasd_for_10s: Option<bool>,
}

impl SendSchedule {
    fn new(now: Instant) -> Self {
        Self {
            observed_revision: 0,
            next_send: now,
            last_broadcast_context: None,
            last_no_wasd_for_10s: None,
        }
    }

    fn observe_config(&mut self, revision: u64, interval: Duration, now: Instant) -> bool {
        if revision != self.observed_revision {
            self.observed_revision = revision;
            // Applying settings must never create an extra Chatbox packet. Wait
            // for a complete normal interval before using the new template.
            self.next_send = now + interval;
            true
        } else {
            false
        }
    }

    fn is_due(&self, now: Instant) -> bool {
        now >= self.next_send
    }

    fn observe_broadcast_context(
        &mut self,
        current: Option<BroadcastContext>,
        now: Instant,
    ) -> bool {
        let changed = self.last_broadcast_context != current;
        self.last_broadcast_context = current;
        if changed && current.is_some() {
            // The local rate limiter reserves a fifth slot specifically for
            // this transition, so it is safe to replace queued work and send
            // the new context immediately.
            self.next_send = now;
        }
        changed
    }

    fn observe_no_wasd_condition(&mut self, current: bool, now: Instant) -> bool {
        let changed = self
            .last_no_wasd_for_10s
            .replace(current)
            .is_some_and(|previous| previous != current);
        if changed {
            self.next_send = now;
        }
        changed
    }

    fn complete_cycle(&mut self, interval: Duration, now: Instant) {
        self.next_send = now + interval;
    }
}

pub fn render_message(template: &str, snapshot: &GameSnapshot) -> anyhow::Result<String> {
    render_message_with_display_name(template, snapshot, "")
}

fn render_message_with_display_name(
    template: &str,
    snapshot: &GameSnapshot,
    display_name: &str,
) -> anyhow::Result<String> {
    let mut handlebars = Handlebars::new();
    handlebars.set_strict_mode(true);
    handlebars.register_template_string("message", template)?;
    let report = snapshot.round_report.as_ref();
    let report_has_output = report.is_some_and(|value| value.has_output_data);
    let data = serde_json::json!({
        "latest_dps": snapshot.latest_dps_text(),
        "avg_dps": snapshot.average_dps_text(),
        "round_avg_dps": snapshot.round_average_dps_text(),
        "round_effective_dps": snapshot.round_effective_dps_text(),
        "round_burst_10s": snapshot.round_burst_10s_dps_text(),
        "round_damage_taken": snapshot.round_damage_taken.to_string(),
        "max_dps": snapshot.max_dps_text(),
        "boss_lock": snapshot.boss_lock.as_deref().unwrap_or(""),
        "boss": snapshot.boss.as_deref().unwrap_or(""),
        "status": snapshot.status.label(),
        "has_latest_dps": snapshot.has_damage_data,
        "has_avg_dps": snapshot.has_damage_data,
        "has_round_avg_dps": snapshot.has_damage_data,
        "has_round_effective_dps": snapshot.has_damage_data,
        "has_round_burst_10s": snapshot.round_burst_10s_dps.is_some(),
        "has_round_damage_taken": snapshot.has_damage_data || snapshot.round_damage_taken > 0,
        "has_max_dps": snapshot.has_max_dps_data,
        "is_self_boss_locked": is_self_boss_locked(snapshot, display_name),
        "rapid_damage_danger": snapshot.rapid_damage_danger,
        "no_dps_for_10s": snapshot.no_dps_for_10s,
        "no_wasd_for_10s": snapshot.no_wasd_for_10s,
        "waiting_for_next_round": snapshot.waiting_for_next_round,
        "phase": snapshot.phase.label(),
        "has_round_report": report.is_some(),
        "has_round_duration": report.is_some_and(|value| value.has_duration_data),
        "has_round_combat_duration": report_has_output,
        "has_round_report_avg_dps": report_has_output,
        "has_round_max_dps": report_has_output,
        "has_round_report_effective_dps": report_has_output,
        "has_round_report_burst_10s": report.and_then(|value| value.burst_10s_dps).is_some(),
        "has_dps_growth_rate": report.is_some_and(|value| value.has_dps_growth_rate),
        "has_round_dps_growth_rate": report.is_some_and(|value| value.has_dps_growth_rate),
        "has_round_longest_standstill": report.is_some_and(|value| value.has_longest_standstill_data),
        "has_step_estimate": report.is_some() && snapshot.has_step_estimate,
        "current_step": if snapshot.has_step_estimate { snapshot.current_step.to_string() } else { "-".to_owned() },
        "until_boss_step": if snapshot.has_step_estimate { snapshot.until_boss_step.to_string() } else { "-".to_owned() },
        "round_duration": report.map(|value| value.duration_text()).unwrap_or_else(|| "-".to_owned()),
        "round_combat_duration": report.map(|value| value.combat_duration_text()).unwrap_or_else(|| "-".to_owned()),
        "round_total_damage": report.map(|value| value.total_damage.to_string()).unwrap_or_else(|| "-".to_owned()),
        "round_report_avg_dps": report.map(|value| value.average_dps_text()).unwrap_or_else(|| "-".to_owned()),
        "round_max_dps": report.map(|value| value.max_dps_text()).unwrap_or_else(|| "-".to_owned()),
        "round_report_effective_dps": report.map(|value| value.effective_dps_text()).unwrap_or_else(|| "-".to_owned()),
        "round_report_burst_10s": report.map(|value| value.burst_10s_dps_text()).unwrap_or_else(|| "-".to_owned()),
        "dps_growth_rate": report.map(|value| value.dps_growth_rate_text()).unwrap_or_else(|| "0".to_owned()),
        "round_dps_growth_rate": report.map(|value| value.dps_growth_rate_text()).unwrap_or_else(|| "0".to_owned()),
        "round_report_damage_taken": report.map(|value| value.damage_taken.to_string()).unwrap_or_else(|| "-".to_owned()),
        "round_longest_standstill": report.map(|value| value.longest_standstill_text()).unwrap_or_else(|| "-".to_owned()),
    });
    let rendered = handlebars.render("message", &data)?;
    // Handlebars intentionally preserves line breaks around hidden blocks.
    // Strip only the final message boundary before applying VRChat's limit;
    // whitespace inside the message remains untouched.
    Ok(limit_chatbox(rendered.trim()))
}

fn is_self_boss_locked(snapshot: &GameSnapshot, display_name: &str) -> bool {
    let display_name = display_name.trim();
    !display_name.is_empty()
        && snapshot.boss_active
        && snapshot
            .boss_lock
            .as_deref()
            .is_some_and(|name| normalized_name(name) == normalized_name(display_name))
}

fn limit_chatbox(message: &str) -> String {
    let mut output = String::new();
    let mut newlines = 0;
    for (chars, character) in message.chars().enumerate() {
        if chars >= 144 || (character == '\n' && newlines >= 8) {
            break;
        }
        output.push(character);
        newlines += usize::from(character == '\n');
    }
    output
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SendOutcome {
    Sent,
    SkippedEmpty,
}

impl SendOutcome {
    fn sent_packet(self) -> bool {
        matches!(self, Self::Sent)
    }
}

fn send_chatbox_update(
    socket: &UdpSocket,
    address: &str,
    update: &ChatboxUpdate,
) -> anyhow::Result<SendOutcome> {
    match update {
        ChatboxUpdate::Message(text) => send_chatbox(socket, address, text),
        ChatboxUpdate::SkipEmpty => Ok(SendOutcome::SkippedEmpty),
    }
}

fn send_chatbox(socket: &UdpSocket, address: &str, text: &str) -> anyhow::Result<SendOutcome> {
    // Keep this final gate defensive even though the renderer already trims,
    // so ordinary callers cannot accidentally transmit a blank packet. The
    // state replacements must also be real non-empty messages.
    let text = text.trim();
    if text.is_empty() {
        return Ok(SendOutcome::SkippedEmpty);
    }
    send_chatbox_packet(socket, address, text)?;
    Ok(SendOutcome::Sent)
}

fn send_chatbox_packet(socket: &UdpSocket, address: &str, text: &str) -> anyhow::Result<()> {
    let packet = OscPacket::Message(OscMessage {
        addr: "/chatbox/input".to_owned(),
        args: vec![
            OscType::String(text.to_owned()),
            OscType::Bool(true),
            OscType::Bool(false),
        ],
    });
    let bytes = encoder::encode(&packet)?;
    socket.send_to(&bytes, address)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_supported_variables_and_preserves_newlines() {
        let snapshot = GameSnapshot {
            latest_dps: 123,
            average_dps: 45.67,
            round_effective_dps: 54.32,
            round_burst_10s_dps: Some(78.9),
            round_damage_taken: 16,
            max_dps: 321,
            has_damage_data: true,
            has_max_dps_data: true,
            boss_lock: Some("Alice".to_owned()),
            ..GameSnapshot::default()
        };
        assert_eq!(
            render_message(
                "{{latest_dps}}\n{{avg_dps}}\n{{round_avg_dps}}\n{{round_effective_dps}}\n{{round_burst_10s}}\n{{round_damage_taken}}\n{{max_dps}}\n{{boss_lock}}",
                &snapshot,
            )
            .unwrap(),
            "123\n45.7\n0.0\n54.3\n78.9\n16\n321\nAlice"
        );
    }

    #[test]
    fn renders_dashes_before_any_damage_data_arrives() {
        let snapshot = GameSnapshot::default();
        assert_eq!(
            render_message(
                "DPS: {{latest_dps}} / AVG: {{avg_dps}} / ROUND: {{round_avg_dps}}",
                &snapshot,
            )
            .unwrap(),
            "DPS: - / AVG: - / ROUND: -"
        );
    }

    #[test]
    fn truncates_at_unicode_char_and_line_limits() {
        assert_eq!(limit_chatbox(&"伤".repeat(200)).chars().count(), 144);
        assert_eq!(
            limit_chatbox("1\n2\n3\n4\n5\n6\n7\n8\n9\n10")
                .lines()
                .count(),
            9
        );
    }

    #[test]
    fn conditional_blocks_hide_unavailable_values_but_keep_real_zero() {
        let template = "{{#if has_latest_dps}}DPS: {{latest_dps}}{{/if}}\n{{#if boss_lock}}LOCK: {{boss_lock}}{{/if}}";
        assert_eq!(
            render_message(template, &GameSnapshot::default()).unwrap(),
            ""
        );

        let snapshot = GameSnapshot {
            has_damage_data: true,
            latest_dps: 0,
            ..GameSnapshot::default()
        };
        assert_eq!(render_message(template, &snapshot).unwrap(), "DPS: 0");
    }

    #[test]
    fn help_documented_handlebars_syntax_is_supported() {
        let snapshot = GameSnapshot {
            phase: RoundPhase::Combat,
            has_damage_data: true,
            latest_dps: 42,
            round_damage_taken: 51,
            rapid_damage_danger: true,
            no_dps_for_10s: false,
            ..GameSnapshot::default()
        };

        let template = "{{#unless boss_lock}}NO LOCK{{/unless}}|{{#if (eq phase \"COMBAT\")}}COMBAT{{/if}}|{{#if (ne status \"ERROR\")}}OK{{/if}}|{{#if (gt round_damage_taken 50)}}HURT{{/if}}|{{#if (and has_latest_dps (or rapid_damage_danger no_dps_for_10s))}}ALERT{{/if}}|{{#if (not waiting_for_next_round)}}PLAYING{{/if}}|{{! hidden comment }}DPS: {{latest_dps}}";

        assert_eq!(
            render_message(template, &snapshot).unwrap(),
            "NO LOCK|COMBAT|OK|HURT|ALERT|PLAYING|DPS: 42"
        );
        assert!(crate::config::validate_template(template).is_ok());
    }

    #[test]
    fn configured_player_lock_flag_requires_a_name_and_active_matching_boss_lock() {
        let template = "{{#if is_self_boss_locked}}LOCKED{{else}}SAFE{{/if}}";
        let snapshot = GameSnapshot {
            boss_active: true,
            boss_lock: Some("Ａlice".to_owned()),
            ..GameSnapshot::default()
        };

        assert_eq!(render_message(template, &snapshot).unwrap(), "SAFE");

        let matching = AppConfig {
            display_name: "Alice".to_owned(),
            message_template: template.to_owned(),
            ..AppConfig::default()
        };
        assert_eq!(
            render_configured_message(&matching, &snapshot).unwrap(),
            "LOCKED"
        );

        let missing_name = AppConfig {
            display_name: "   ".to_owned(),
            message_template: template.to_owned(),
            ..AppConfig::default()
        };
        assert_eq!(
            render_configured_message(&missing_name, &snapshot).unwrap(),
            "SAFE"
        );
    }

    #[test]
    fn rapid_damage_danger_is_available_to_message_templates() {
        let template = "{{#if rapid_damage_danger}}危险：快速掉血{{/if}}";
        assert_eq!(
            render_message(template, &GameSnapshot::default()).unwrap(),
            ""
        );
        let snapshot = GameSnapshot {
            rapid_damage_danger: true,
            ..GameSnapshot::default()
        };
        assert_eq!(
            render_message(template, &snapshot).unwrap(),
            "危险：快速掉血"
        );
    }

    #[test]
    fn no_dps_for_10s_is_available_to_message_templates() {
        let template = "{{#if no_dps_for_10s}}10 秒无输出{{/if}}";
        assert_eq!(
            render_message(template, &GameSnapshot::default()).unwrap(),
            ""
        );
        let snapshot = GameSnapshot {
            no_dps_for_10s: true,
            ..GameSnapshot::default()
        };
        assert_eq!(render_message(template, &snapshot).unwrap(), "10 秒无输出");
    }

    #[test]
    fn wasd_idle_boolean_is_directly_available_to_templates() {
        let template = "{{#if no_wasd_for_10s}}IDLE{{else}}ACTIVE{{/if}}";
        assert_eq!(
            render_message(template, &GameSnapshot::default()).unwrap(),
            "ACTIVE"
        );
        let idle = GameSnapshot {
            wasd_listener_available: true,
            no_wasd_for_10s: true,
            ..GameSnapshot::default()
        };
        assert_eq!(render_message(template, &idle).unwrap(), "IDLE");
    }

    #[test]
    fn report_flags_distinguish_missing_output_and_incomplete_burst_windows() {
        let snapshot = GameSnapshot {
            round_report: Some(crate::analysis::RoundReport {
                has_duration_data: true,
                has_output_data: false,
                duration_seconds: 20,
                combat_duration_seconds: 1,
                total_damage: 0,
                average_dps: 0.0,
                max_dps: 0,
                effective_dps: 0.0,
                burst_10s_dps: None,
                dps_growth_rate: 0.0,
                has_dps_growth_rate: false,
                damage_taken: 23,
                has_longest_standstill_data: false,
                longest_standstill_seconds: 0,
            }),
            ..GameSnapshot::default()
        };
        let template = "{{#if has_round_duration}}duration={{round_duration}}{{/if}}|{{#if has_round_combat_duration}}{{round_combat_duration}}{{else}}no-combat-duration{{/if}}|{{#if has_round_report_avg_dps}}{{round_report_avg_dps}}{{else}}no-average{{/if}}|{{#if has_round_max_dps}}max={{round_max_dps}}{{else}}no-max{{/if}}|{{#if has_round_report_effective_dps}}effective={{round_report_effective_dps}}{{else}}no-effective{{/if}}|{{#if has_round_report_burst_10s}}{{round_report_burst_10s}}{{else}}no-burst{{/if}}|{{#if has_round_report}}taken={{round_report_damage_taken}}{{/if}}";

        assert_eq!(
            render_message(template, &snapshot).unwrap(),
            "duration=00:20|no-combat-duration|no-average|no-max|no-effective|no-burst|taken=23"
        );
    }

    #[test]
    fn hidden_first_line_does_not_leave_a_leading_newline() {
        let template =
            "{{#if has_latest_dps}}DPS: {{latest_dps}}{{/if}}\n始终显示\n  保留内部缩进  ";
        assert_eq!(
            render_message(template, &GameSnapshot::default()).unwrap(),
            "始终显示\n  保留内部缩进"
        );
    }

    #[test]
    fn default_template_is_empty_without_data_and_compact_with_data() {
        assert_eq!(
            render_message(crate::config::DEFAULT_TEMPLATE, &GameSnapshot::default()).unwrap(),
            ""
        );

        let snapshot = GameSnapshot {
            latest_dps: 0,
            round_effective_dps: 45.6,
            round_burst_10s_dps: Some(72.3),
            round_damage_taken: 18,
            has_damage_data: true,
            ..GameSnapshot::default()
        };
        assert_eq!(
            render_message(crate::config::DEFAULT_TEMPLATE, &snapshot).unwrap(),
            "DPS: 0"
        );

        let incoming_only = GameSnapshot {
            round_damage_taken: 23,
            ..GameSnapshot::default()
        };
        assert_eq!(
            render_message(crate::config::DEFAULT_TEMPLATE, &incoming_only).unwrap(),
            ""
        );
    }

    #[test]
    fn default_round_report_is_compact_and_uses_archived_values() {
        let snapshot = GameSnapshot {
            round_report: Some(crate::analysis::RoundReport {
                has_duration_data: true,
                has_output_data: true,
                duration_seconds: 367,
                combat_duration_seconds: 328,
                total_damage: 12_480,
                average_dps: 38.0,
                max_dps: 146,
                effective_dps: 82.4,
                burst_10s_dps: Some(126.7),
                dps_growth_rate: 0.0,
                has_dps_growth_rate: false,
                damage_taken: 73,
                has_longest_standstill_data: true,
                longest_standstill_seconds: 74,
            }),
            ..GameSnapshot::default()
        };
        let rendered = render_message(crate::config::DEFAULT_ROUND_REPORT_TEMPLATE, &snapshot)
            .expect("default report template should render");
        assert_eq!(
            rendered,
            "【回合战报】\n用时 06:07｜我打了 12480\n平均 82.4 DPS｜最高 146 DPS"
        );
        assert!(rendered.chars().count() < 144);
    }

    #[test]
    fn report_step_estimate_is_hidden_until_flagged_and_renders_zero() {
        let mut snapshot = GameSnapshot {
            round_report: Some(crate::analysis::RoundReport {
                has_duration_data: true,
                has_output_data: true,
                duration_seconds: 60,
                combat_duration_seconds: 50,
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
            current_step: 12,
            until_boss_step: 0,
            ..GameSnapshot::default()
        };
        let template =
            "{{#if has_step_estimate}}第{{current_step}}回合，距Jim {{until_boss_step}}回合{{/if}}";
        assert_eq!(render_message(template, &snapshot).unwrap(), "");

        snapshot.has_step_estimate = true;
        assert_eq!(
            render_message(template, &snapshot).unwrap(),
            "第12回合，距Jim 0回合"
        );
    }

    #[test]
    fn report_dps_growth_rate_defaults_to_zero_and_uses_its_flag() {
        let template = "{{dps_growth_rate}}|{{#if has_dps_growth_rate}}known{{else}}unknown{{/if}}";
        assert_eq!(
            render_message(template, &GameSnapshot::default()).unwrap(),
            "0|unknown"
        );

        let snapshot = GameSnapshot {
            round_report: Some(crate::analysis::RoundReport {
                has_duration_data: true,
                has_output_data: true,
                duration_seconds: 60,
                combat_duration_seconds: 50,
                total_damage: 100,
                average_dps: 2.0,
                max_dps: 10,
                effective_dps: 4.0,
                burst_10s_dps: Some(5.0),
                dps_growth_rate: -12.5,
                has_dps_growth_rate: true,
                damage_taken: 3,
                has_longest_standstill_data: false,
                longest_standstill_seconds: 0,
            }),
            ..GameSnapshot::default()
        };
        assert_eq!(render_message(template, &snapshot).unwrap(), "-12.5|known");
    }

    #[test]
    fn entering_combat_replaces_a_published_report_when_live_template_is_empty() {
        let config = AppConfig::default();
        let report_snapshot = GameSnapshot {
            phase: RoundPhase::Lobby,
            round_report: Some(crate::analysis::RoundReport {
                has_duration_data: true,
                has_output_data: true,
                duration_seconds: 60,
                combat_duration_seconds: 50,
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
            combat_round_epoch: 4,
            ..GameSnapshot::default()
        };
        let mut published = PublishedChatboxState::default();
        let report_update = published.next_update(
            render_configured_message(&config, &report_snapshot).unwrap(),
            BroadcastContext::RoundReport(4),
            config.language,
        );
        assert!(matches!(report_update, ChatboxUpdate::Message(_)));
        published.complete(&report_update);

        let combat_snapshot = GameSnapshot {
            phase: RoundPhase::Combat,
            combat_round_epoch: 5,
            ..GameSnapshot::default()
        };
        let live_message = render_configured_message(&config, &combat_snapshot).unwrap();
        assert_eq!(live_message, "");
        let replacement = published.next_update(
            live_message.clone(),
            BroadcastContext::Combat(5),
            config.language,
        );
        assert_eq!(
            replacement,
            ChatboxUpdate::Message(
                crate::i18n::text::EMPTY_COMBAT_REPLACEMENT
                    .get(config.language)
                    .to_owned()
            )
        );
        published.complete(&replacement);

        // Keep the deterministic replacement present until actual combat data
        // makes the user's selected template non-empty.
        assert_eq!(
            published.next_update(live_message, BroadcastContext::Combat(5), config.language),
            ChatboxUpdate::Message(
                crate::i18n::text::EMPTY_COMBAT_REPLACEMENT
                    .get(config.language)
                    .to_owned()
            )
        );
    }

    #[test]
    fn empty_template_without_any_previously_published_text_sends_nothing() {
        let published = PublishedChatboxState::default();
        assert_eq!(
            published.next_update(
                String::new(),
                BroadcastContext::Combat(1),
                crate::i18n::Language::Chinese
            ),
            ChatboxUpdate::SkipEmpty
        );
    }

    #[test]
    fn empty_combat_template_replaces_report_with_non_empty_packet() {
        let receiver = UdpSocket::bind("127.0.0.1:0").unwrap();
        receiver
            .set_read_timeout(Some(Duration::from_secs(1)))
            .unwrap();
        let sender = UdpSocket::bind("127.0.0.1:0").unwrap();
        let replacement =
            crate::i18n::text::EMPTY_COMBAT_REPLACEMENT.get(crate::i18n::Language::Chinese);
        let update = ChatboxUpdate::Message(replacement.to_owned());

        assert_eq!(
            send_chatbox_update(
                &sender,
                &receiver.local_addr().unwrap().to_string(),
                &update,
            )
            .unwrap(),
            SendOutcome::Sent
        );

        let mut bytes = [0_u8; 256];
        let (length, _) = receiver.recv_from(&mut bytes).unwrap();
        let (_, packet) = rosc::decoder::decode_udp(&bytes[..length]).unwrap();
        let OscPacket::Message(message) = packet else {
            panic!("expected a Chatbox message");
        };
        assert_eq!(message.addr, "/chatbox/input");
        assert_eq!(
            message.args,
            vec![
                OscType::String(replacement.to_owned()),
                OscType::Bool(true),
                OscType::Bool(false),
            ]
        );
    }

    #[test]
    fn final_send_gate_skips_whitespace_only_messages() {
        let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
        assert_eq!(
            send_chatbox(&socket, "not an address", " \n\t ").unwrap(),
            SendOutcome::SkippedEmpty
        );
    }

    #[test]
    fn config_save_restarts_a_full_send_interval() {
        let start = Instant::now();
        let mut schedule = SendSchedule::new(start);
        assert!(schedule.is_due(start));
        schedule.complete_cycle(Duration::from_secs(3), start);

        let save_time = start + Duration::from_secs(2);
        schedule.observe_config(1, Duration::from_secs(1), save_time);
        assert!(!schedule.is_due(save_time));
        assert!(!schedule.is_due(save_time + Duration::from_millis(999)));
        assert!(schedule.is_due(save_time + Duration::from_secs(1)));
    }

    #[test]
    fn one_point_five_second_interval_is_not_rounded_to_whole_seconds() {
        let start = Instant::now();
        let mut schedule = SendSchedule::new(start);
        schedule.complete_cycle(crate::config::SendInterval::OnePointFive.duration(), start);

        assert!(!schedule.is_due(start + Duration::from_millis(1_499)));
        assert!(schedule.is_due(start + Duration::from_millis(1_500)));
    }

    #[test]
    fn wasd_condition_edges_replace_pending_work_immediately() {
        let start = Instant::now();
        let mut schedule = SendSchedule::new(start);
        schedule.complete_cycle(Duration::from_secs(3), start);
        schedule.observe_no_wasd_condition(true, start);
        assert!(!schedule.is_due(start + Duration::from_millis(999)));

        let key_press = start + Duration::from_millis(250);
        assert!(schedule.observe_no_wasd_condition(false, key_press));
        assert!(schedule.is_due(key_press));
    }

    #[test]
    fn phase_switch_is_due_immediately_for_the_reserved_slot() {
        let start = Instant::now();
        let mut schedule = SendSchedule::new(start);
        schedule.observe_broadcast_context(Some(BroadcastContext::RoundReport(4)), start);
        schedule.complete_cycle(Duration::from_secs(3), start);

        let round_start = start + Duration::from_millis(250);
        assert!(schedule.observe_broadcast_context(Some(BroadcastContext::Combat(5)), round_start));
        assert!(schedule.is_due(round_start));
    }

    #[test]
    fn context_switch_clears_the_queued_report_instead_of_sending_it_later() {
        let mut queue = SendQueue::default();
        let report = BroadcastContext::RoundReport(4);
        let combat = BroadcastContext::Combat(5);
        queue.enqueue(report, SendPriority::Regular);
        assert_eq!(queue.pending().unwrap().context, report);

        queue.clear();
        queue.enqueue(combat, SendPriority::StateChange);
        let pending = queue.pending().unwrap();
        assert_eq!(pending.context, combat);
        assert_eq!(pending.priority, SendPriority::StateChange);

        // A normal refresh must update/coalesce the request without
        // downgrading the transition priority.
        queue.enqueue(combat, SendPriority::Regular);
        assert_eq!(queue.pending().unwrap().priority, SendPriority::StateChange);
    }

    #[test]
    fn rate_limited_report_is_replaced_by_the_current_combat_message() {
        let start = Instant::now();
        let mut limiter = ChatboxRateLimiter::default();
        for offset in 0..4 {
            let now = start + Duration::from_secs(offset);
            assert!(limiter.can_send(SendPriority::Regular, now));
            limiter.record_send(now);
        }

        let report = BroadcastContext::RoundReport(4);
        let combat = BroadcastContext::Combat(5);
        let mut queue = SendQueue::default();
        queue.enqueue(report, SendPriority::Regular);
        let transition = start + Duration::from_secs(4);
        assert!(!limiter.can_send(queue.pending().unwrap().priority, transition));

        queue.clear();
        queue.enqueue(combat, SendPriority::StateChange);
        assert!(limiter.can_send(queue.pending().unwrap().priority, transition));

        let config = AppConfig {
            message_template: "COMBAT".to_owned(),
            round_report_template: "REPORT".to_owned(),
            ..AppConfig::default()
        };
        let snapshot = GameSnapshot {
            phase: RoundPhase::Combat,
            combat_round_epoch: 5,
            // Even a malformed transition snapshot retaining its archived
            // report must render the authoritative Combat template.
            round_report: Some(crate::analysis::RoundReport {
                has_duration_data: true,
                has_output_data: true,
                duration_seconds: 60,
                combat_duration_seconds: 50,
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
        assert_eq!(broadcast_context(&snapshot), Some(combat));
        assert_eq!(
            render_configured_message(&config, &snapshot).unwrap(),
            "COMBAT"
        );
    }

    #[test]
    fn queued_wasd_update_renders_the_latest_value_at_dispatch_time() {
        let config = AppConfig {
            message_template: "{{#if no_wasd_for_10s}}NO_WASD{{else}}WASD_ACTIVE{{/if}}".to_owned(),
            ..AppConfig::default()
        };
        let mut snapshot = GameSnapshot {
            phase: RoundPhase::Combat,
            combat_round_epoch: 5,
            wasd_listener_available: true,
            no_wasd_for_10s: true,
            ..GameSnapshot::default()
        };
        let mut queue = SendQueue::default();
        queue.enqueue(BroadcastContext::Combat(5), SendPriority::Regular);
        assert_eq!(
            render_configured_message(&config, &snapshot).unwrap(),
            "NO_WASD"
        );

        // The queue stores no rendered text. A key press updates the snapshot
        // before dispatch, so the pending request renders only the new value.
        snapshot.no_wasd_for_10s = false;
        queue.clear();
        queue.enqueue(BroadcastContext::Combat(5), SendPriority::StateChange);
        assert_eq!(
            render_configured_message(&config, &snapshot).unwrap(),
            "WASD_ACTIVE"
        );
        assert_eq!(queue.pending().unwrap().priority, SendPriority::StateChange);
    }

    #[test]
    fn wasd_activation_keeps_the_reserved_slot_for_clearing_idle_text() {
        assert_eq!(
            edge_priority(false, true, true),
            Some(SendPriority::Regular)
        );
        assert_eq!(
            edge_priority(false, true, false),
            Some(SendPriority::StateChange)
        );
        assert_eq!(
            edge_priority(true, false, false),
            Some(SendPriority::StateChange)
        );
    }

    #[test]
    fn local_rate_limiter_reserves_the_fifth_slot_for_state_changes() {
        let start = Instant::now();
        let mut limiter = ChatboxRateLimiter::default();
        for offset in 0..4 {
            let now = start + Duration::from_secs(offset);
            assert!(limiter.can_send(SendPriority::Regular, now));
            limiter.record_send(now);
        }

        let transition = start + Duration::from_secs(4);
        assert!(!limiter.can_send(SendPriority::Regular, transition));
        assert!(limiter.can_send(SendPriority::StateChange, transition));
        limiter.record_send(transition);
        assert!(!limiter.can_send(SendPriority::StateChange, transition));

        // Once the oldest packet leaves the five-second window, another
        // transition can use the newly available hard-limit slot.
        assert!(limiter.can_send(SendPriority::StateChange, start + CHATBOX_RATE_WINDOW));
    }

    #[test]
    fn unchanged_combat_context_keeps_the_normal_send_interval() {
        let start = Instant::now();
        let mut schedule = SendSchedule::new(start);
        schedule.observe_broadcast_context(Some(BroadcastContext::Combat(5)), start);
        schedule.complete_cycle(Duration::from_secs(3), start);

        let update = start + Duration::from_millis(250);
        schedule.observe_broadcast_context(Some(BroadcastContext::Combat(5)), update);
        assert!(!schedule.is_due(update));
        assert!(schedule.is_due(start + Duration::from_secs(3)));
    }

    #[test]
    fn configured_message_switches_to_round_report_template() {
        let config = AppConfig {
            message_template: "NORMAL".to_owned(),
            round_report_template: "REPORT {{round_total_damage}}".to_owned(),
            ..AppConfig::default()
        };
        let mut snapshot = GameSnapshot::default();
        assert_eq!(
            render_configured_message(&config, &snapshot).unwrap(),
            "NORMAL"
        );

        snapshot.round_report = Some(crate::analysis::RoundReport {
            has_duration_data: true,
            has_output_data: true,
            duration_seconds: 125,
            combat_duration_seconds: 100,
            total_damage: 12_345,
            average_dps: 123.45,
            max_dps: 999,
            effective_dps: 234.5,
            burst_10s_dps: Some(300.0),
            dps_growth_rate: 0.0,
            has_dps_growth_rate: false,
            damage_taken: 12,
            has_longest_standstill_data: true,
            longest_standstill_seconds: 42,
        });
        assert_eq!(
            render_configured_message(&config, &snapshot).unwrap(),
            "REPORT 12345"
        );

        // Combat is the authoritative live state. Even if a malformed or
        // concurrently transitioning snapshot retained an archived report,
        // it must never make OSC fall back to the previous round template.
        snapshot.phase = RoundPhase::Combat;
        assert_eq!(
            render_configured_message(&config, &snapshot).unwrap(),
            "NORMAL"
        );
    }

    #[test]
    fn osc_waits_for_room_phase_and_does_not_broadcast_an_empty_lobby() {
        let mut snapshot = GameSnapshot {
            in_ecliptica: true,
            phase: RoundPhase::Syncing,
            ..GameSnapshot::default()
        };
        assert!(!broadcast_context_ready(&snapshot));

        snapshot.phase = RoundPhase::Lobby;
        assert!(!broadcast_context_ready(&snapshot));

        snapshot.phase = RoundPhase::Combat;
        assert!(broadcast_context_ready(&snapshot));

        snapshot.phase = RoundPhase::Lobby;
        snapshot.round_report = Some(crate::analysis::RoundReport {
            has_duration_data: true,
            has_output_data: true,
            duration_seconds: 60,
            combat_duration_seconds: 50,
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
        });
        assert!(broadcast_context_ready(&snapshot));
    }
}
