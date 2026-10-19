pub(super) fn grid_then(ui: &mut egui::Ui, d: &mut Dialog, unit: Unit, more: impl FnOnce(&mut egui::Ui, &mut Dialog)) {
    let rail = Tokens::get(ui.ctx()).input_border;
    let (lengths, first) = (lengths(&d.kind), order(&d.kind));
    let mut keys: Vec<String> = d
        .fields
        .iter()
        .filter(|(k, v)| !matches!(k.as_str(), "x" | "y" | "origin" | "index") && !k.starts_with("__") && !v.is_boolean())
        .map(|(k, _)| k.clone())
        .collect();
    // Stable: the fields `order` doesn't name keep their order after the ones it does.
    keys.sort_by_key(|k| first.iter().position(|f| f == k).unwrap_or(first.len()));
    egui::Grid::new("dlg").num_columns(2).spacing([10.0, 8.0]).show(ui, |ui| {
        for k in keys {
            let options = choices(&d.kind, &k);
            if lengths.contains(&k.as_str()) {
                length_field(ui, d, &k, &humanized(&k), unit);
            } else if let Some(suffix) = suffix(&d.kind, &k) {
                row_label(ui, &humanized(&k));
                if let Some(v) = crate::widgets::plain_field(ui, ("dlg-num", &k), d.f64(&k, 0.0), suffix, 3, FIELD_W) {
                    d.fields.insert(k, json!(v));
                }
                ui.end_row();
            } else if let Some(ends) = skew_ends(&d.kind, &k) {
                let max = vectorcraft_geom::shapes::GRID_SKEW_MAX;
                row_label(ui, tl!("Skew:"));
                ui.vertical(|ui| {
                    ui.horizontal(|ui| slider_field_named(ui, d, &k, tl!("Skew:"), -max..=max, "%", &|_| rail));
                    rail_ends(ui, ends);
                });
                ui.end_row();
            } else if options.is_empty() {
                let label = match (d.kind.as_str(), k.as_str()) {
                    ("rectangularGrid", "rows") => tl!("Horizontal Dividers:").into(),
                    ("rectangularGrid", "columns") => tl!("Vertical Dividers:").into(),
                    ("polarGrid", "concentric") => tl!("Concentric Dividers:").into(),
                    ("polarGrid", "radial") => tl!("Radial Dividers:").into(),
                    _ => humanized(&k),
                };
                field(ui, d, &k, &label);
            } else {
                row_label(ui, &humanized(&k));
                dropdown(ui, d, &k, FIELD_W, &options, true);
                ui.end_row();
            }
        }
        more(ui, d);
    });
}
