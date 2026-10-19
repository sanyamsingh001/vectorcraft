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

fn tabs_with_logic(app: &mut VectorcraftApp, ui: &mut Ui) {
    app.logic(ui.ctx());
    app.ui(ui);
}
fn tabs(h: &Harness) -> Vec<Rect> {
    let mut tabs: Vec<_> = h.ctx.viewport(|vp| {
        vp.prev_pass
            .widgets
            .layers()
            .flat_map(|(_, w)| w.iter())
            .filter(|w| w.rect.height() == 34.0 && w.rect.width() > 50.0)
            .map(|w| w.rect)
            .collect()
    });
    tabs.sort_by(|a, b| a.left().total_cmp(&b.left()));
    tabs
}
fn hover_on_pasteboard(h: &mut Harness) {
    let view = h.app.view_info();
    crate::canvas::dispatch(&mut h.app, &vectorcraft_tools::PointerEvent::new(vectorcraft_tools::PointerKind::Hover, -100.0, -100.0), view);
}
fn delete_with_keyboard(h: &mut Harness) {
    fn draw(app: &mut VectorcraftApp, ui: &mut Ui) {
        app.logic(ui.ctx());
    }
    // The real logic entry installs fonts on its first frame and enables shortcuts on the next.
    h.settle(draw);
    h.frame(vec![Event::Key { key: egui::Key::Delete, physical_key: None, pressed: true, repeat: false, modifiers: Default::default() }], draw);
}
fn two_documents(h: &mut Harness) {
    h.app.run("artboard.new", json!({})).unwrap();
    h.app.run("view.goToArtboard", json!({"index": 1})).unwrap();
    h.app.select_tool("artboard");
    h.app.run("file.new", json!({"width": 70, "height": 90})).unwrap();
    assert_eq!(h.app.session.active_index(), Some(1));
    assert_eq!(h.app.view().unwrap().artboard, 0);
}

#[test]
fn routing_extension_navigator_while_editing_keeps_hover_and_delete_on_the_chosen_board() {
    let mut h = Harness::new();
    h.app.run("artboard.new", json!({})).unwrap();
    h.app.select_tool("artboard");
    let first = h.app.session.active().unwrap().doc.artboards[0].rect;
    h.settle(chrome);
    let mut nav: Vec<_> = widgets(&h.ctx, vec2(18.0, 18.0)).into_iter().filter(|r| r.center().y > 1568.0).collect();
    nav.sort_by(|a, b| a.left().total_cmp(&b.left()));
    assert_eq!(nav.len(), 4);
    h.click(nav[2].center(), chrome);
    assert_eq!(h.app.view().unwrap().artboard, 1);
    assert_eq!(h.app.session.tool_options()["active"], 1);
    hover_on_pasteboard(&mut h);
    assert_eq!(h.app.view().unwrap().artboard, 1, "hover must not undo navigation");
    delete_with_keyboard(&mut h);
    let boards = &h.app.session.active().unwrap().doc.artboards;
    assert_eq!(boards.len(), 1);
    assert_eq!(boards[0].rect, first, "Delete removes the navigated board");
}

#[test]
fn routing_extension_clicking_a_document_tab_restores_its_board_before_delete() {
    let mut h = Harness::new();
    two_documents(&mut h);
    let first = h.app.session.documents()[0].doc.artboards[0].rect;
    let untouched = h.app.session.documents()[1].doc.artboards[0].rect;
    h.settle(tabs_with_logic);
    let row = tabs(&h);
    assert_eq!(row.len(), 2);
    h.click(pos2(row[0].left() + 80.0, row[0].center().y), tabs_with_logic);
    assert_eq!(h.app.session.active_index(), Some(0));
    assert_eq!(h.app.view().unwrap().artboard, 1);
    assert_eq!(h.app.session.tool_options()["active"], 1);
    hover_on_pasteboard(&mut h);
    assert_eq!(h.app.view().unwrap().artboard, 1);
    delete_with_keyboard(&mut h);
    assert_eq!(h.app.session.documents()[0].doc.artboards.len(), 1);
    assert_eq!(h.app.session.documents()[0].doc.artboards[0].rect, first);
    assert_eq!(h.app.session.documents()[1].doc.artboards[0].rect, untouched);
}

