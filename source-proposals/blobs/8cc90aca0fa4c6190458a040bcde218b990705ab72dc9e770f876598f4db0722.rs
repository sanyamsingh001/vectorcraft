pub(super) fn grid(ui: &mut egui::Ui, d: &mut Dialog, unit: Unit) {
    grid_then(ui, d, unit, |_, _| {});
}
