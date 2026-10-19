//! Artboard-tool entry and routing through the navigator, Artboards and Properties panels.
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
    crate::canvas::dispatch(&mut h.app, &vectorcraft_tools::PointerEvent::new(vectorcraft_tools::PointerKind::Move, -100.0, -100.0), view);
}
fn delete_with_keyboard(h: &mut Harness) {
    key_with_keyboard(h, egui::Key::Delete);
}
fn key_with_keyboard(h: &mut Harness, key: egui::Key) {
    fn draw(app: &mut VectorcraftApp, ui: &mut Ui) {
        app.logic(ui.ctx());
    }
    // The real logic entry installs fonts on its first frame and enables shortcuts on the next.
    h.settle(draw);
    let event = |pressed| Event::Key { key, physical_key: None, pressed, repeat: false, modifiers: Default::default() };
    h.frame(vec![event(true)], draw);
    h.frame(vec![event(false)], draw);
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
    // The first canvas fit changes the zoom in the title, moving a trailing close button.
    h.settle(tabs_with_logic);
    let row = tabs(&h);
    assert_eq!(row.len(), 2);
    // Discover the rendered close button inside the tab, on either side of its title.
    let close_ids: Vec<_> = h.ctx.viewport(|vp| {
        vp.prev_pass
            .widgets
            .layers()
            .flat_map(|(_, w)| w.iter())
            .filter(|w| w.sense.senses_click() && row[1].contains_rect(w.rect) && w.rect != row[1])
            .map(|w| w.id)
            .collect()
    });
    assert_eq!(close_ids.len(), 1, "the second tab has one close button");
    let close = h.ctx.read_response(close_ids[0]).expect("the second tab's close button is rendered");
    assert!(close.enabled(), "the second tab's close button is enabled");
    h.click(close.rect.center(), tabs_with_logic);
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

#[test]
fn artboard_delete_keys_preserve_artwork_even_on_the_only_board() {
    for key in [egui::Key::Delete, egui::Key::Backspace] {
        for board_count in [1, 2] {
            let mut h = Harness::new();
            if board_count == 2 {
                h.app.run("artboard.new", json!({})).unwrap();
                h.app.run("view.goToArtboard", json!({"index": 1})).unwrap();
            }
            let id = h.app.run("shape.rectangle", json!({"x": 10, "y": 10, "width": 20, "height": 30})).unwrap()["id"].as_u64().unwrap();
            let id = vectorcraft_doc::NodeId(id);
            let st = h.app.session.active().unwrap();
            let art = serde_json::to_value(st.doc.node(id).unwrap()).unwrap();
            let selection = st.selection.objects.clone();
            let first_board = st.doc.artboards[0].id;
            let undo = st.history.undo.len();
            h.settle(chrome);
            let mut tools = widgets(&h.ctx, vec2(36.0, 29.0));
            tools.sort_by(|a, b| a.top().total_cmp(&b.top()));
            let slot = vectorcraft_tools::TOOL_GROUPS.iter().position(|g| g.iter().any(|t| t.id == "artboard")).unwrap();
            assert_eq!(tools.len(), vectorcraft_tools::TOOL_GROUPS.len());
            h.click(tools[slot].center(), chrome);
            assert_eq!(h.app.session.tool_id(), "artboard");
            assert_eq!(h.app.session.tool_options()["active"], board_count - 1);
            key_with_keyboard(&mut h, key);
            let st = h.app.session.active().unwrap();
            assert_eq!(st.doc.artboards.len(), 1, "{key:?}, {board_count} boards");
            assert_eq!(st.doc.artboards[0].id, first_board, "the active last board is removed, if allowed");
            assert_eq!(serde_json::to_value(st.doc.node(id).unwrap()).unwrap(), art, "{key:?} preserves selected artwork");
            assert_eq!(st.selection.objects, selection);
            assert_eq!(st.history.undo.len(), undo + usize::from(board_count == 2), "the only board is a no-op; deletion is one undo step");
            // Positive control: the same real key still clears selected art with Selection.
            h.app.select_tool("selection");
            key_with_keyboard(&mut h, key);
            assert!(h.app.session.active().unwrap().doc.node(id).is_none(), "{key:?} reached the ordinary shortcut handler");
        }
    }
}

#[test]
fn properties_delete_last_of_three_artboards_updates_navigator_and_tool() {
    fn draw(app: &mut VectorcraftApp, ui: &mut Ui) {
        app.logic(ui.ctx());
        egui::Panel::right("routing-properties").exact_size(300.0).show(ui, |ui| panels::properties::show(app, ui));
    }
    let mut h = Harness::new();
    h.app.run("artboard.new", json!({})).unwrap();
    h.app.run("artboard.new", json!({})).unwrap();
    h.app.run("view.goToArtboard", json!({"index": 2})).unwrap();
    let before = h.settle(draw);
    h.click(text_at(&before, "Edit Artboards"), draw);
    assert_eq!(h.app.session.tool_id(), "artboard");
    assert_eq!(h.app.session.tool_options()["active"], 2);
    let st = h.app.session.active().unwrap();
    let original = serde_json::to_value(&st.doc.artboards).unwrap();
    let survivors = serde_json::to_value(&st.doc.artboards[..2]).unwrap();
    let undo = st.history.undo.len();
    let before = h.settle(draw);
    text_at(&before, "Artboard 3");
    let after = h.click(text_at(&before, "Delete Artboard"), draw);
    let st = h.app.session.active().unwrap();
    assert_eq!(serde_json::to_value(&st.doc.artboards).unwrap(), survivors, "only the last board is deleted");
    assert_eq!(st.history.undo.len(), undo + 1);
    assert_eq!(h.app.view().unwrap().artboard, 1);
    assert_eq!(h.app.session.tool_options()["active"], 1);
    text_at(&after, "Artboard 2");
    let view = h.app.view_info();
    let overlays = h.app.session.overlays(view);
    assert_eq!(overlays.iter().filter(|o| matches!(o, vectorcraft_tools::Overlay::Anchor { .. })).count(), 8, "surviving board keeps its handles");
    hover_on_pasteboard(&mut h);
    assert_eq!(h.app.view().unwrap().artboard, 1, "hover keeps the survivor active");
    assert_eq!(h.app.session.tool_options()["active"], 1);
    h.app.run("edit.undo", json!({})).unwrap();
    assert_eq!(serde_json::to_value(&h.app.session.active().unwrap().doc.artboards).unwrap(), original);
}

#[test]
fn properties_scale_artwork_click_persists_and_numeric_resize_targets_the_active_board() {
    fn draw(app: &mut VectorcraftApp, ui: &mut Ui) {
        app.logic(ui.ctx());
        egui::Panel::right("scale-properties").exact_size(300.0).show(ui, |ui| panels::properties::show(app, ui));
    }
    fn bounds(h: &Harness, id: vectorcraft_doc::NodeId) -> vectorcraft_geom::Rect {
        h.app.session.active().unwrap().doc.node(id).unwrap().geometric_bounds().unwrap()
    }
    fn set_width(h: &mut Harness, old: &str, new: &str) {
        let out = h.settle(draw);
        h.click(text_at(&out, old), draw);
        h.frame(vec![Event::Key { key: egui::Key::A, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::COMMAND }], draw);
        h.frame(vec![Event::Text(new.into())], draw);
        for pressed in [true, false] {
            h.frame(vec![Event::Key { key: egui::Key::Enter, physical_key: None, pressed, repeat: false, modifiers: Default::default() }], draw);
        }
        h.settle(draw);
    }
    let mut h = Harness::new();
    let first_art =
        vectorcraft_doc::NodeId(h.app.run("shape.rectangle", json!({"x": 10, "y": 10, "width": 10, "height": 10})).unwrap()["id"].as_u64().unwrap());
    h.app.run("artboard.new", json!({"x": 600, "y": 0, "width": 100, "height": 80})).unwrap();
    let art =
        vectorcraft_doc::NodeId(h.app.run("shape.rectangle", json!({"x": 610, "y": 20, "width": 10, "height": 10})).unwrap()["id"].as_u64().unwrap());
    h.app.run("view.goToArtboard", json!({"index": 1})).unwrap();
    h.app.run("select.none", json!({})).unwrap();
    let out = h.settle(draw);
    h.click(text_at(&out, "Edit Artboards"), draw);
    assert_eq!(h.app.session.tool_id(), "artboard");
    assert_eq!(h.app.session.tool_options()["active"], 1);
    assert_eq!(h.app.session.tool_options()["moveArt"], true);
    assert_eq!(h.app.session.tool_options()["scaleArt"], false);
    let first_board = h.app.session.active().unwrap().doc.artboards[0].rect;
    let first_bounds = bounds(&h, first_art);
    let original_board = h.app.session.active().unwrap().doc.artboards[1].rect;
    let original_art = serde_json::to_value(h.app.session.active().unwrap().doc.node(art).unwrap()).unwrap();
    let selection = h.app.session.active().unwrap().selection.objects.clone();
    let undo = h.app.session.active().unwrap().history.undo.len();
    // Default/off is exercised through the real Properties width editor.
    set_width(&mut h, "100 pt", "200 pt");
    assert_eq!(h.app.session.active().unwrap().doc.artboards[1].rect, vectorcraft_geom::Rect::new(600.0, 0.0, 800.0, 80.0));
    assert_eq!(bounds(&h, art), vectorcraft_geom::Rect::new(610.0, 20.0, 620.0, 30.0));
    assert_eq!(h.app.session.active().unwrap().history.undo.len(), undo + 1);
    h.app.run("edit.undo", json!({})).unwrap();
    let out = h.settle(draw);
    h.click(text_at(&out, "Move Artwork with Artboard"), draw);
    let out = h.settle(draw);
    h.click(text_at(&out, "Scale Artwork with Artboard"), draw);
    assert_eq!(h.app.session.tool_options()["moveArt"], false, "Move and Scale are independent");
    assert_eq!(h.app.session.tool_options()["scaleArt"], true);
    assert_eq!(h.app.session.prefs.tool_settings["artboard"]["moveArt"], false);
    assert_eq!(h.app.session.prefs.tool_settings["artboard"]["scaleArt"], true);
    assert_eq!(h.app.session.active().unwrap().history.undo.len(), undo, "checkboxes do not edit the document");
    h.app.select_tool("selection");
    let out = h.settle(draw);
    h.click(text_at(&out, "Edit Artboards"), draw);
    assert_eq!(h.app.session.tool_options()["active"], 1);
    assert_eq!(h.app.session.tool_options()["moveArt"], false);
    assert_eq!(h.app.session.tool_options()["scaleArt"], true);
    let mut reopened = Session::new();
    reopened.prefs = serde_json::from_value(serde_json::to_value(&h.app.session.prefs).unwrap()).unwrap();
    reopened.execute("file.new", &json!({})).unwrap();
    reopened.select_tool("artboard", Default::default()).unwrap();
    assert_eq!(reopened.tool_options()["moveArt"], false);
    assert_eq!(reopened.tool_options()["scaleArt"], true);
    // Scale is still enabled after leaving/re-entering, and only the navigator's board changes.
    set_width(&mut h, "100 pt", "200 pt");
    let st = h.app.session.active().unwrap();
    assert_eq!(st.doc.artboards[1].rect, vectorcraft_geom::Rect::new(600.0, 0.0, 800.0, 80.0));
    assert_eq!(bounds(&h, art), vectorcraft_geom::Rect::new(620.0, 20.0, 640.0, 30.0));
    assert_eq!(st.doc.artboards[0].rect, first_board);
    assert_eq!(bounds(&h, first_art), first_bounds);
    assert_eq!(st.selection.objects, selection);
    assert_eq!(st.history.undo.len(), undo + 1, "the board and art resize in one undo step");
    assert_eq!(h.app.view().unwrap().artboard, 1);
    assert_eq!(h.app.session.tool_options()["active"], 1);
    h.app.run("edit.undo", json!({})).unwrap();
    let st = h.app.session.active().unwrap();
    assert_eq!(st.doc.artboards[1].rect, original_board);
    assert_eq!(serde_json::to_value(st.doc.node(art).unwrap()).unwrap(), original_art);
    let out = h.settle(draw);
    h.click(text_at(&out, "Scale Artwork with Artboard"), draw);
    assert_eq!(h.app.session.tool_options()["scaleArt"], false);
    assert_eq!(h.app.session.prefs.tool_settings["artboard"]["scaleArt"], false);
    set_width(&mut h, "100 pt", "200 pt");
    assert_eq!(bounds(&h, art), vectorcraft_geom::Rect::new(610.0, 20.0, 620.0, 30.0));
    assert_eq!(h.app.session.active().unwrap().history.undo.len(), undo + 1);
    h.app.run("edit.undo", json!({})).unwrap();
    // The Properties toggle also reaches a real handle drag with repeated previews.
    let out = h.settle(draw);
    h.click(text_at(&out, "Scale Artwork with Artboard"), draw);
    let view = h.app.view_info();
    for (kind, x, y) in [
        (vectorcraft_tools::PointerKind::Down, 700.0, 80.0),
        (vectorcraft_tools::PointerKind::Drag, 750.0, 120.0),
        (vectorcraft_tools::PointerKind::Drag, 800.0, 160.0),
        (vectorcraft_tools::PointerKind::Up, 800.0, 160.0),
    ] {
        crate::canvas::dispatch(&mut h.app, &vectorcraft_tools::PointerEvent::new(kind, x, y), view);
    }
    h.settle(draw);
    let st = h.app.session.active().unwrap();
    assert_eq!(st.doc.artboards[1].rect, vectorcraft_geom::Rect::new(600.0, 0.0, 800.0, 160.0));
    assert_eq!(bounds(&h, art), vectorcraft_geom::Rect::new(620.0, 40.0, 640.0, 60.0), "previews scale from the original art");
    assert_eq!(st.doc.artboards[0].rect, first_board);
    assert_eq!(bounds(&h, first_art), first_bounds);
    assert_eq!(st.history.undo.len(), undo + 1, "the handle drag remains one undo step");
    assert_eq!(h.app.view().unwrap().artboard, 1);
    assert_eq!(h.app.session.tool_options()["active"], 1);
    h.app.run("edit.undo", json!({})).unwrap();
    let st = h.app.session.active().unwrap();
    assert_eq!(st.doc.artboards[1].rect, original_board);
    assert_eq!(serde_json::to_value(st.doc.node(art).unwrap()).unwrap(), original_art);
}

#[test]
fn localized_properties_artwork_checkboxes_fit_narrow_panels() {
    const CHILD: &str = "VECTORCRAFT_ARTBOARD_LAYOUT_TEST_CHILD";
    // Drawing language is process-wide. A separate copy of this test binary keeps localized
    // frames from changing the English expected by other tests running in parallel.
    if std::env::var_os(CHILD).is_none() {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "tests_artboard_routing::localized_properties_artwork_checkboxes_fit_narrow_panels", "--nocapture", "--test-threads=1"])
            .env(CHILD, "1")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "localized Properties layout: {}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    fn draw(app: &mut VectorcraftApp, ui: &mut Ui) {
        app.logic(ui.ctx());
        let width = ui.ctx().data(|d| d.get_temp::<f32>(egui::Id::new("locale-properties-width"))).unwrap();
        // The real dock permits 230..=520 pt (300 pt default). Keep its Properties margins
        // and vertical scroll policy, while fixing the width for each input scenario.
        egui::Panel::right("locale-properties").exact_size(width).frame(egui::Frame::NONE).show(ui, |ui| {
            egui::Frame::NONE.inner_margin(egui::Margin { left: 12, right: 10, top: 10, bottom: 8 }).show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("locale-properties-scroll")
                    .auto_shrink([false, false])
                    .show(ui, |ui| panels::properties::show(app, ui));
            });
        });
    }
    fn label_rect(out: &egui::FullOutput, label: &str) -> (Rect, Rect) {
        fn find(shape: &Shape, label: &str) -> Option<Rect> {
            match shape {
                Shape::Text(t) if t.galley.text().trim() == label => Some(Rect::from_min_size(t.pos, t.galley.size())),
                Shape::Vec(v) => v.iter().find_map(|s| find(s, label)),
                _ => None,
            }
        }
        out.shapes.iter().find_map(|s| find(&s.shape, label).map(|r| (r, s.clip_rect))).unwrap_or_else(|| panic!("missing {label}"))
    }
    let mut failures = vec![];
    for code in ["en", "fr", "ja"] {
        let lang = crate::i18n::Lang::from_code(code).unwrap();
        for width in [230.0, 300.0] {
            let mut h = Harness::new();
            h.app.session.prefs.interface_language = code.into();
            h.app.select_tool("artboard");
            h.ctx.data_mut(|d| d.insert_temp(egui::Id::new("locale-properties-width"), width));
            let undo = h.app.session.active().unwrap().history.undo.len();
            let content = Rect::from_min_max(pos2(1000.0 - width + 12.0, 10.0), pos2(990.0, 1592.0));
            let mut rows = vec![];
            for (key, source, default) in [("moveArt", "Move Artwork with Artboard", true), ("scaleArt", "Scale Artwork with Artboard", false)] {
                if code != "en" {
                    assert!(crate::i18n::has(lang, source), "{code}: missing {source}");
                }
                let label = crate::i18n::tr(lang, source);
                let out = h.settle(draw);
                let (rect, clip) = label_rect(&out, label);
                rows.push(rect);
                if !clip.contains_rect(rect) || !content.contains_rect(rect) {
                    failures.push(format!("{code}, {width} pt, {source}: label {rect:?}, clip {clip:?}, content {content:?}"));
                }
                // Check the entire label, but exercise the visible checkbox itself even if a
                // long label extends beyond the panel. choice_row places it 18 pt before text.
                let checkbox = pos2(rect.left() - 11.5, rect.center().y);
                assert!(clip.contains(checkbox) && content.contains(checkbox), "{code}: checkbox is outside its panel");
                h.click(checkbox, draw);
                assert_eq!(h.app.session.tool_options()[key], !default, "{code}, {width} pt: {key} click");
                assert_eq!(h.app.session.prefs.tool_settings["artboard"][key], !default);
            }
            assert!(rows[0].bottom() <= rows[1].top(), "{code}: checkbox labels overlap");
            assert_eq!(h.app.session.active().unwrap().history.undo.len(), undo, "localized checkbox clicks do not edit the document");
        }
    }
    assert!(failures.is_empty(), "clipped Properties labels (English baseline included):\n{}", failures.join("\n"));
}

