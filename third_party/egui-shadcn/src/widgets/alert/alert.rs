//! Alert builder struct — a status message container.

/// An alert container: `rounded-lg border px-4 py-3 text-sm`.
#[must_use]
pub struct Alert {
    pub(crate) title: Option<String>,
    pub(crate) variant: crate::tokens::alert_variant::AlertVariant,
    pub(crate) full_width: bool,
}

impl Alert {
    pub fn new() -> Self {
        Self {
            title: None,
            variant: crate::tokens::alert_variant::AlertVariant::Default,
            full_width: false,
        }
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn variant(mut self, variant: crate::tokens::alert_variant::AlertVariant) -> Self {
        self.variant = variant;
        self
    }

    pub fn full_width(mut self) -> Self {
        self.full_width = true;
        self
    }
}
