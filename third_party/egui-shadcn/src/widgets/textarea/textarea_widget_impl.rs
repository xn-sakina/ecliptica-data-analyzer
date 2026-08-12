//! Widget trait implementation for Textarea.

impl egui::Widget for super::textarea::Textarea<'_> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let theme = crate::theme::shadcn_theme_ext::ShadcnThemeExt::shadcn_theme(ui.ctx());

        let h_padding: f32 = 10.0; // px-2.5
        let v_padding: f32 = 8.0; // py-2
        let width = self
            .desired_width
            .unwrap_or(ui.available_width().min(240.0));
        let corner_radius = theme.radius;
        let cr = egui::CornerRadius::same(corner_radius.round() as u8);
        // shadcn/web textareas use a relaxed leading. egui's default
        // multiline metrics are too tight for mixed CJK/template text.
        let font_id = if self.monospace {
            egui::FontId::monospace(14.0)
        } else {
            egui::FontId::proportional(14.0)
        };
        let line_height = 22.0;
        let inner_width = (width - h_padding * 2.0).max(1.0);
        let height = if self.auto_resize {
            let mut job = egui::text::LayoutJob::default();
            job.wrap.max_width = inner_width;
            job.append(
                if self.text.is_empty() {
                    " "
                } else {
                    self.text.as_str()
                },
                0.0,
                egui::TextFormat {
                    font_id: font_id.clone(),
                    color: theme.foreground,
                    line_height: Some(line_height),
                    ..Default::default()
                },
            );
            let content_height = ui.fonts(|fonts| fonts.layout_job(job).size().y);
            let max_height = self
                .max_height
                .unwrap_or(f32::INFINITY)
                .max(self.min_height);
            (content_height + v_padding * 2.0).clamp(self.min_height, max_height)
        } else {
            self.min_height
        };

        let desired = egui::vec2(width, height);
        let (outer_rect, outer_response) = ui.allocate_exact_size(desired, egui::Sense::hover());
        let outer_hovered = outer_response.hovered() || ui.rect_contains_pointer(outer_rect);

        // Background and border
        let mut bg =
            crate::paint::interpolate_color::interpolate_color(theme.background, theme.muted, 0.4);
        if outer_hovered {
            bg = crate::paint::interpolate_color::interpolate_color(bg, theme.accent, 0.35);
        }
        ui.painter().rect_filled(outer_rect, cr, bg);
        ui.painter().rect_stroke(
            outer_rect,
            cr,
            egui::Stroke::new(
                1.0,
                if outer_hovered {
                    theme.input
                } else {
                    theme.border
                },
            ),
            egui::epaint::StrokeKind::Inside,
        );

        // Inner area with scroll for overflow
        let inner_rect = outer_rect.shrink2(egui::vec2(h_padding, v_padding));
        let mut child_ui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(inner_rect)
                .layout(egui::Layout::top_down(egui::Align::LEFT)),
        );

        let scroll_id = self
            .id_salt
            .unwrap_or_else(|| outer_response.id.with("textarea-scroll"));
        let scroll_resp = egui::ScrollArea::vertical()
            .id_salt(scroll_id)
            .max_height(inner_rect.height())
            .show(&mut child_ui, |ui| {
                let mut layouter = |ui: &egui::Ui, text: &dyn egui::TextBuffer, wrap_width: f32| {
                    let mut job = egui::text::LayoutJob::default();
                    job.wrap.max_width = wrap_width;
                    job.append(
                        text.as_str(),
                        0.0,
                        egui::TextFormat {
                            font_id: font_id.clone(),
                            color: theme.foreground,
                            line_height: Some(line_height),
                            ..Default::default()
                        },
                    );
                    ui.fonts(|fonts| fonts.layout_job(job))
                };
                let text_edit = egui::TextEdit::multiline(self.text)
                    .frame(false)
                    .hint_text(&self.placeholder)
                    .font(font_id.clone())
                    .lock_focus(self.monospace)
                    .text_color(theme.foreground)
                    .desired_width(inner_rect.width())
                    .desired_rows(3)
                    .layouter(&mut layouter);

                ui.add(text_edit)
            });

        let response = scroll_resp.inner;

        // Focus ring
        if response.has_focus() {
            ui.painter().rect_stroke(
                outer_rect,
                cr,
                egui::Stroke::new(1.0, theme.ring),
                egui::epaint::StrokeKind::Inside,
            );
            crate::paint::paint_focus_ring::paint_focus_ring(
                ui.painter(),
                outer_rect,
                corner_radius,
                theme.ring,
            );
        }

        response
    }
}
