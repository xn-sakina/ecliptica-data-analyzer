use std::{collections::HashMap, sync::Arc};

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
    let mut handlebars = Handlebars::new();
    handlebars.set_strict_mode(true);
    handlebars.register_helper("random", Box::new(RandomHelper { random_mode }));
    handlebars
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
}
