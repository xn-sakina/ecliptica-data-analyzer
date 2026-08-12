//! ScrollArea builder struct — a themed scrollable container.

/// A styled scroll area wrapping egui's built-in ScrollArea.
#[must_use]
pub struct ScrollArea {
    pub(crate) max_height: f32,
    pub(crate) horizontal: bool,
    pub(crate) id_salt: Option<egui::Id>,
    pub(crate) framed: bool,
    pub(crate) stick_to_bottom: bool,
    pub(crate) auto_shrink: [bool; 2],
    pub(crate) fill_available: bool,
}

impl ScrollArea {
    pub fn new(max_height: f32) -> Self {
        Self {
            max_height,
            horizontal: false,
            id_salt: None,
            framed: true,
            stick_to_bottom: false,
            auto_shrink: [true, true],
            fill_available: false,
        }
    }

    pub fn horizontal(mut self) -> Self {
        self.horizontal = true;
        self
    }

    pub fn id_salt(mut self, id_salt: impl std::hash::Hash) -> Self {
        self.id_salt = Some(egui::Id::new(id_salt));
        self
    }

    pub fn framed(mut self, framed: bool) -> Self {
        self.framed = framed;
        self
    }

    pub fn stick_to_bottom(mut self, stick_to_bottom: bool) -> Self {
        self.stick_to_bottom = stick_to_bottom;
        self
    }

    pub fn auto_shrink(mut self, auto_shrink: [bool; 2]) -> Self {
        self.auto_shrink = auto_shrink;
        self
    }

    pub fn fill_available(mut self, fill_available: bool) -> Self {
        self.fill_available = fill_available;
        self
    }
}
