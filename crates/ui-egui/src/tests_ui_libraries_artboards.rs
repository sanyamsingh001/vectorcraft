//! Shared UI routing for swatch libraries (#536) and artboard editing (#530).

use egui::{Event, Pos2, Rect, Shape, Ui, pos2, vec2};
use serde_json::json;
use vectorcraft_engine::Session;

use crate::{VectorcraftApp, menus, panels};

struct Harness {
    app: VectorcraftApp,
    ctx: egui::Context,
    time: f64,
}

impl Harness {
    fn new() -> Self {
        let mut app = VectorcraftApp::new(Session::new(), Default::default());
        app.run("file.new", json!({"width": 200, "height": 150})).unwrap();
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        Self { app, ctx, time: 0.0 }
    }

    fn frame(&mut self, events: Vec<Event>, size: egui::Vec2, draw: fn(&mut VectorcraftApp, &mut Ui)) -> egui::FullOutput {
        self.time += 0.1;
        let mut input =
            egui::RawInput { events, time: Some(self.time), screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)), ..Default::default() };
        self.app.raw_input_hook(&mut input);
        let mut out = self.ctx.run_ui(input, |ui| draw(&mut self.app, ui));
        out.textures_delta.clear();
        out
    }

    fn settle(&mut self, size: egui::Vec2, draw: fn(&mut VectorcraftApp, &mut Ui)) -> egui::FullOutput {
        self.frame(vec![], size, draw);
        self.frame(vec![], size, draw)
    }

    fn click(&mut self, at: Pos2, size: egui::Vec2, draw: fn(&mut VectorcraftApp, &mut Ui)) -> egui::FullOutput {
        let button = |pressed| Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
        self.frame(vec![Event::PointerMoved(at)], size, draw);
        self.frame(vec![button(true)], size, draw);
        self.frame(vec![button(false)], size, draw);
        self.settle(size, draw)
    }
}

fn text_at(out: &egui::FullOutput, label: &str) -> Pos2 {
    fn find(shape: &Shape, label: &str) -> Option<Pos2> {
        match shape {
            Shape::Text(t) if t.galley.text().trim() == label => Some(t.pos + vec2(4.0, 4.0)),
            Shape::Vec(v) => v.iter().find_map(|s| find(s, label)),
            _ => None,
        }
    }
    out.shapes.iter().find_map(|s| find(&s.shape, label)).unwrap_or_else(|| panic!("missing {label}: {}", crate::tests_labels::shapes_text(out)))
}

/// Locate actual clickable widgets after egui has placed the floating area.
fn widgets_in(ctx: &egui::Context, area: Rect, size: egui::Vec2) -> Vec<Rect> {
    ctx.viewport(|vp| {
        vp.prev_pass
            .widgets
            .layers()
            .flat_map(|(_, w)| w.iter())
            .filter(|w| w.rect.size() == size && area.contains(w.rect.center()))
            .map(|w| w.rect)
            .collect()
    })
}

fn library_button(ctx: &egui::Context, area: Rect) -> Pos2 {
    let mut buttons = widgets_in(ctx, area, vec2(24.0, 24.0));
    buttons.sort_by(|a, b| b.top().total_cmp(&a.top()).then(a.left().total_cmp(&b.left())));
    buttons.first().expect("the Swatches bottom bar has a library button").center()
}

fn swatches_panel(app: &mut VectorcraftApp, ui: &mut Ui) {
    panels::swatches::show(app, ui);
    panels::library_panel::show_window(app, ui.ctx());
}

fn assert_library_visible(h: &Harness) {
    let id = panels::library_panel::tile_id::<panels::swatches::SwatchLibraries>("#000000");
    let tile = h.ctx.read_response(id).expect("the opened palette renders its swatches");
    assert!(tile.rect.is_positive() && h.ctx.content_rect().intersects(tile.rect));
}

fn menu_bar(app: &mut VectorcraftApp, ui: &mut Ui) {
    menus::menu_bar(app, ui);
    panels::library_panel::show_window(app, ui.ctx());
}