// Original, small Artboard/Properties integration controls. No canvas pointer dispatch occurs
// after the command: a user can keep the pointer over the panel while choosing Undo.
fn round3_properties(app: &mut VectorcraftApp, ui: &mut Ui) {
    app.logic(ui.ctx());
    egui::Panel::right("round3-artboard-properties").exact_size(300.0).show(ui, |ui| panels::properties::show(app, ui));
}

fn round3_artboard_fixture(third: bool) -> (Harness, Vec<vectorcraft_doc::NodeId>) {
    let mut app = VectorcraftApp::new(Session::new(), Default::default());
    app.run("file.new", json!({"width":100,"height":80,"created":1_000_000_000})).unwrap();
    app.run("artboard.setProps", json!({"index":0,"name":"First"})).unwrap();
    let mut art = vec![];
    let board_count = if third { 3 } else { 2 };
    for index in 0..board_count {
        if index != 0 {
            app.run("artboard.new", json!({"x":200*index,"y":0,"width":100,"height":80,"name":if index == 1 { "Second" } else { "Third" }})).unwrap();
        }
        art.push(vectorcraft_doc::NodeId(
            app.run("shape.rectangle", json!({"x":200*index+10,"y":20,"width":10,"height":10})).unwrap()["id"].as_u64().unwrap(),
        ));
    }
    app.run("select.none", json!({})).unwrap();
    app.run("view.goToArtboard", json!({"index":1})).unwrap();
    app.select_tool("artboard");
    app.run("tool.setOption", json!({"key":"scaleArt","value":true})).unwrap();
    // Properties occupies x=700..1000 in this existing 1000 pt harness.
    app.canvas_rect = Some(Rect::from_min_size(Pos2::ZERO, vec2(700.0, 1600.0)));
    let ctx = egui::Context::default();
    crate::theme::install_fonts(&ctx);
    let mut h = Harness { app, ctx, time: 0.0 };
    h.settle(round3_properties);
    (h, art)
}

