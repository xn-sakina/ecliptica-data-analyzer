//! Widget trait implementation for Label.

impl egui::Widget for super::label::Label {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let theme = crate::theme::shadcn_theme_ext::ShadcnThemeExt::shadcn_theme(ui.ctx());
        let color = if self.muted {
            theme.muted_foreground
        } else {
            theme.foreground
        };

        let (font_size, fixed_height) = match self.size {
            Some(size) => {
                let (height, _padding, fs) = size.metrics();
                (fs, Some(height))
            }
            None => (14.0, None),
        };

        let label_text = self.text.clone();
        let font_id = egui::FontId::proportional(font_size);
        let full_galley =
            ui.painter()
                .layout_no_wrap(self.text, font_id.clone(), color);
        let max_width = ui.available_width().max(1.0);
        let truncated = self.truncate && full_galley.size().x > max_width;
        let galley = if truncated {
            let mut job =
                egui::text::LayoutJob::simple_singleline(label_text.clone(), font_id, color);
            job.wrap = egui::text::TextWrapping::truncate_at_width(max_width);
            ui.painter().layout_job(job)
        } else {
            full_galley
        };

        let desired = match fixed_height {
            Some(h) => egui::vec2(galley.size().x, h),
            None => galley.size(),
        };
        let (rect, response) = ui.allocate_exact_size(desired, egui::Sense::hover());
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Label, ui.is_enabled(), label_text.clone())
        });

        if ui.is_rect_visible(rect) {
            // Center text vertically within the allocated rect (matches button centering)
            let text_pos = egui::pos2(rect.min.x, rect.center().y - galley.size().y / 2.0);
            ui.painter()
                .with_clip_rect(rect)
                .galley(text_pos, galley, color);
        }

        if truncated {
            response.on_hover_text(label_text)
        } else {
            response
        }
    }
}
