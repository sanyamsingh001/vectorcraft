pub fn dropdown_names(ui: &mut Ui, id: impl std::hash::Hash + std::fmt::Debug, current: &str, options: &[&str], width: f32) -> Option<usize> {
    combo(ui, id, current, width, false, |ui| {
        let mut chosen = None;
        for (i, o) in options.iter().enumerate() {
            if ui.add(egui::Button::selectable(*o == current, *o)).clicked() {
                chosen = Some(i);
            }
        }
        chosen
    })
}