#[test]
fn window_swatch_libraries_menu_opens_the_selected_library() {
    let mut h = Harness::new();
    let size = vec2(1200.0, 1600.0);
    let out = h.settle(size, menu_bar);
    let window = text_at(&out, "Window");
    let out = h.click(window, size, menu_bar);
    let libraries = text_at(&out, "Swatch Libraries");
    let out = h.click(libraries, size, menu_bar);
    let library = text_at(&out, "Web Safe 216");
    assert!(menus::enabled(&h.app, "window.swatchLibrary"));
    h.click(library, size, menu_bar);
    assert_eq!(h.app.ui.library_panel.as_ref().map(|o| (o.kind.as_str(), o.id.as_str())), Some(("swatches", "web-safe-216")));
    assert_library_visible(&h);
}

#[test]
fn swatches_panel_library_button_opens_the_selected_library() {
    let mut h = Harness::new();
    let size = vec2(280.0, 700.0);
    let out = h.settle(size, swatches_panel);
    let t = crate::theme::Tokens::get(&h.ctx);
    let bar = out
        .shapes
        .iter()
        .filter_map(|s| match &s.shape {
            Shape::Rect(r) if r.fill == t.divider => Some(r.rect),
            _ => None,
        })
        .max_by(|a, b| a.top().total_cmp(&b.top()))
        .unwrap();
    let library_button = pos2(bar.left() + 12.0, bar.bottom() + 14.0);
    let out = h.click(library_button, size, swatches_panel);
    let library = text_at(&out, "Web Safe 216");
    h.click(library, size, swatches_panel);
    assert_eq!(h.app.ui.library_panel.as_ref().map(|o| o.id.as_str()), Some("web-safe-216"));
    assert_library_visible(&h);
}

#[test]
fn editing_artboards_offers_new_artboard_and_keeps_the_copy_active() {
    let mut h = Harness::new();
    let size = vec2(320.0, 700.0);
    let out = h.settle(size, panels::properties::show);
    let edit = text_at(&out, "Edit Artboards");
    let out = h.click(edit, size, panels::properties::show);
    assert_eq!(h.app.session.tool_id(), "artboard");
    let new = text_at(&out, "New Artboard");
    h.click(new, size, panels::properties::show);
    assert_eq!(h.app.session.active().unwrap().doc.artboards.len(), 2);
    assert_eq!(h.app.view().unwrap().artboard, 1);
    assert_eq!(h.app.session.tool_options()["active"], 1);
    assert_eq!(h.app.session.tool_id(), "artboard");
}

#[test]
fn artboards_panel_menu_new_artboard_becomes_active_while_editing() {
    let mut h = Harness::new();
    h.app.select_tool("artboard");
    let size = vec2(280.0, 700.0);
    let out = h.settle(size, panels::artboards::menu);
    let new = text_at(&out, "New Artboard");
    h.click(new, size, panels::artboards::menu);
    assert_eq!(h.app.session.active().unwrap().doc.artboards.len(), 2);
    assert_eq!(h.app.view().unwrap().artboard, 1);
    assert_eq!(h.app.session.tool_options()["active"], 1);
}

fn swatch_flyout(app: &mut VectorcraftApp, ui: &mut Ui) {
    crate::dock::show(app, ui);
    crate::dock::floating_panel(app, ui.ctx());
    panels::library_panel::show_window(app, ui.ctx());
}

#[test]
fn swatch_libraries_open_from_the_icon_panel_with_auto_collapse() {
    let mut h = Harness::new();
    h.app.ui.dock_collapsed = true;
    h.app.ui.open_panel = Some("swatches".into());
    h.app.run("prefs.set", json!({"key": "autoCollapseIconPanels", "value": true})).unwrap();
    let size = vec2(1200.0, 900.0);
    h.settle(size, swatch_flyout);
    let flyout = h.ctx.memory(|m| m.area_rect(egui::Id::new("icon-panel"))).expect("the collapsed Swatches panel is open");
    let out = h.click(library_button(&h.ctx, flyout), size, swatch_flyout);
    let library = text_at(&out, "Web Safe 216");
    h.click(library, size, swatch_flyout);
    assert_eq!(h.app.ui.library_panel.as_ref().map(|o| o.id.as_str()), Some("web-safe-216"));
    assert_library_visible(&h);
}

