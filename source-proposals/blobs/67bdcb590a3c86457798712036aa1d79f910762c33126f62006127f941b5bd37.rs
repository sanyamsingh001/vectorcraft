pub(crate) fn scrolled_rows<R>(ui: &mut egui::Ui, salt: impl egui::AsIdSalt, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let id = ui.layer_id().id;
    let rest = ui.ctx().data(|m| m.get_temp::<f32>(id.with(REST_KEY))).unwrap_or(0.0);
    let room = (ui.ctx().content_rect().height() - rest - 2.0 * super::EDGE_GAP).max(MIN_ROWS_H);
    // As tall as the room at once: by itself a scroll area grows a frame at a time with the window.
    let rows = ui.scope(|ui| egui::ScrollArea::vertical().id_salt(salt).max_height(room).min_scrolled_height(room).show(ui, add).inner);
    ui.ctx().data_mut(|m| m.insert_temp(id.with(ROWS_KEY), rows.response.rect.height()));
    rows.inner
}
