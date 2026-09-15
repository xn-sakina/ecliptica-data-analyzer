use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};

use handlebars::{
    Context, Handlebars, Helper, HelperDef, JsonValue, RenderContext, RenderError,
    RenderErrorReason, ScopedJson,
};
use parking_lot::Mutex;

#[derive(Debug, Clone)]
pub(crate) enum RandomMode {
    Secure,
    Stage(StageRandomState),
    First,
}

#[derive(Debug, Clone)]
pub(crate) enum CooldownMode {
    Stateless,
    Live {
        state: TemplateRuntimeState,
        now: Instant,
    },
}

#[derive(Debug, Clone, Default)]
pub(crate) struct TemplateRuntimeState {
    inner: Arc<Mutex<TemplateRuntimeStateInner>>,
}

#[derive(Debug, Default)]
struct TemplateRuntimeStateInner {
    cooldowns: HashMap<String, CooldownState>,
    traffic_light: Option<TrafficLightState>,
    revision: u64,
}

#[derive(Debug)]
struct TrafficLightState {
    started_at: Instant,
    period: Duration,
    is_green: bool,
}

#[derive(Debug)]
struct CooldownState {
    activated_at: Option<Instant>,
    last_trigger: bool,
    last_ready: bool,
}

impl Default for CooldownState {
    fn default() -> Self {
        Self {
            activated_at: None,
            last_trigger: false,
            last_ready: true,
        }
    }
}

impl TemplateRuntimeState {
    fn cooldown_ready(&self, id: &str, duration: Duration, trigger: bool, now: Instant) -> bool {
        let mut inner = self.inner.lock();
        let state = inner.cooldowns.entry(id.to_owned()).or_default();
        let mut ready = state
            .activated_at
            .is_none_or(|activated| now.saturating_duration_since(activated) >= duration);
        if ready {
            state.activated_at = None;
        }

        // Treat the condition as an event edge. A displayed DPS value is held
        // briefly, so a level-triggered timer would restart several times for
        // one damage spike.
        if ready && trigger && !state.last_trigger {
            state.activated_at = Some(now);
            ready = false;
        }
        state.last_trigger = trigger;

        if ready != state.last_ready {
            state.last_ready = ready;
            inner.revision = inner.revision.wrapping_add(1);
        }
        ready
    }

    fn traffic_light(&self, period: Duration, now: Instant) -> &'static str {
        let mut inner = self.inner.lock();
        let state = inner
            .traffic_light
            .get_or_insert_with(|| TrafficLightState {
                started_at: now,
                period,
                is_green: false,
            });

        // Changing the template argument starts a fresh red-light cycle. This
        // keeps the experimental helper self-contained and predictable while
        // the user tunes its period.
        if state.period != period {
            state.started_at = now;
            state.period = period;
        }

        let completed_periods =
            now.saturating_duration_since(state.started_at).as_nanos() / period.as_nanos();
        let is_green = completed_periods % 2 == 1;
        if is_green != state.is_green {
            state.is_green = is_green;
            inner.revision = inner.revision.wrapping_add(1);
        }

        if is_green { "green" } else { "red" }
    }

    pub(crate) fn revision(&self) -> u64 {
        self.inner.lock().revision
    }

    pub(crate) fn clear(&self) {
        let mut inner = self.inner.lock();
        if !inner.cooldowns.is_empty() || inner.traffic_light.is_some() {
            inner.cooldowns.clear();
            inner.traffic_light = None;
            inner.revision = inner.revision.wrapping_add(1);
        }
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct StageRandomState {
    choices: Arc<Mutex<HashMap<Vec<String>, usize>>>,
}

impl StageRandomState {
    fn choose(&self, choices: &[&str]) -> Result<usize, RenderError> {
        let key = choices
            .iter()
            .map(|choice| (*choice).to_owned())
            .collect::<Vec<_>>();
        let mut cached = self.choices.lock();
        if let Some(index) = cached.get(&key) {
            return Ok(*index);
        }

        let index = secure_random_index(choices.len())?;
        cached.insert(key, index);
        Ok(index)
    }

    #[cfg(test)]
    pub(crate) fn cached_choice_count(&self) -> usize {
        self.choices.lock().len()
    }
}

pub(crate) fn engine(random_mode: RandomMode) -> Handlebars<'static> {
    engine_with_cooldowns(random_mode, CooldownMode::Stateless)
}