#[test]
fn drawing_an_artboard_on_the_pasteboard_creates_and_activates_it() {
    use vectorcraft_tools::{PointerEvent, PointerKind};
    let mut h = Harness::new();
    h.app.select_tool("artboard");
    let undo = h.app.session.active().unwrap().history.undo.len();
    let view = h.app.view_info();
    for ev in [
        PointerEvent::new(PointerKind::Down, 400.0, 300.0),
        PointerEvent::new(PointerKind::Drag, 500.0, 360.0),
        PointerEvent::new(PointerKind::Up, 500.0, 360.0),
    ] {
        crate::canvas::dispatch(&mut h.app, &ev, view);
    }
    let d = h.app.session.active().unwrap();
    assert_eq!(d.doc.artboards.len(), 2);
    assert_eq!(d.doc.artboards[1].rect, vectorcraft_geom::Rect::new(400.0, 300.0, 500.0, 360.0));
    assert_eq!(d.history.undo.len(), undo + 1);
    assert_eq!(h.app.view().unwrap().artboard, 1);
    assert_eq!(h.app.session.tool_options()["active"], 1);
    h.app.run("edit.undo", json!({})).unwrap();
    assert_eq!(h.app.session.active().unwrap().doc.artboards.len(), 1);
}

#[test]
fn edit_artboards_targets_the_artboard_in_the_navigator() {
    let mut h = Harness::new();
    h.app.run("artboard.new", json!({})).unwrap();
    h.app.view_mut().unwrap().artboard = 1;
    let size = vec2(320.0, 700.0);
    let out = h.settle(size, panels::properties::show);
    let edit = text_at(&out, "Edit Artboards");
    h.click(edit, size, panels::properties::show);
    assert_eq!(h.app.session.tool_id(), "artboard");
    assert_eq!(h.app.session.tool_options()["active"], 1);
}

fn dialogs(app: &mut VectorcraftApp, ui: &mut Ui) {
    crate::dialogs::show(app, ui.ctx());
}

#[test]
fn artboard_options_numeric_edits_target_the_active_board_and_cancel_rolls_back() {
    let mut h = Harness::new();
    h.app.run("artboard.new", json!({})).unwrap();
    h.app.view_mut().unwrap().artboard = 1;
    h.app.select_tool("artboard");
    assert_eq!(h.app.session.tool_options()["active"], 1);
    let size = vec2(900.0, 700.0);
    let open = |h: &mut Harness| {
        let i = h.app.session.tool_options()["active"].as_u64().unwrap() as usize;
        let ab = &h.app.session.active().unwrap().doc.artboards[i];
        let params = json!({"index": i, "name": ab.name, "x": ab.rect.x0, "y": ab.rect.y0, "width": ab.rect.width(), "height": ab.rect.height()});
        crate::dialogs::open_tool_dialog(&mut h.app, "artboardOptions", params);
    };
    open(&mut h);
    let undo = h.app.session.active().unwrap().history.undo.len();
    let out = h.settle(size, dialogs);
    h.click(text_at(&out, "200 pt"), size, dialogs);
    let out = h.frame(vec![Event::Text("25*2 mm".into())], size, dialogs);
    h.click(text_at(&out, "OK"), size, dialogs);
    assert!(h.app.ui.dialog.is_none());
    let d = h.app.session.active().unwrap();
    assert_eq!(d.doc.artboards[0].rect.width(), 200.0);
    let width = d.doc.artboards[1].rect.width();
    assert!((width - vectorcraft_doc::Unit::Millimeters.to_pt(50.0)).abs() < 1e-6);
    assert_eq!(d.history.undo.len(), undo + 1);
    open(&mut h);
    let out = h.settle(size, dialogs);
    let shown = vectorcraft_doc::Unit::Points.format(width);
    h.click(text_at(&out, &shown), size, dialogs);
    let out = h.frame(vec![Event::Text("300 pt".into())], size, dialogs);
    h.click(text_at(&out, "Cancel"), size, dialogs);
    assert!(h.app.ui.dialog.is_none());
    assert_eq!(h.app.session.active().unwrap().doc.artboards[1].rect.width(), width);
    assert_eq!(h.app.session.active().unwrap().history.undo.len(), undo + 1);
}

