//! Typography builder struct — styled text display.

/// Styled text matching shadcn/ui typography variants.
#[must_use]
pub struct Typography {
    pub(crate) text: String,
    pub(crate) variant: crate::tokens::typography_variant::TypographyVariant,
    pub(crate) color: Option<egui::Color32>,
    pub(crate) monospace: bool,
    pub(crate) italics: bool,
    pub(crate) wrap: bool,
    pub(crate) truncate: bool,
    pub(crate) font_size: Option<f32>,
    pub(crate) line_height: Option<f32>,
    pub(crate) strong: Option<bool>,
}

impl Typography {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            variant: crate::tokens::typography_variant::TypographyVariant::P,
            color: None,
            monospace: false,
            italics: false,
            wrap: false,
            truncate: false,
            font_size: None,
            line_height: None,
            strong: None,
        }
    }

    pub fn variant(
        mut self,
        variant: crate::tokens::typography_variant::TypographyVariant,
    ) -> Self {
        self.variant = variant;
        self
    }

    pub fn h1(text: impl Into<String>) -> Self {
        Self::new(text).variant(crate::tokens::typography_variant::TypographyVariant::H1)
    }

    pub fn h2(text: impl Into<String>) -> Self {
        Self::new(text).variant(crate::tokens::typography_variant::TypographyVariant::H2)
    }

    pub fn h3(text: impl Into<String>) -> Self {
        Self::new(text).variant(crate::tokens::typography_variant::TypographyVariant::H3)
    }

    pub fn h4(text: impl Into<String>) -> Self {
        Self::new(text).variant(crate::tokens::typography_variant::TypographyVariant::H4)
    }

    pub fn lead(text: impl Into<String>) -> Self {
        Self::new(text).variant(crate::tokens::typography_variant::TypographyVariant::Lead)
    }

    pub fn muted(text: impl Into<String>) -> Self {
        Self::new(text).variant(crate::tokens::typography_variant::TypographyVariant::Muted)
    }

    pub fn small(text: impl Into<String>) -> Self {
        Self::new(text).variant(crate::tokens::typography_variant::TypographyVariant::Small)
    }

    pub fn color(mut self, color: egui::Color32) -> Self {
        self.color = Some(color);
        self
    }

    pub fn monospace(mut self) -> Self {
        self.monospace = true;
        self
    }

    pub fn italics(mut self) -> Self {
        self.italics = true;
        self
    }

    pub fn wrap(mut self) -> Self {
        self.wrap = true;
        self
    }

    pub fn truncate(mut self) -> Self {
        self.truncate = true;
        self
    }

    pub fn font_size(mut self, font_size: f32) -> Self {
        self.font_size = Some(font_size);
        self
    }

    /// Sets explicit leading for multiline text, matching CSS line-height.
    pub fn line_height(mut self, line_height: f32) -> Self {
        self.line_height = Some(line_height);
        self
    }

    pub fn strong(mut self) -> Self {
        self.strong = Some(true);
        self
    }

    pub fn show(self, ui: &mut egui::Ui) -> egui::Response {
        ui.add(self)
    }
}
