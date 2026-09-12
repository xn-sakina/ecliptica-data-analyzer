//! Widget trait implementation for Textarea.

fn textarea_scroll_source(editor_focused: bool) -> egui::scroll_area::ScrollSource {
    if editor_focused {
        egui::scroll_area::ScrollSource::ALL
    } else {
        // Match web textarea behavior: hovering an unfocused editor should
        // leave wheel scrolling to the enclosing page. Keep its scrollbar
        // draggable so it can still be operated directly.
        egui::scroll_area::ScrollSource::SCROLL_BAR
    }
}

fn textarea_captures_wheel(pointer_over_editor: bool, editor_focused: bool) -> bool {
    pointer_over_editor && editor_focused
}

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
        let editor_id = scroll_id.with("editor");
        let editor_focused = ui.memory(|memory| memory.has_focus(editor_id));
        let mut custom_layouter = self.layouter;
        let scroll_resp = egui::ScrollArea::vertical()
            .id_salt(scroll_id)
            .max_height(inner_rect.height())
            .scroll_source(textarea_scroll_source(editor_focused))
            .show(&mut child_ui, |ui| {
                let mut layouter = |ui: &egui::Ui, text: &dyn egui::TextBuffer, wrap_width: f32| {
                    let mut job = if let Some(layouter) = custom_layouter.as_deref_mut() {
                        layouter(ui, text, wrap_width)
                    } else {
                        egui::text::LayoutJob::default()
                    };
                    job.wrap.max_width = wrap_width;
                    if job.sections.is_empty() {
                        job.append(
                            text.as_str(),
                            0.0,
                            egui::TextFormat {
                                color: theme.foreground,
                                ..Default::default()
                            },
                        );
                    }
                    for section in &mut job.sections {
                        section.format.font_id = font_id.clone();
                        section.format.line_height = Some(line_height);
                    }
                    ui.fonts(|fonts| fonts.layout_job(job))
                };
                let text_edit = egui::TextEdit::multiline(self.text)
                    .id(editor_id)
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

        // Once focused, keep wheel gestures inside the textarea while the
        // pointer is over it, including at a scroll edge. Otherwise an
        // unconsumed delta would also move an enclosing ScrollArea.
        if textarea_captures_wheel(outer_hovered, response.has_focus()) {
            ui.ctx().input_mut(|input| {
                input.smooth_scroll_delta = egui::Vec2::ZERO;
            });
        }

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

#[cfg(test)]
mod tests {
    use super::{textarea_captures_wheel, textarea_scroll_source};

    #[test]
    fn unfocused_textarea_leaves_mouse_wheel_to_its_parent() {
        let source = textarea_scroll_source(false);

        assert!(source.scroll_bar);
        assert!(!source.mouse_wheel);
        assert!(!textarea_captures_wheel(true, false));
    }

    #[test]
    fn focused_textarea_captures_wheel_only_under_the_pointer() {
        let source = textarea_scroll_source(true);

        assert!(source.mouse_wheel);
        assert!(textarea_captures_wheel(true, true));
        assert!(!textarea_captures_wheel(false, true));
    }
}
