//! Show method for AlertDialog — renders a confirmation modal.

/// Result of an alert dialog interaction.
pub enum AlertDialogResult {
    /// Dialog is still open, no action taken.
    Open,
    /// User cancelled.
    Cancelled,
    /// User confirmed the action.
    Confirmed,
}

impl super::alert_dialog::AlertDialog {
    /// Shows the alert dialog when `open` is true.
    /// Returns the result of the interaction.
    pub fn show(self, ctx: &egui::Context, open: &mut bool) -> AlertDialogResult {
        if !*open {
            return AlertDialogResult::Open;
        }
        if ctx.input(|input| input.key_pressed(egui::Key::Escape)) {
            *open = false;
            ctx.request_repaint();
            return AlertDialogResult::Cancelled;
        }

        let theme = crate::theme::shadcn_theme_ext::ShadcnThemeExt::shadcn_theme(ctx);
        let mut result = AlertDialogResult::Open;
        let accessible_title = self.title.clone();

        // Backdrop
        let screen = ctx.screen_rect();
        let backdrop_layer =
            egui::LayerId::new(egui::Order::Middle, egui::Id::new("alert_dialog_backdrop"));
        let painter = ctx.layer_painter(backdrop_layer);
        painter.rect_filled(
            screen,
            egui::CornerRadius::ZERO,
            egui::Color32::from_black_alpha(60),
        );

        let cr = (theme.radius + 2.0).round() as u8;

        // Dialog panel
        let area_response = egui::Area::new(egui::Id::new("alert_dialog_panel"))
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
                    ui.set_max_width(420.0);

                    // Close button
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                        let close_size = 16.0;
                        let (close_rect, close_resp) = ui.allocate_exact_size(
                            egui::vec2(close_size, close_size),
                            egui::Sense::click(),
                        );
                        let close_resp = close_resp.on_hover_cursor(egui::CursorIcon::PointingHand);
                        close_resp.widget_info(|| {
                            egui::WidgetInfo::labeled(
                                egui::WidgetType::Button,
                                ui.is_enabled(),
                                "关闭确认对话框",
                            )
                        });
                        if ui.is_rect_visible(close_rect) {
                            crate::icons::paint_icon::paint_icon(
                                ui.painter(),
                                close_rect,
                                &crate::icons::lucide_icon::LucideIcon::X,
                                theme.muted_foreground,
                            );
                        }
                        if close_resp.clicked() {
                            *open = false;
                            result = AlertDialogResult::Cancelled;
                            ctx.request_repaint();
                        }
                    });

                    ui.label(
                        egui::RichText::new(&self.title)
                            .color(theme.foreground)
                            .size(18.0)
                            .strong(),
                    );

                    ui.add_space(4.0);
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(&self.description)
                                .color(theme.muted_foreground)
                                .size(14.0),
                        )
                        .wrap(),
                    );

                    ui.add_space(20.0);

                    // Button row aligned to right
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // Action button (shows first from right)
                        let action_variant = if self.destructive {
                            crate::tokens::button_variant::ButtonVariant::Destructive
                        } else {
                            crate::tokens::button_variant::ButtonVariant::Default
                        };

                        let action_btn =
                            crate::widgets::button::button::Button::new(&self.action_text)
                                .variant(action_variant)
                                .show(ui);

                        if action_btn.clicked() {
                            *open = false;
                            result = AlertDialogResult::Confirmed;
                            ui.ctx().request_repaint();
                        }

                        ui.add_space(8.0);

                        // Cancel button
                        let cancel_btn =
                            crate::widgets::button::button::Button::new(&self.cancel_text)
                                .variant(crate::tokens::button_variant::ButtonVariant::Outline)
                                .show(ui);

                        if cancel_btn.clicked() {
                            *open = false;
                            result = AlertDialogResult::Cancelled;
                            ui.ctx().request_repaint();
                        }
                    });
                });
            });
        area_response.response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Window, true, accessible_title.clone())
        });

        result
    }
}
