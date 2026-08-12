//! PropertyRow builder struct.

/// A single label/control row for an inspector or settings panel.
#[must_use]
pub struct PropertyRow {
    pub(crate) label: String,
    pub(crate) label_width: Option<f32>,
    pub(crate) align_start: bool,
}

impl PropertyRow {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            label_width: None,
            align_start: false,
        }
    }

    pub fn label_width(mut self, width: f32) -> Self {
        self.label_width = Some(width);
        self
    }

    /// Align the label with the first line of multiline content.
    pub fn align_start(mut self) -> Self {
        self.align_start = true;
        self
    }
}
