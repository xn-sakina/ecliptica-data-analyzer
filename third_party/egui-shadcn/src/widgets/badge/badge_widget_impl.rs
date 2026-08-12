//! Widget trait implementation for Badge.

impl egui::Widget for super::badge::Badge {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let theme = crate::theme::shadcn_theme_ext::ShadcnThemeExt::shadcn_theme(ui.ctx());
        let style = super::badge_variant_style::resolve_badge_style(&theme, self.variant);

        let badge_text = self.text.clone();
        let galley =
            ui.painter()
                .layout_no_wrap(self.text, egui::FontId::proportional(13.0), style.fg);

        let h_padding: f32 = 8.0; // px-2
        let height: f32 = 22.0;
        let desired = egui::vec2(galley.size().x + h_padding * 2.0, height);
        let (rect, response) = ui.allocate_exact_size(desired, egui::Sense::hover());
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Label, ui.is_enabled(), badge_text.clone())
        });

        if ui.is_rect_visible(rect) {
            let painter = ui.painter();
            let cr = egui::CornerRadius::same(255); // rounded-full

            painter.rect_filled(rect, cr, style.bg);

            if let Some(border_color) = style.border {
                painter.rect_stroke(
                    rect,
                    cr,
                    egui::Stroke::new(1.0, border_color),
                    egui::epaint::StrokeKind::Inside,
                );
            }

            let text_pos = egui::pos2(
                rect.center().x - galley.size().x / 2.0,
                rect.center().y - galley.size().y / 2.0,
            );
            painter.galley(text_pos, galley, style.fg);
        }

        response
    }
}
