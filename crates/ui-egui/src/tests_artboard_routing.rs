//! Artboard-tool entry and Artboards-panel routing, independent of Properties.
use crate::{VectorcraftApp, panels};
use egui::{Event, Pos2, Rect, Shape, Ui, pos2, vec2};
use serde_json::json;
use vectorcraft_engine::Session;

struct Harness {
    app: VectorcraftApp,
    ctx: egui::Context,
    time: f64,
}
impl Harness {
    fn new() -> Self {
        let mut app = VectorcraftApp::new(Session::new(), Default::default());
        app.run("file.new", json!({"width": 200, "height": 150})).unwrap();
        app.ui.toolbar_advanced = true;
        app.ui.toolbar_double = false;
        app.canvas_rect = Some(Rect::from_min_size(pos2(48.0, 0.0), vec2(900.0, 1500.0)));
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        Self { app, ctx, time: 0.0 }
    }
    fn frame(&mut self, events: Vec<Event>, draw: fn(&mut VectorcraftApp, &mut Ui)) -> egui::FullOutput {
        self.time += 0.1;
        let mut input = egui::RawInput {
            events,
            time: Some(self.time),
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1000.0, 1600.0))),
            ..Default::default()
        };
        self.app.raw_input_hook(&mut input);
        let mut out = self.ctx.run_ui(input, |ui| draw(&mut self.app, ui));
        out.textures_delta.clear();
        out
    }
    fn settle(&mut self, draw: fn(&mut VectorcraftApp, &mut Ui)) -> egui::FullOutput {
        self.frame(vec![], draw);
        self.frame(vec![], draw)
    }
    fn click(&mut self, at: Pos2, draw: fn(&mut VectorcraftApp, &mut Ui)) -> egui::FullOutput {
        let b = |pressed| Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
        self.frame(vec![Event::PointerMoved(at)], draw);
        self.frame(vec![b(true)], draw);
        self.frame(vec![b(false)], draw);
        self.settle(draw)
    }
}
fn text_at(out: &egui::FullOutput, label: &str) -> Pos2 {
    fn find(s: &Shape, label: &str) -> Option<Pos2> {
        match s {
            Shape::Text(t) if t.galley.text().trim() == label => Some(t.pos + vec2(4.0, 4.0)),
            Shape::Vec(v) => v.iter().find_map(|s| find(s, label)),
            _ => None,
        }
    }
    out.shapes.iter().find_map(|s| find(&s.shape, label)).unwrap_or_else(|| panic!("missing {label}: {}", crate::tests_labels::shapes_text(out)))
}
fn chrome(app: &mut VectorcraftApp, ui: &mut Ui) {
    crate::chrome::status_bar(app, ui);
    crate::toolbar::show(app, ui);
}
fn widgets(ctx: &egui::Context, size: egui::Vec2) -> Vec<Rect> {
    ctx.viewport(|vp| vp.prev_pass.widgets.layers().flat_map(|(_, w)| w.iter()).filter(|w| w.rect.size() == size).map(|w| w.rect).collect())
}
#[test]
fn entering_artboards_from_the_toolbar_targets_the_navigators_board() {
    let mut h = Harness::new();
    h.app.run("artboard.new", json!({})).unwrap();
    h.app.run("shape.rectangle", json!({"x":10,"y":10,"width":20,"height":30})).unwrap();
    let selection = h.app.session.active().unwrap().selection.clone();
    h.settle(chrome);
    let mut nav: Vec<_> = widgets(&h.ctx, vec2(18.0, 18.0)).into_iter().filter(|r| r.center().y > 1568.0).collect();
    nav.sort_by(|a, b| a.left().total_cmp(&b.left()));
    assert_eq!(nav.len(), 4, "navigator buttons");
    h.click(nav[2].center(), chrome);
    assert_eq!(h.app.view().unwrap().artboard, 1);
    let mut tools = widgets(&h.ctx, vec2(36.0, 29.0));
    tools.sort_by(|a, b| a.top().total_cmp(&b.top()));
    let slot = vectorcraft_tools::TOOL_GROUPS.iter().position(|g| g.iter().any(|t| t.id == "artboard")).unwrap();
    assert_eq!(tools.len(), vectorcraft_tools::TOOL_GROUPS.len());
    h.click(tools[slot].center(), chrome);
    assert_eq!(h.app.session.tool_id(), "artboard");
    assert_eq!(h.app.session.tool_options()["active"], 1);
    assert_eq!(h.app.view().unwrap().artboard, 1);
    assert_eq!(h.app.session.active().unwrap().selection.objects, selection.objects);
}
#[test]
fn new_from_the_artboards_menu_activates_one_undoable_board() {
    let mut h = Harness::new();
    h.app.select_tool("artboard");
    let undo = h.app.session.active().unwrap().history.undo.len();
    let out = h.settle(panels::artboards::menu);
    h.click(text_at(&out, "New Artboard"), panels::artboards::menu);
    let d = h.app.session.active().unwrap();
    assert_eq!(d.doc.artboards.len(), 2);
    assert_eq!(d.history.undo.len(), undo + 1);
    assert_eq!(h.app.view().unwrap().artboard, 1);
    assert_eq!(h.app.session.tool_options()["active"], 1);
    h.app.run("edit.undo", json!({})).unwrap();
    assert_eq!(h.app.session.active().unwrap().doc.artboards.len(), 1);
}
#[test]
fn new_from_the_artboards_panel_button_keeps_the_new_board_active() {
    let mut h = Harness::new();
    h.app.select_tool("artboard");
    h.settle(panels::artboards::show);
    let mut buttons = widgets(&h.ctx, vec2(24.0, 24.0));
    buttons.sort_by(|a, b| b.top().total_cmp(&a.top()).then(a.left().total_cmp(&b.left())));
    // The footer has Rearrange, Move Up, Move Down, New Artboard, Delete Artboard.
    let y = buttons.first().unwrap().top();
    let row: Vec<_> = buttons.into_iter().filter(|r| (r.top() - y).abs() < 0.5).collect();
    assert_eq!(row.len(), 5);
    h.click(row[3].center(), panels::artboards::show);
    assert_eq!(h.app.session.active().unwrap().doc.artboards.len(), 2);
    assert_eq!(h.app.view().unwrap().artboard, 1);
    assert_eq!(h.app.session.tool_options()["active"], 1);
}
