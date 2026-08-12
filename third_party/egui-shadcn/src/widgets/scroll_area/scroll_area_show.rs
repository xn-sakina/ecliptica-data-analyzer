//! Show method for ScrollArea — renders a themed scrollable region.

impl super::scroll_area::ScrollArea {
    pub fn show(self, ui: &mut egui::Ui, content: impl FnOnce(&mut egui::Ui)) -> egui::Response {
        let theme = crate::theme::shadcn_theme_ext::ShadcnThemeExt::shadcn_theme(ui.ctx());
        let cr = egui::CornerRadius::same(theme.radius.round() as u8);
        let framed = self.framed;

        let frame = egui::Frame::NONE
            .fill(egui::Color32::TRANSPARENT)
            .corner_radius(cr)
            .stroke(egui::Stroke::new(1.0, theme.border));

        if framed {
            frame
                .show(ui, move |ui| self.show_inner(ui, content))
                .response
        } else if self.fill_available {
            let size = ui.available_size();
            ui.allocate_ui_with_layout(size, egui::Layout::top_down(egui::Align::Min), move |ui| {
                self.show_inner(ui, content)
            })
            .response
        } else {
            ui.scope(move |ui| self.show_inner(ui, content)).response
        }
    }

    fn show_inner(self, ui: &mut egui::Ui, content: impl FnOnce(&mut egui::Ui)) {
        let mut scroll = if self.horizontal {
            egui::ScrollArea::horizontal()
        } else {
            egui::ScrollArea::vertical()
        };
        if let Some(id_salt) = self.id_salt {
            scroll = scroll.id_salt(id_salt);
        }
        scroll
            .max_height(self.max_height)
            .stick_to_bottom(self.stick_to_bottom)
            .auto_shrink(self.auto_shrink)
            .show(ui, content);
    }
}
