fn keep_rest(ctx: &egui::Context, id: Id, height: f32) {
    let Some(rows) = ctx.data_mut(|m| m.remove_temp::<f32>(id.with(ROWS_KEY))) else { return };
    let rest = (height - rows).max(0.0);
    let used = ctx.data_mut(|m| std::mem::replace(m.get_temp_mut_or(id.with(REST_KEY), 0.0), rest));
    if (used - rest).abs() > 0.5 {
        ctx.request_repaint();
    }
}
