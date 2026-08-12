//! Widget trait implementation for Typography.

impl egui::Widget for super::typography::Typography {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let theme = crate::theme::shadcn_theme_ext::ShadcnThemeExt::shadcn_theme(ui.ctx());
        let (variant_font_size, _line_height, variant_is_bold) = self.variant.metrics();
        let font_size = self.font_size.unwrap_or(variant_font_size);
        let is_bold = self.strong.unwrap_or(variant_is_bold);

        let color = self.color.unwrap_or_else(|| match self.variant {
            crate::tokens::typography_variant::TypographyVariant::Muted => theme.muted_foreground,
            crate::tokens::typography_variant::TypographyVariant::Lead => theme.muted_foreground,
            _ => theme.foreground,
        });

        if let Some(line_height) = self.line_height {
            let mut job = egui::text::LayoutJob::default();
            job.append(
                &self.text,
                0.0,
                egui::TextFormat {
                    font_id: if self.monospace {
                        egui::FontId::monospace(font_size)
                    } else {
                        egui::FontId::proportional(font_size)
                    },
                    color,
                    italics: self.italics,
                    line_height: Some(line_height),
                    ..Default::default()
                },
            );
            let mut label = egui::Label::new(job);
            if self.wrap {
                label = label.wrap();
            }
            if self.truncate {
                label = label.truncate();
            }
            return ui.add(label);
        }

        let mut rich_text = egui::RichText::new(self.text).color(color).size(font_size);
        if is_bold {
            rich_text = rich_text.strong();
        }
        if self.monospace {
            rich_text = rich_text.monospace();
        }
        if self.italics {
            rich_text = rich_text.italics();
        }

        let mut label = egui::Label::new(rich_text);
        if self.wrap {
            label = label.wrap();
        }
        if self.truncate {
            label = label.truncate();
        }
        ui.add(label)
    }
}
