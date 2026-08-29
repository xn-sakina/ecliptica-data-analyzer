//! Label builder struct — styled text following shadcn/ui's `text-sm font-medium`.

/// A styled text label: `text-sm font-medium`.
#[must_use]
pub struct Label {
    pub(crate) text: String,
    pub(crate) muted: bool,
    pub(crate) size: Option<crate::tokens::component_size::ComponentSize>,
    pub(crate) truncate: bool,
}

impl Label {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            muted: false,
            size: None,
            truncate: false,
        }
    }

    /// Use muted foreground color.
    pub fn muted(mut self) -> Self {
        self.muted = true;
        self
    }

    /// Match a component size for consistent baseline alignment with buttons.
    /// Sets both the font size and allocated height to match the given size.
    pub fn size(mut self, size: crate::tokens::component_size::ComponentSize) -> Self {
        self.size = Some(size);
        self
    }

    /// Keep the label inside the UI's available width and elide overflowing text.
    pub fn truncate(mut self) -> Self {
        self.truncate = true;
        self
    }

    pub fn show(self, ui: &mut egui::Ui) -> egui::Response {
        ui.add(self)
    }
}
