//! Textarea builder struct — a multi-line text input styled after shadcn/ui.

/// A multi-line text area: `border-input rounded-lg px-2.5 py-2 min-h-16`.
#[must_use]
pub struct Textarea<'a> {
    pub(crate) text: &'a mut String,
    pub(crate) placeholder: String,
    pub(crate) desired_width: Option<f32>,
    pub(crate) min_height: f32,
    pub(crate) max_height: Option<f32>,
    pub(crate) auto_resize: bool,
    pub(crate) id_salt: Option<egui::Id>,
    pub(crate) monospace: bool,
}

impl<'a> Textarea<'a> {
    pub fn new(text: &'a mut String) -> Self {
        Self {
            text,
            placeholder: String::new(),
            desired_width: None,
            min_height: 64.0, // min-h-16 = 4rem = 64px
            max_height: None,
            auto_resize: false,
            id_salt: None,
            monospace: false,
        }
    }

    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    pub fn desired_width(mut self, width: f32) -> Self {
        self.desired_width = Some(width);
        self
    }

    pub fn min_height(mut self, height: f32) -> Self {
        self.min_height = height;
        self
    }

    /// Grow with the laid-out text until `max_height` is reached, then scroll.
    pub fn auto_resize(mut self) -> Self {
        self.auto_resize = true;
        self
    }

    /// Limit auto-resizing to this height before vertical scrolling takes over.
    pub fn max_height(mut self, height: f32) -> Self {
        self.max_height = Some(height);
        self
    }

    /// Use a fixed-width font and keep Tab inside the editor for code/template editing.
    pub fn monospace(mut self) -> Self {
        self.monospace = true;
        self
    }

    /// Gives the internal scroll area a stable, caller-controlled identity.
    /// Required when multiple textareas can occupy equivalent nested layouts.
    pub fn id_salt(mut self, id_salt: impl std::hash::Hash) -> Self {
        self.id_salt = Some(egui::Id::new(id_salt));
        self
    }

    pub fn show(self, ui: &mut egui::Ui) -> egui::Response {
        ui.add(self)
    }
}
