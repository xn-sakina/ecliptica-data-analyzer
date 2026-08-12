//! Show method for PropertyRow.

impl super::property_row::PropertyRow {
    /// Renders a property label and a control area.
    pub fn show(self, ui: &mut egui::Ui, content: impl FnOnce(&mut egui::Ui)) -> egui::Response {
        let key = super::property_grid_context_key::property_grid_context_key();
        let context = ui
            .ctx()
            .data(|data| data.get_temp::<super::property_grid_context::PropertyGridContext>(key));
        let label_width = self
            .label_width
            .or_else(|| context.map(|ctx| ctx.label_width))
            .unwrap_or(84.0);

        let row_width = ui.available_width();
        let min_height = if self.align_start { 20.0 } else { 36.0 };
        let cross_align = if self.align_start {
            egui::Align::Min
        } else {
            egui::Align::Center
        };
        let value_width = (row_width - label_width - 10.0).max(1.0);

        ui.allocate_ui_with_layout(
            egui::vec2(row_width, min_height),
            egui::Layout::left_to_right(cross_align),
            |row| {
                row.spacing_mut().item_spacing.x = 10.0;
                row.allocate_ui_with_layout(
                    egui::vec2(label_width, min_height),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |label_ui| {
                        label_ui.set_min_width(label_width);
                        label_ui.set_min_height(min_height);
                        crate::widgets::label::label::Label::new(self.label)
                            .muted()
                            .show(label_ui);
                    },
                );

                row.allocate_ui_with_layout(
                    egui::vec2(value_width, min_height),
                    egui::Layout::left_to_right(cross_align),
                    |value_ui| {
                        value_ui.set_min_width(value_width);
                        value_ui.set_min_height(min_height);
                        content(value_ui);
                    },
                );
            },
        )
        .response
    }
}
