//! Dialog builder struct — a modal overlay panel.

/// A modal dialog: centered panel with backdrop overlay.
#[must_use]
pub struct Dialog {
    pub(crate) title: Option<String>,
    pub(crate) description: Option<String>,
    pub(crate) width: f32,
    pub(crate) close_on_backdrop: bool,
    pub(crate) close_on_escape: bool,
    pub(crate) close_label: String,
}

impl Dialog {
    pub fn new() -> Self {
        Self {
            title: None,
            description: None,
            width: 420.0,
            close_on_backdrop: true,
            close_on_escape: true,
            close_label: "Close dialog".to_owned(),
        }
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    pub fn close_on_backdrop(mut self, close: bool) -> Self {
        self.close_on_backdrop = close;
        self
    }

    pub fn close_on_escape(mut self, close: bool) -> Self {
        self.close_on_escape = close;
        self
    }

    pub fn close_label(mut self, label: impl Into<String>) -> Self {
        self.close_label = label.into();
        self
    }
}
