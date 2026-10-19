fn slider_info(ui: &Ui, resp: &Response, name: &str, shown: Option<SliderValue>) {
    if name.is_empty() {
        return;
    }
    let enabled = ui.is_enabled();
    let Some(s) = shown else {
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Slider, enabled, name));
        return;
    };
    resp.widget_info(|| egui::WidgetInfo { current_text_value: s.text.clone(), ..egui::WidgetInfo::slider(enabled, s.value, name) });
    ui.ctx().accesskit_node_builder(resp.id, |node| {
        node.set_min_numeric_value(s.range.start().min(s.value));
        node.set_max_numeric_value(s.range.end().max(s.value));
    });
}