#[test]
fn new_artboard_remains_available_when_art_is_selected() {
    let mut h = Harness::new();
    h.app.run("shape.rectangle", json!({"x": 10, "y": 10, "width": 20, "height": 30})).unwrap();
    let selected = h.app.session.active().unwrap().selection.clone();
    h.app.select_tool("artboard");
    let size = vec2(320.0, 700.0);
    let out = h.settle(size, panels::properties::show);
    h.click(text_at(&out, "New Artboard"), size, panels::properties::show);
    assert_eq!(h.app.session.active().unwrap().doc.artboards.len(), 2);
    assert_eq!(h.app.view().unwrap().artboard, 1);
    assert_eq!(h.app.session.tool_options()["active"], 1);
    assert_eq!(h.app.session.active().unwrap().selection.objects, selected.objects);
}

#[test]
fn library_popup_dismisses_and_the_palette_can_be_closed_and_reopened() {
    let mut h = Harness::new();
    let size = vec2(1200.0, 900.0);
    h.app.run("window.swatchLibrary", json!({"library": "web-safe-216"})).unwrap();
    h.settle(size, swatches_panel);
    assert_library_visible(&h);
    let area = h.ctx.memory(|m| m.area_rect(egui::Id::new(("library-panel", "swatches")))).unwrap();
    let close = widgets_in(&h.ctx, area, vec2(14.0, 14.0)).into_iter().next().expect("the palette close button");
    h.click(close.center(), size, swatches_panel);
    assert!(h.app.ui.library_panel.is_none());
    let out = h.settle(size, swatches_panel);
    let t = crate::theme::Tokens::get(&h.ctx);
    let bar = out
        .shapes
        .iter()
        .filter_map(|s| match &s.shape {
            Shape::Rect(r) if r.fill == t.divider => Some(r.rect),
            _ => None,
        })
        .max_by(|a, b| a.top().total_cmp(&b.top()))
        .unwrap();
    let out = h.click(pos2(bar.left() + 12.0, bar.bottom() + 14.0), size, swatches_panel);
    h.click(text_at(&out, "Web Safe 216"), size, swatches_panel);
    assert!(!egui::Popup::is_any_open(&h.ctx));
    for _ in 0..3 {
        h.frame(vec![], size, swatches_panel);
        assert_library_visible(&h);
        assert_eq!(h.app.ui.library_panel.as_ref().unwrap().id, "web-safe-216");
    }
}

fn native_library(app: &mut VectorcraftApp, ui: &mut Ui) {
    crate::native_menu::run(app, ui.ctx());
    panels::library_panel::show_window(app, ui.ctx());
}

#[test]
fn shared_native_menu_library_click_opens_the_palette_once() {
    use crate::native_menu::{Backend, MenuBar, NativeMenu};
    struct Fake(Vec<crate::native_menu::Event>);
    impl Backend for Fake {
        fn sync(&mut self, _: &MenuBar) {}
        fn drain(&mut self) -> Vec<crate::native_menu::Event> {
            std::mem::take(&mut self.0)
        }
    }
    let mut h = Harness::new();
    let bar = crate::native_menu::layout(&h.app).bar;
    let item =
        bar.items().into_iter().find(|it| it.command == Some("window.swatchLibrary") && it.params["library"] == "web-safe-216").unwrap().clone();
    assert!(item.enabled);
    h.app.services.native_menu = Some(NativeMenu::new(Box::new(Fake(vec![crate::native_menu::Event::Click(item)]))));
    let before = h.app.run_count;
    let size = vec2(1200.0, 900.0);
    h.settle(size, native_library);
    assert_library_visible(&h);
    assert_eq!(h.app.ui.library_panel.as_ref().unwrap().id, "web-safe-216");
    h.settle(size, native_library);
    assert_eq!(h.app.run_count, before + 1);
}
