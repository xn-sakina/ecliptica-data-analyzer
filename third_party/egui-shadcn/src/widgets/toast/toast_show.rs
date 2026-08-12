//! Show method for Toast -- renders compact notifications in the top-right.

impl super::toast_state::ToastState {
    /// Shows all active toasts. Call this once per frame from your top-level UI.
    pub fn show(&mut self, ctx: &egui::Context) {
        let theme = crate::theme::shadcn_theme_ext::ShadcnThemeExt::shadcn_theme(ctx);
        let current_time = ctx.input(|i| i.time);
        self.cleanup(current_time);

        if self.toasts.is_empty() {
            return;
        }

        let cr = egui::CornerRadius::same((theme.radius + 2.0).round() as u8);
        let spacing: f32 = 6.0;

        let mut dismissed: Vec<usize> = Vec::new();

        for (idx, toast) in self.toasts.iter().enumerate() {
            let offset_y = idx as f32 * (36.0 + spacing) + 14.0;

            egui::Area::new(egui::Id::new("toast").with(idx))
                .order(egui::Order::Foreground)
                .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-14.0, offset_y))
                .show(ctx, |ui| {
                    let accent = match toast.variant {
                        crate::tokens::toast_variant::ToastVariant::Default => {
                            egui::Color32::from_rgb(225, 218, 255)
                        }
                        crate::tokens::toast_variant::ToastVariant::Success => {
                            egui::Color32::from_rgb(225, 218, 255)
                        }
                        crate::tokens::toast_variant::ToastVariant::Error => {
                            theme.destructive
                        }
                        crate::tokens::toast_variant::ToastVariant::Warning => {
                            egui::Color32::from_rgb(255, 204, 102)
                        }
                        crate::tokens::toast_variant::ToastVariant::Info => {
                            egui::Color32::from_rgb(190, 174, 255)
                        }
                    };

                    let frame = egui::Frame::NONE
                        .fill(egui::Color32::from_rgb(43, 36, 60))
                        .inner_margin(egui::Margin::symmetric(10, 7))
                        .corner_radius(cr)
                        .stroke(egui::Stroke::new(
                            1.0,
                            egui::Color32::from_rgb(126, 106, 190),
                        ));

                    frame.show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let icon_size = 14.0;
                            let (icon_rect, _) = ui.allocate_exact_size(
                                egui::vec2(icon_size, icon_size),
                                egui::Sense::hover(),
                            );
                            if ui.is_rect_visible(icon_rect) {
                                let icon = match toast.variant {
                                    crate::tokens::toast_variant::ToastVariant::Error => {
                                        crate::icons::lucide_icon::LucideIcon::CircleAlert
                                    }
                                    _ => crate::icons::lucide_icon::LucideIcon::CopyCheck,
                                };
                                crate::icons::paint_icon::paint_icon(
                                    ui.painter(),
                                    icon_rect,
                                    &icon,
                                    accent,
                                );
                            }
                            ui.label(
                                egui::RichText::new(&toast.title)
                                    .color(accent)
                                    .size(13.0),
                            );
                            let close_size = 12.0;
                            let (close_rect, close_resp) = ui.allocate_exact_size(
                                egui::vec2(close_size, close_size),
                                egui::Sense::click(),
                            );
                            if ui.is_rect_visible(close_rect) {
                                crate::icons::paint_icon::paint_icon(
                                    ui.painter(),
                                    close_rect,
                                    &crate::icons::lucide_icon::LucideIcon::X,
                                    theme.muted_foreground,
                                );
                            }
                            if close_resp.clicked() {
                                dismissed.push(idx);
                            }
                        });
                    });
                });
        }

        // Remove dismissed toasts (reverse order to keep indices valid)
        for idx in dismissed.into_iter().rev() {
            self.toasts.remove(idx);
        }

        ctx.request_repaint();
    }
}