pub(crate) fn engine_with_cooldowns(
    random_mode: RandomMode,
    cooldown_mode: CooldownMode,
) -> Handlebars<'static> {
    let mut handlebars = Handlebars::new();
    handlebars.set_strict_mode(true);
    // Templates are rendered as plain text for VRChat's OSC chatbox, not as
    // HTML. Preserve characters such as `&`, `<`, quotes, and `=` in dynamic
    // values instead of emitting HTML entities like `&amp;`.
    handlebars.register_escape_fn(handlebars::no_escape);
    handlebars.register_helper("random", Box::new(RandomHelper { random_mode }));
    handlebars.register_helper(
        "cooldown_ready",
        Box::new(CooldownReadyHelper {
            cooldown_mode: cooldown_mode.clone(),
        }),
    );
    handlebars.register_helper(
        "traffic_light",
        Box::new(TrafficLightHelper {
            runtime_mode: cooldown_mode,
        }),
    );
    handlebars
}

#[derive(Debug, Clone)]
struct TrafficLightHelper {
    runtime_mode: CooldownMode,
}

impl HelperDef for TrafficLightHelper {
    fn call_inner<'reg: 'rc, 'rc>(
        &self,
        helper: &Helper<'rc>,
        _: &'reg Handlebars<'reg>,
        _: &'rc Context,
        _: &mut RenderContext<'reg, 'rc>,
    ) -> Result<ScopedJson<'rc>, RenderError> {
        if helper.params().len() > 1 {
            return Err(RenderErrorReason::Other(
                "traffic_light accepts at most one period in seconds".to_owned(),
            )
            .into());
        }
        let seconds = match helper.param(0) {
            Some(parameter) => parameter.value().as_f64().ok_or_else(|| {
                RenderError::from(RenderErrorReason::Other(
                    "traffic_light seconds must be a number".to_owned(),
                ))
            })?,
            None => 20.0,
        };
        if !seconds.is_finite() || !(10.0..=20.0).contains(&seconds) {
            return Err(RenderErrorReason::Other(
                "traffic_light seconds must be between 10 and 20".to_owned(),
            )
            .into());
        }

        let color = match &self.runtime_mode {
            CooldownMode::Stateless => "red",
            CooldownMode::Live { state, now } => {
                state.traffic_light(Duration::from_secs_f64(seconds), *now)
            }
        };
        Ok(ScopedJson::Derived(JsonValue::String(color.to_owned())))
    }
}

#[derive(Debug, Clone)]
struct CooldownReadyHelper {
    cooldown_mode: CooldownMode,
}

impl HelperDef for CooldownReadyHelper {
    fn call_inner<'reg: 'rc, 'rc>(
        &self,
        helper: &Helper<'rc>,
        _: &'reg Handlebars<'reg>,
        _: &'rc Context,
        _: &mut RenderContext<'reg, 'rc>,
    ) -> Result<ScopedJson<'rc>, RenderError> {
        if helper.params().len() != 3 {
            return Err(RenderErrorReason::Other(
                "cooldown_ready requires an id, cooldown seconds, and a trigger condition"
                    .to_owned(),
            )
            .into());
        }
        let id = helper.param(0).unwrap().value().as_str().ok_or_else(|| {
            RenderError::from(RenderErrorReason::Other(
                "cooldown_ready id must be a string".to_owned(),
            ))
        })?;
        if id.trim().is_empty() {
            return Err(
                RenderErrorReason::Other("cooldown_ready id cannot be empty".to_owned()).into(),
            );
        }
        let seconds = helper.param(1).unwrap().value().as_f64().ok_or_else(|| {
            RenderError::from(RenderErrorReason::Other(
                "cooldown_ready seconds must be a number".to_owned(),
            ))
        })?;
        if !seconds.is_finite() || seconds <= 0.0 || seconds > 86_400.0 {
            return Err(RenderErrorReason::Other(
                "cooldown_ready seconds must be greater than 0 and at most 86400".to_owned(),
            )
            .into());
        }
        let trigger = helper.param(2).unwrap().value().as_bool().ok_or_else(|| {
            RenderError::from(RenderErrorReason::Other(
                "cooldown_ready trigger must be a boolean".to_owned(),
            ))
        })?;
        let duration = Duration::from_secs_f64(seconds);
        let ready = match &self.cooldown_mode {
            CooldownMode::Stateless => !trigger,
            CooldownMode::Live { state, now } => state.cooldown_ready(id, duration, trigger, *now),
        };
        Ok(ScopedJson::Derived(JsonValue::Bool(ready)))
    }
}

#[derive(Debug, Clone)]
struct RandomHelper {
    random_mode: RandomMode,
}

