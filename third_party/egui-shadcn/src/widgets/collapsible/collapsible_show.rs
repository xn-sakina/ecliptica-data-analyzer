//! Show method for Collapsible — renders a toggleable panel.

impl super::collapsible::Collapsible {
    /// Shows the collapsible. `open` controls whether content is visible.
    pub fn show(
        self,
        ui: &mut egui::Ui,
        open: &mut bool,
        content: impl FnOnce(&mut egui::Ui),
    ) -> egui::Response {
        let theme = crate::theme::shadcn_theme_ext::ShadcnThemeExt::shadcn_theme(ui.ctx());
        let icon_size: f32 = 14.0;
        let horizontal_padding: f32 = 8.0;
        let content_indent: i8 = 12;

        ui.vertical(|ui| {
            // Trigger row
            let response = ui.horizontal(|ui| {
                let chevron_icon = if *open {
                    crate::icons::lucide_icon::LucideIcon::ChevronDown
                } else {
                    crate::icons::lucide_icon::LucideIcon::ChevronRight
                };

                let gap: f32 = 4.0;
                let vertical_padding: f32 = 6.0;
                let galley = ui.painter().layout_no_wrap(
                    self.title.clone(),
                    egui::FontId::proportional(14.0),
                    theme.foreground,
                );
                let desired = egui::vec2(
                    icon_size + gap + galley.size().x + horizontal_padding * 2.0,
                    galley.size().y.max(icon_size) + vertical_padding * 2.0,
                );
                let (rect, trigger) = ui.allocate_exact_size(desired, egui::Sense::click());

                if ui.is_rect_visible(rect) {
                    if trigger.hovered() {
                        ui.painter().rect_filled(
                            rect,
                            egui::CornerRadius::same(6),
                            theme.accent,
                        );
                    }

                    let icon_rect = egui::Rect::from_min_size(
                        egui::pos2(
                            rect.min.x + horizontal_padding,
                            rect.center().y - icon_size / 2.0,
                        ),
                        egui::vec2(icon_size, icon_size),
                    );
                    crate::icons::paint_icon::paint_icon(
                        ui.painter(),
                        icon_rect,
                        &chevron_icon,
                        theme.foreground,
                    );

                    let text_pos = egui::pos2(
                        rect.min.x + horizontal_padding + icon_size + gap,
                        rect.center().y - galley.size().y / 2.0,
                    );
                    ui.painter().galley(text_pos, galley, theme.foreground);
                }

                if trigger.clicked() {
                    *open = !*open;
                    ui.ctx().request_repaint();
                }

                if trigger.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }

                trigger
            });

            // Content area
            if *open {
                ui.add_space(4.0);
                egui::Frame::NONE
                    .inner_margin(egui::Margin {
                        left: content_indent,
                        right: 0,
                        top: 0,
                        bottom: 0,
                    })
                    .show(ui, content);
            }

            response.inner
        })
        .inner
    }
}
