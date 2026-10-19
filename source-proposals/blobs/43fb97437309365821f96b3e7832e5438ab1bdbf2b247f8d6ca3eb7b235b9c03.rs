fn dropdown(ui: &mut egui::Ui, d: &mut Dialog, key: &str, width: f32, options: &[(&str, &str)], translated: bool) {
    let cur = d.str(key);
    let shown = options.iter().find(|(v, _)| v.eq_ignore_ascii_case(&cur)).map_or(cur.as_str(), |(_, l)| l);
    let labels: Vec<&str> = options.iter().map(|(_, l)| *l).collect();
    let chosen = if translated {
        crate::widgets::dropdown_names(ui, key, shown, &labels, width)
    } else {
        crate::widgets::dropdown(ui, key, shown, &labels, width)
    };
    if let Some((value, _)) = chosen.and_then(|i| options.get(i)) {
        d.fields.insert(key.into(), json!(value));
    }
}