impl HelperDef for RandomHelper {
    fn call_inner<'reg: 'rc, 'rc>(
        &self,
        helper: &Helper<'rc>,
        _: &'reg Handlebars<'reg>,
        _: &'rc Context,
        _: &mut RenderContext<'reg, 'rc>,
    ) -> Result<ScopedJson<'rc>, RenderError> {
        let choices = helper
            .params()
            .iter()
            .map(|parameter| {
                parameter.value().as_str().ok_or_else(|| {
                    RenderError::from(RenderErrorReason::Other(
                        "random only accepts string arguments".to_owned(),
                    ))
                })
            })
            .collect::<Result<Vec<_>, _>>()?;

        if choices.is_empty() {
            return Err(RenderErrorReason::Other(
                "random requires at least one string argument".to_owned(),
            )
            .into());
        }

        let index = match &self.random_mode {
            RandomMode::Secure => secure_random_index(choices.len())?,
            RandomMode::Stage(state) => state.choose(&choices)?,
            RandomMode::First => 0,
        };
        Ok(ScopedJson::Derived(JsonValue::String(
            choices[index].to_owned(),
        )))
    }
}

fn secure_random_index(upper_bound: usize) -> Result<usize, RenderError> {
    let upper_bound = u64::try_from(upper_bound).map_err(|_| {
        RenderError::from(RenderErrorReason::Other(
            "random received too many arguments".to_owned(),
        ))
    })?;
    if upper_bound == 1 {
        return Ok(0);
    }

    // Rejection sampling keeps every candidate exactly equally likely instead
    // of introducing modulo bias. getrandom reads from the operating system's
    // cryptographically secure random source.
    let unbiased_range = u64::MAX - u64::MAX % upper_bound;
    loop {
        let value = getrandom::u64().map_err(|error| {
            RenderError::from(RenderErrorReason::Other(format!(
                "system random source failed: {error}"
            )))
        })?;
        if value < unbiased_range {
            return Ok((value % upper_bound) as usize);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn random_helper_requires_at_least_one_string() {
        assert!(
            engine(RandomMode::First)
                .render_template("{{random}}", &json!({}))
                .is_err()
        );
        assert!(
            engine(RandomMode::First)
                .render_template("{{random 1 \"two\"}}", &json!({}))
                .is_err()
        );
    }

    #[test]
    fn deterministic_mode_uses_first_choice_for_stable_previews() {
        assert_eq!(
            engine(RandomMode::First)
                .render_template("{{random \"one\" \"two\"}}", &json!({}))
                .unwrap(),
            "one"
        );
    }

    #[test]
    fn variables_are_rendered_as_plain_text_without_html_escaping() {
        let value = "A & B <mix> \"live\" 'edit' = `remix`";
        assert_eq!(
            engine(RandomMode::First)
                .render_template("{{value}}", &json!({ "value": value }))
                .unwrap(),
            value
        );
    }

    #[test]
    fn secure_mode_only_returns_supplied_choices() {
        for _ in 0..32 {
            let rendered = engine(RandomMode::Secure)
                .render_template("{{random \"one\" \"two\" \"three\"}}", &json!({}))
                .unwrap();
            assert!(["one", "two", "three"].contains(&rendered.as_str()));
        }
    }

    #[test]
    fn stage_mode_keeps_each_choice_stable_across_renders() {
        let state = StageRandomState::default();
        let template = "{{random \"one\" \"two\"}}|{{random \"red\" \"blue\"}}";
        let first = engine(RandomMode::Stage(state.clone()))
            .render_template(template, &json!({}))
            .unwrap();

        for _ in 0..32 {
            assert_eq!(
                engine(RandomMode::Stage(state.clone()))
                    .render_template(template, &json!({}))
                    .unwrap(),
                first
            );
        }
        assert_eq!(state.cached_choice_count(), 2);
    }

    fn render_cooldown(state: &TemplateRuntimeState, now: Instant, trigger: bool) -> String {
        engine_with_cooldowns(
            RandomMode::First,
            CooldownMode::Live {
                state: state.clone(),
                now,
            },
        )
        .render_template(
            "{{#if (cooldown_ready \"ability\" 10 trigger)}}READY{{else}}COOLING{{/if}}",
            &json!({ "trigger": trigger }),
        )
        .unwrap()
    }

    #[test]
    fn cooldown_starts_on_a_rising_edge_and_becomes_ready_at_its_deadline() {
        let state = TemplateRuntimeState::default();
        let start = Instant::now();

        assert_eq!(render_cooldown(&state, start, false), "READY");
        assert_eq!(render_cooldown(&state, start, true), "COOLING");
        assert_eq!(
            render_cooldown(&state, start + Duration::from_secs(9), true),
            "COOLING"
        );
        assert_eq!(
            render_cooldown(&state, start + Duration::from_secs(10), true),
            "READY"
        );

        // A held high condition is the same event, not a second activation.
        assert_eq!(
            render_cooldown(&state, start + Duration::from_secs(11), true),
            "READY"
        );
        assert_eq!(
            render_cooldown(&state, start + Duration::from_secs(11), false),
            "READY"
        );
        assert_eq!(
            render_cooldown(&state, start + Duration::from_secs(12), true),
            "COOLING"
        );
    }

    #[test]
    fn cooldown_ids_are_independent_and_stateless_rendering_is_repeatable() {
        let template = "{{cooldown_ready \"a\" 10 trigger}}|{{cooldown_ready \"b\" 10 false}}";
        let values = json!({ "trigger": true });
        let state = TemplateRuntimeState::default();
        let now = Instant::now();
        let live = engine_with_cooldowns(
            RandomMode::First,
            CooldownMode::Live {
                state: state.clone(),
                now,
            },
        )
        .render_template(template, &values)
        .unwrap();
        assert_eq!(live, "false|true");
        assert_eq!(state.revision(), 1);

        for _ in 0..2 {
            assert_eq!(
                engine(RandomMode::First)
                    .render_template(template, &values)
                    .unwrap(),
                "false|true"
            );
        }
    }

    #[test]
    fn cooldown_rejects_invalid_arguments() {
        for template in [
            "{{cooldown_ready}}",
            "{{cooldown_ready \"\" 10 false}}",
            "{{cooldown_ready \"x\" 0 false}}",
            "{{cooldown_ready \"x\" \"ten\" false}}",
            "{{cooldown_ready \"x\" 10 \"false\"}}",
        ] {
            assert!(
                engine(RandomMode::First)
                    .render_template(template, &json!({}))
                    .is_err(),
                "{template} should fail"
            );
        }
    }

    fn render_traffic_light(
        state: &TemplateRuntimeState,
        now: Instant,
        seconds: Option<f64>,
    ) -> String {
        let template = seconds.map_or_else(
            || "{{traffic_light}}".to_owned(),
            |seconds| format!("{{{{traffic_light {seconds}}}}}"),
        );
        engine_with_cooldowns(
            RandomMode::First,
            CooldownMode::Live {
                state: state.clone(),
                now,
            },
        )
        .render_template(&template, &json!({}))
        .unwrap()
    }

    #[test]
    fn traffic_light_starts_red_and_alternates_each_period() {
        let state = TemplateRuntimeState::default();
        let start = Instant::now();

        assert_eq!(render_traffic_light(&state, start, Some(10.0)), "red");
        assert_eq!(
            render_traffic_light(&state, start + Duration::from_secs(9), Some(10.0)),
            "red"
        );
        assert_eq!(
            render_traffic_light(&state, start + Duration::from_secs(10), Some(10.0)),
            "green"
        );
        assert_eq!(
            render_traffic_light(&state, start + Duration::from_secs(20), Some(10.0)),
            "red"
        );
        assert_eq!(
            render_traffic_light(&state, start + Duration::from_secs(30), Some(10.0)),
            "green"
        );
        assert_eq!(state.revision(), 3);
    }

    #[test]
    fn traffic_light_defaults_to_twenty_seconds_and_preview_is_red() {
        let state = TemplateRuntimeState::default();
        let start = Instant::now();

        assert_eq!(render_traffic_light(&state, start, None), "red");
        assert_eq!(
            render_traffic_light(&state, start + Duration::from_secs(19), None),
            "red"
        );
        assert_eq!(
            render_traffic_light(&state, start + Duration::from_secs(20), None),
            "green"
        );
        assert_eq!(
            engine(RandomMode::First)
                .render_template("{{traffic_light}}", &json!({}))
                .unwrap(),
            "red"
        );
        assert_eq!(
            engine(RandomMode::First)
                .render_template(
                    "{{#if (eq (traffic_light) \"green\")}}GO{{else}}STOP{{/if}}",
                    &json!({}),
                )
                .unwrap(),
            "STOP"
        );
    }

    #[test]
    fn traffic_light_rejects_invalid_arguments() {
        for template in [
            "{{traffic_light 9}}",
            "{{traffic_light 21}}",
            "{{traffic_light \"twenty\"}}",
            "{{traffic_light 10 20}}",
        ] {
            assert!(
                engine(RandomMode::First)
                    .render_template(template, &json!({}))
                    .is_err(),
                "{template} should fail"
            );
        }
    }
}