fn round3_resize_second_and_check(h: &mut Harness, art: &[vectorcraft_doc::NodeId], second: u32) {
    let before = h.app.session.active().unwrap().doc.clone();
    let before_json = serde_json::to_value(before.as_ref()).unwrap();
    let selection = h.app.session.active().unwrap().selection.clone();
    let undo = h.app.session.active().unwrap().history.undo.len();
    let active = h.app.session.tool_options()["active"].as_u64().unwrap() as usize;
    assert_eq!(before.artboards[active].id, second, "positive control: the engine follows Second");
    assert!(h.app.session.tool_options()["scaleArt"].as_bool().unwrap());
    assert!(!h.app.session.tool_busy() && !h.app.session.in_interaction());
    let out = h.settle(round3_properties);
    h.click(text_at(&out, "100 pt"), round3_properties);
    h.frame(
        vec![Event::Key { key: egui::Key::A, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::COMMAND }],
        round3_properties,
    );
    h.frame(vec![Event::Text("200 pt".into())], round3_properties);
    for pressed in [true, false] {
        h.frame(
            vec![Event::Key { key: egui::Key::Enter, physical_key: None, pressed, repeat: false, modifiers: Default::default() }],
            round3_properties,
        );
    }
    h.settle(round3_properties);
    let st = h.app.session.active().unwrap();
    let board = st.doc.artboards.iter().find(|b| b.id == second).unwrap();
    assert_eq!(board.rect, vectorcraft_geom::Rect::new(200.0, 0.0, 400.0, 80.0), "Properties must resize Second, which the engine kept active");
    assert_eq!(st.doc.node(art[1]).unwrap().geometric_bounds().unwrap(), vectorcraft_geom::Rect::new(220.0, 20.0, 240.0, 30.0));
    for (index, id) in art.iter().enumerate().filter(|(index, _)| *index != 1) {
        assert_eq!(st.doc.node(*id), before.node(*id), "other original rectangle {index} stays unchanged");
    }
    for old in before.artboards.iter().filter(|b| b.id != second) {
        assert_eq!(st.doc.artboards.iter().find(|b| b.id == old.id).unwrap(), old);
    }
    assert_eq!(st.selection, selection);
    assert_eq!(st.history.undo.len(), undo + 1);
    assert_eq!(h.app.view().unwrap().artboard, active);
    let edited_json = serde_json::to_value(st.doc.as_ref()).unwrap();
    h.app.run("edit.undo", json!({})).unwrap();
    assert_eq!(serde_json::to_value(h.app.session.active().unwrap().doc.as_ref()).unwrap(), before_json);
    h.app.run("edit.redo", json!({})).unwrap();
    assert_eq!(serde_json::to_value(h.app.session.active().unwrap().doc.as_ref()).unwrap(), edited_json);
    let mut replay = Session::new();
    for (command, params) in &h.app.session.journal {
        replay.execute(command, params).unwrap();
    }
    assert_eq!(serde_json::to_value(replay.active().unwrap().doc.as_ref()).unwrap(), edited_json);
}

