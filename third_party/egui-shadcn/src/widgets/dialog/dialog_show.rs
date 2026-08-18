//! Show method for Dialog — renders a modal overlay.

impl super::dialog::Dialog {
    /// Shows the dialog when `open` is true. Content closure receives a `&mut Ui`.
    pub fn show(self, ctx: &egui::Context, open: &mut bool, content: impl FnOnce(&mut egui::Ui)) {
        if !*open {
            return;
        }
        if self.close_on_escape && ctx.input(|input| input.key_pressed(egui::Key::Escape)) {
            *open = false;
            ctx.request_repaint();
            return;
        }

        let theme = crate::theme::shadcn_theme_ext::ShadcnThemeExt::shadcn_theme(ctx);
        let accessible_title = self.title.clone().unwrap_or_default();
        let dialog_id = egui::Id::new(("dialog", accessible_title.as_str()));

        // Backdrop
        let screen = ctx.input(|i| i.screen_rect());
        let backdrop_layer = egui::LayerId::new(egui::Order::Middle, dialog_id.with("backdrop"));
        let painter = ctx.layer_painter(backdrop_layer);
        painter.rect_filled(
            screen,
            egui::CornerRadius::ZERO,
            egui::Color32::from_black_alpha(60),
        );

        // Consume clicks on backdrop to close
        let backdrop_response = egui::Area::new(dialog_id.with("backdrop_sense"))
            .order(egui::Order::Middle)
            .anchor(egui::Align2::LEFT_TOP, egui::Vec2::ZERO)
            .show(ctx, |ui| {
                let (_, response) = ui.allocate_exact_size(screen.size(), egui::Sense::click());
                response
            });

        if self.close_on_backdrop && backdrop_response.inner.clicked() {
            *open = false;
            ctx.request_repaint();
            return;
        }

        // Dialog panel
        let cr = (theme.radius + 2.0).round() as u8;

        let area_response = egui::Area::new(dialog_id.with("panel"))
            .order(egui::Order::Foreground)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ctx, |ui| {
                let frame = egui::Frame::NONE
                    .fill(theme.background)
                    .inner_margin(egui::Margin::same(24))
                    .corner_radius(egui::CornerRadius::same(cr))
                    .stroke(egui::Stroke::new(1.0, theme.border))
                    .shadow(egui::Shadow {
                        offset: [0, 8],
                        blur: 24,
                        spread: 0,
                        color: egui::Color32::from_black_alpha(12),
                    });

                frame.show(ui, |ui| {
                    ui.set_width(self.width);

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                        let close_size = 28.0;
                        let (close_rect, close_resp) = ui.allocate_exact_size(
                            egui::vec2(close_size, close_size),
                            egui::Sense::click(),
                        );
                        let close_resp = close_resp.on_hover_cursor(egui::CursorIcon::PointingHand);
                        close_resp.widget_info(|| {
                            egui::WidgetInfo::labeled(
                                egui::WidgetType::Button,
                                ui.is_enabled(),
                                self.close_label.clone(),
                            )
                        });
                        if ui.is_rect_visible(close_rect) {
                            if close_resp.hovered() || close_resp.has_focus() {
                                ui.painter().rect_filled(
                                    close_rect,
                                    5.0,
                                    theme.accent.gamma_multiply(0.72),
                                );
                            }
                            let icon_rect = egui::Rect::from_center_size(
                                close_rect.center(),
                                egui::vec2(16.0, 16.0),
                            );
                            crate::icons::paint_icon::paint_icon(
                                ui.painter(),
                                icon_rect,
                                &crate::icons::lucide_icon::LucideIcon::X,
                                if close_resp.hovered() || close_resp.has_focus() {
                                    theme.foreground
                                } else {
                                    theme.muted_foreground
                                },
                            );
                        }
                        if close_resp.clicked() {
                            *open = false;
                            ctx.request_repaint();
                        }

                        ui.allocate_ui_with_layout(
                            egui::vec2(ui.available_width(), 0.0),
                            egui::Layout::top_down(egui::Align::Min),
                            |ui| {
                                if let Some(title) = self.title {
                                    ui.label(
                                        egui::RichText::new(title)
                                            .color(theme.foreground)
                                            .size(18.0)
                                            .strong(),
                                    );
                                }

                                if let Some(desc) = self.description {
                                    ui.add_space(4.0);
                                    ui.label(
                                        egui::RichText::new(desc)
                                            .color(theme.muted_foreground)
                                            .size(14.0),
                                    );
                                }
                            },
                        );
                    });

                    ui.add_space(16.0);
                    content(ui);
                });
            });
        area_response.response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Window, true, accessible_title.clone())
        });
    }
}
