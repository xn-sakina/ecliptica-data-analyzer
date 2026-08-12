//! Show method for ToggleGroup — renders a set of exclusive toggles.

impl super::toggle_group::ToggleGroup {
    /// Shows the toggle group. `selected` is the index of the active item.
    /// Returns the new selected index if changed.
    pub fn show(self, ui: &mut egui::Ui, selected: &mut usize) -> egui::Response {
        let theme = crate::theme::shadcn_theme_ext::ShadcnThemeExt::shadcn_theme(ui.ctx());
        let cr = theme.radius.round() as u8;

        let outer_frame = egui::Frame::NONE
            .fill(theme.muted)
            .inner_margin(egui::Margin::same(2))
            .corner_radius(egui::CornerRadius::same(cr));

        outer_frame
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 2.0;
                    for (idx, label) in self.items.iter().enumerate() {
                        let is_selected = idx == *selected;
                        let is_applied = self.applied_index == Some(idx);
                        let is_draft = is_selected && self.draft_changed;
                        let response = self.render_item(
                            ui,
                            &theme,
                            label,
                            is_selected,
                            is_applied,
                            is_draft,
                            cr,
                        );
                        if response.clicked() {
                            *selected = idx;
                            ui.ctx().request_repaint();
                        }
                    }
                });
            })
            .response
    }

    fn render_item(
        &self,
        ui: &mut egui::Ui,
        theme: &crate::theme::shadcn_theme::ShadcnTheme,
        label: &str,
        is_selected: bool,
        is_applied: bool,
        is_draft: bool,
        cr: u8,
    ) -> egui::Response {
        let (height, h_pad, font_size) = self.size.metrics();
        let high_emphasis = self.variant == crate::tokens::toggle_variant::ToggleVariant::Outline;
        let display_label =
            if high_emphasis && (self.applied_index.is_some() || !self.selection_markers) {
                label.to_owned()
            } else if high_emphasis {
                format!("{} {label}", if is_selected { "●" } else { "○" })
            } else {
                label.to_owned()
            };
        let font_size = if high_emphasis {
            font_size + 1.5
        } else {
            font_size
        };

        let galley = ui.painter().layout_no_wrap(
            display_label,
            egui::FontId::proportional(font_size),
            theme.foreground,
        );

        let desired = egui::vec2(galley.size().x + h_pad * 2.0, height);
        let (rect, response) = ui.allocate_exact_size(desired, egui::Sense::click());
        response.widget_info(|| {
            egui::WidgetInfo::selected(
                egui::WidgetType::Button,
                ui.is_enabled(),
                is_selected,
                label,
            )
        });

        if ui.is_rect_visible(rect) {
            let painter = ui.painter();
            let corner = egui::CornerRadius::same(cr.saturating_sub(1));
            let dark_primary = |primary: u8, background: u8| {
                ((u16::from(primary) * 2 + u16::from(background) * 3) / 5) as u8
            };
            let selected_outline_bg = egui::Color32::from_rgb(
                dark_primary(theme.primary.r(), theme.background.r()),
                dark_primary(theme.primary.g(), theme.background.g()),
                dark_primary(theme.primary.b(), theme.background.b()),
            );

            let warning = egui::Color32::from_rgb(255, 204, 102);
            let applied_bg = egui::Color32::from_rgb(
                dark_primary(theme.primary.r(), theme.background.r()),
                dark_primary(theme.primary.g(), theme.background.g()),
                dark_primary(theme.primary.b(), theme.background.b()),
            );
            let draft_bg = egui::Color32::from_rgb(
                dark_primary(warning.r(), theme.background.r()),
                dark_primary(warning.g(), theme.background.g()),
                dark_primary(warning.b(), theme.background.b()),
            );

            let (bg, fg) = if is_draft {
                (draft_bg, warning)
            } else if is_selected {
                match self.variant {
                    crate::tokens::toggle_variant::ToggleVariant::Default => {
                        (theme.background, theme.foreground)
                    }
                    crate::tokens::toggle_variant::ToggleVariant::Outline => {
                        (selected_outline_bg, theme.foreground)
                    }
                }
            } else if is_applied && high_emphasis {
                (applied_bg, theme.primary)
            } else if response.hovered() {
                (
                    egui::Color32::from_rgba_unmultiplied(
                        theme.background.r(),
                        theme.background.g(),
                        theme.background.b(),
                        128,
                    ),
                    theme.foreground,
                )
            } else if high_emphasis {
                (egui::Color32::TRANSPARENT, theme.foreground)
            } else {
                (egui::Color32::TRANSPARENT, theme.muted_foreground)
            };

            painter.rect_filled(rect, corner, bg);

            if is_selected || (is_applied && high_emphasis) {
                let stroke_color = if is_draft {
                    warning
                } else if is_applied && !is_selected {
                    theme.primary
                } else {
                    match self.variant {
                    crate::tokens::toggle_variant::ToggleVariant::Default => {
                        egui::Color32::from_rgba_unmultiplied(
                            theme.foreground.r(),
                            theme.foreground.g(),
                            theme.foreground.b(),
                            13,
                        )
                    }
                    crate::tokens::toggle_variant::ToggleVariant::Outline => theme.primary,
                    }
                };
                painter.rect_stroke(
                    rect,
                    corner,
                    egui::Stroke::new(1.0, stroke_color),
                    egui::epaint::StrokeKind::Inside,
                );
            }

            let text_pos = egui::pos2(
                rect.center().x - galley.size().x / 2.0,
                rect.center().y - galley.size().y / 2.0,
            );
            if high_emphasis {
                // The bundled proportional font has no separate bold face.
                // A sub-pixel double pass gives compact tabs a clearly heavier
                // label without changing the font or increasing control height.
                painter.galley(text_pos + egui::vec2(0.55, 0.0), galley.clone(), fg);
            }
            painter.galley(text_pos, galley, fg);
        }

        let response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
        if is_draft {
            response.on_hover_text("未保存草稿")
        } else if is_applied && high_emphasis {
            response.on_hover_text("当前已应用")
        } else {
            response
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::toggle_group::ToggleGroup;
    use crate::tokens::{component_size::ComponentSize, toggle_variant::ToggleVariant};

    #[test]
    fn markerless_outline_tabs_still_change_selection_when_clicked() {
        let ctx = egui::Context::default();
        let mut selected = 0;
        let mut group_rect = egui::Rect::NOTHING;
        let items = || {
            vec![
                "Normal".to_owned(),
                "Waiting".to_owned(),
                "Report".to_owned(),
            ]
        };
        let group = || {
            ToggleGroup::new(items())
                .variant(ToggleVariant::Outline)
                .size(ComponentSize::Xs)
                .selection_markers(false)
        };

        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                group_rect = group().show(ui, &mut selected).rect;
            });
        });

        // The three labels are similar widths; the right quarter is safely in
        // the final tab and avoids relying on private child response rects.
        let pointer = egui::pos2(group_rect.right() - 12.0, group_rect.center().y);
        let pointer_event = |pressed| egui::Event::PointerButton {
            pos: pointer,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        let _ = ctx.run(
            egui::RawInput {
                events: vec![egui::Event::PointerMoved(pointer), pointer_event(true)],
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    group().show(ui, &mut selected);
                });
            },
        );
        let _ = ctx.run(
            egui::RawInput {
                events: vec![pointer_event(false)],
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    group().show(ui, &mut selected);
                });
            },
        );

        assert_eq!(selected, 2);
    }
}
