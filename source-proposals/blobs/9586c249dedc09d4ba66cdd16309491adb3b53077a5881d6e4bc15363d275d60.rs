pub fn dropdown(ui: &mut Ui, id: impl std::hash::Hash + std::fmt::Debug, current: &str, options: &[&str], width: f32) -> Option<usize> {
    dropdown_with(ui, id, current, options, width, |_| true)
}