#[test]
fn routing_extension_properties_resize_after_external_delete_follows_active_board() {
    let (mut h, art) = round3_artboard_fixture(true);
    let second = h.app.session.active().unwrap().doc.artboards[1].id;
    h.app.run("artboard.delete", json!({"index":0})).unwrap();
    assert_eq!(h.app.session.tool_options()["active"], 0, "Second survived at index zero");
    round3_resize_second_and_check(&mut h, &art, second);
}

// A temporary tab can recreate the tool while the final active document UID stays the same.
// Assert the retained navigator before any frame or canvas dispatch can reconcile it.
#[test]
fn routing_extension_temporary_tab_batch_preserves_original_navigator() {
    let (mut h, _) = round3_artboard_fixture(false);
    let before = h.app.session.active().unwrap().clone();
    let original = serde_json::to_value(before.doc.as_ref()).unwrap();
    let second = before.doc.artboards[1].id;
    assert_eq!(before.doc.artboards[1].name, "Second");
    assert_eq!(h.app.session.documents().len(), 1);
    assert_eq!(h.app.view().unwrap().artboard, 1, "positive control: navigator selects Second");
    assert_eq!(h.app.session.tool_options()["active"], 1, "positive control: tool also selects Second");
    assert!(!h.app.session.tool_busy() && !h.app.session.in_interaction());

    let out = h
        .app
        .run(
            "command.batch",
            json!({"label":"Temporary tab", "commands":[
                {"command":"file.new", "params":{"width":100,"height":80,"created":2_000_000_000}},
                {"command":"file.close", "params":{}}
            ]}),
        )
        .unwrap();
    assert_eq!(out["results"].as_array().unwrap().len(), 2, "both normal batch steps completed");
    assert_eq!(h.app.session.documents().len(), 1);
    let current = h.app.session.active().unwrap();
    assert_eq!(current.uid, before.uid, "the original document is active again");
    assert_eq!(serde_json::to_value(current.doc.as_ref()).unwrap(), original, "temporary tab leaves original art unchanged");
    assert_eq!(current.history.undo.len(), before.history.undo.len());
    assert_eq!(current.history.redo.len(), before.history.redo.len());
    assert!(!h.app.session.tool_busy() && !h.app.session.in_interaction());
    assert_eq!(h.app.view().unwrap().artboard, 1, "temporary tab batch must retain navigator Second");
    assert_eq!(current.doc.artboards[h.app.view().unwrap().artboard].id, second);
}
