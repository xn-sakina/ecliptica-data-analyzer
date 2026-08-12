//! ToggleGroup builder struct — a set of exclusive toggle buttons.

/// A group of toggle buttons: `inline-flex gap-0.5 rounded-lg bg-muted p-0.5`.
#[must_use]
pub struct ToggleGroup {
    pub(crate) items: Vec<String>,
    pub(crate) variant: crate::tokens::toggle_variant::ToggleVariant,
    pub(crate) size: crate::tokens::component_size::ComponentSize,
    pub(crate) applied_index: Option<usize>,
    pub(crate) draft_changed: bool,
    pub(crate) selection_markers: bool,
}

impl ToggleGroup {
    pub fn new(items: Vec<String>) -> Self {
        Self {
            items,
            variant: crate::tokens::toggle_variant::ToggleVariant::Default,
            size: crate::tokens::component_size::ComponentSize::Default,
            applied_index: None,
            draft_changed: false,
            selection_markers: true,
        }
    }

    pub fn variant(mut self, variant: crate::tokens::toggle_variant::ToggleVariant) -> Self {
        self.variant = variant;
        self
    }

    pub fn size(mut self, size: crate::tokens::component_size::ComponentSize) -> Self {
        self.size = size;
        self
    }

    /// Marks the item that is currently applied outside the editor.
    pub fn applied_index(mut self, index: usize) -> Self {
        self.applied_index = Some(index);
        self
    }

    /// Marks the selected item as an unsaved draft.
    pub fn draft_changed(mut self, changed: bool) -> Self {
        self.draft_changed = changed;
        self
    }

    /// Controls the optional circle glyphs rendered before outline labels.
    pub fn selection_markers(mut self, visible: bool) -> Self {
        self.selection_markers = visible;
        self
    }
}
