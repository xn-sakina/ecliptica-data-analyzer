//! Show method for Card — renders content inside a bordered container.

impl super::card::Card {
    /// Renders the card container and calls `content` inside it.
    pub fn show(self, ui: &mut egui::Ui, content: impl FnOnce(&mut egui::Ui)) -> egui::Response {
        let theme = crate::theme::shadcn_theme_ext::ShadcnThemeExt::shadcn_theme(ui.ctx());

        let cr = theme.radius + 2.0; // rounded-xl = radius + 2

        let frame = egui::Frame::NONE
            .fill(theme.card)
            .inner_margin(egui::Margin {
                left: 16, // p-4
                right: 16,
                top: 16,
                bottom: 16,
            })
            .corner_radius(egui::CornerRadius::same(cr.round() as u8))
            // Use the opaque border token. A translucent foreground stroke
            // looks washed out on dark cards and can read like a gap in a
            // transparent native viewport.
            .stroke(egui::Stroke::new(1.0, theme.border));

        frame.show(ui, content).response
    }
}