#[test]
fn routing_extension_close_command_restores_the_surviving_documents_board() {
    let mut h = Harness::new();
    two_documents(&mut h);
    h.app.run("file.close", json!({})).unwrap();
    assert!(h.app.ui.dialog.is_none(), "the new document is clean");
    assert_eq!(h.app.session.documents().len(), 1);
    assert_eq!(h.app.view().unwrap().artboard, 1);
    assert_eq!(h.app.session.tool_options()["active"], 1);
    hover_on_pasteboard(&mut h);
    assert_eq!(h.app.view().unwrap().artboard, 1);
}

#[test]
fn routing_extension_tab_close_restores_the_surviving_documents_board() {
    let mut h = Harness::new();
    two_documents(&mut h);
    h.settle(tabs_with_logic);
    let row = tabs(&h);
    assert_eq!(row.len(), 2);
    h.click(pos2(row[1].left() + 16.0, row[1].center().y), tabs_with_logic);
    assert!(h.app.ui.dialog.is_none());
    assert_eq!(h.app.session.documents().len(), 1);
    assert_eq!(h.app.view().unwrap().artboard, 1);
    assert_eq!(h.app.session.tool_options()["active"], 1);
    hover_on_pasteboard(&mut h);
    assert_eq!(h.app.view().unwrap().artboard, 1);
}

#[test]
fn routing_extension_new_document_dialog_keeps_the_new_document_on_its_first_board() {
    let mut h = Harness::new();
    h.app.run("artboard.new", json!({})).unwrap();
    h.app.run("view.goToArtboard", json!({"index": 1})).unwrap();
    h.app.select_tool("artboard");
    h.app.run("file.newDialog", json!({})).unwrap();
    let d = h.app.ui.dialog.as_mut().unwrap();
    d.fields.insert("width".into(), json!(70));
    d.fields.insert("height".into(), json!(90));
    d.fields.insert("artboards".into(), json!(2));
    crate::dialogs::confirm(&mut h.app).unwrap();
    assert_eq!(h.app.session.documents().len(), 2);
    assert_eq!(h.app.session.active_index(), Some(1));
    assert_eq!(h.app.session.active().unwrap().doc.artboards.len(), 2);
    assert_eq!(h.app.view().unwrap().artboard, 0);
    assert_eq!(h.app.session.tool_options()["active"], 0);
    hover_on_pasteboard(&mut h);
    assert_eq!(h.app.view().unwrap().artboard, 0);
    assert_eq!(h.app.views[0].artboard, 1, "the first document keeps its own board");
}

#[test]
fn routing_extension_frame_logic_keeps_the_board_clicked_on_canvas() {
    let mut h = Harness::new();
    h.app.run("artboard.new", json!({})).unwrap();
    h.app.select_tool("artboard");
    let center = h.app.session.active().unwrap().doc.artboards[1].rect.center();
    let view = h.app.view_info();
    for kind in [vectorcraft_tools::PointerKind::Down, vectorcraft_tools::PointerKind::Up] {
        crate::canvas::dispatch(&mut h.app, &vectorcraft_tools::PointerEvent::new(kind, center.x, center.y), view);
    }
    assert_eq!(h.app.view().unwrap().artboard, 1);
    h.settle(tabs_with_logic);
    assert_eq!(h.app.view().unwrap().artboard, 1);
    assert_eq!(h.app.session.tool_options()["active"], 1);
}

#[test]
fn routing_extension_frame_logic_keeps_a_pasted_artboard_active() {
    let mut h = Harness::new();
    h.app.select_tool("artboard");
    h.app.run("edit.copy", json!({})).unwrap();
    let pasted = h.app.run("edit.paste", json!({})).unwrap()["artboard"].as_u64().unwrap();
    assert_eq!(pasted, 1);
    assert_eq!(h.app.session.tool_options()["active"], pasted);
    h.settle(tabs_with_logic);
    assert_eq!(h.app.session.tool_options()["active"], pasted, "document synchronization must not overwrite an ordinary tool action");
    hover_on_pasteboard(&mut h);
    assert_eq!(h.app.view().unwrap().artboard as u64, pasted);
}
