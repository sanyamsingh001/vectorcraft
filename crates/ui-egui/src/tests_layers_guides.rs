//! Original small scenes and actual headless pointer events for #451's Layers guide rows.
//! Hooked as a child of panels::layers so no crate-root/module ownership is needed.

use egui::{Event, Modifiers, Pos2, Shape, pos2};
use vectorcraft_engine::Session;

use super::*;

fn scene() -> (VectorcraftApp, NodeId) {
    let mut app = VectorcraftApp::new(Session::new(), Default::default());
    app.run("file.new", json!({"width": 320, "height": 240})).unwrap();
    let art = NodeId(app.run("shape.rectangle", json!({"x": 10, "y": 10, "width": 20, "height": 30})).unwrap()["id"].as_u64().unwrap());
    app.run("select.none", json!({})).unwrap();
    for (vertical, pos) in [(true, 40), (true, 80), (false, 120)] {
        app.run("guide.add", json!({"vertical": vertical, "pos": pos})).unwrap();
    }
    (app, art)
}

fn frame_size(app: &mut VectorcraftApp, ctx: &egui::Context, events: Vec<Event>, mods: Modifiers, size: egui::Vec2) -> egui::FullOutput {
    let mut events = events;
    events.insert(0, Event::ModifiersChanged(mods));
    let mut out = ctx
        .run_ui(egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(Pos2::ZERO, size)), events, ..Default::default() }, |ui| show(app, ui));
    out.textures_delta.clear();
    out
}

fn frame(app: &mut VectorcraftApp, ctx: &egui::Context, events: Vec<Event>, mods: Modifiers) -> egui::FullOutput {
    frame_size(app, ctx, events, mods, vec2(300.0, 480.0))
}

fn text_positions(out: &egui::FullOutput, name: &str) -> Vec<Pos2> {
    let mut positions = out
        .shapes
        .iter()
        .filter_map(|s| match &s.shape {
            Shape::Text(t) if t.galley.text().trim() == name => Some(t.pos + vec2(0.0, t.galley.size().y / 2.0)),
            _ => None,
        })
        .collect::<Vec<_>>();
    positions.sort_by(|a, b| a.y.total_cmp(&b.y));
    positions
}

fn guide_positions(out: &egui::FullOutput) -> Vec<Pos2> {
    let group = text_positions(out, crate::i18n::tr(crate::i18n::current(), "Guides"))[0];
    text_positions(out, &guide_name()).into_iter().filter(|p| p.y > group.y).collect()
}

fn button(at: Pos2, pressed: bool, mods: Modifiers) -> Event {
    Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: mods }
}

fn click(app: &mut VectorcraftApp, ctx: &egui::Context, at: Pos2, mods: Modifiers) {
    for e in [Event::PointerMoved(at), button(at, true, mods), button(at, false, mods)] {
        frame(app, ctx, vec![e], mods);
    }
}

fn drag(app: &mut VectorcraftApp, ctx: &egui::Context, from: Pos2, to: Pos2) {
    for events in [
        vec![Event::PointerMoved(from)],
        vec![button(from, true, Modifiers::NONE)],
        vec![Event::PointerMoved(from + vec2(0.0, 6.0))],
        vec![Event::PointerMoved(to)],
        vec![button(to, false, Modifiers::NONE)],
        vec![],
    ] {
        frame(app, ctx, events, Modifiers::NONE);
    }
}

fn trash(out: &egui::FullOutput, ctx: &egui::Context) -> Pos2 {
    let divider = Tokens::get(ctx).divider;
    let corner = out
        .shapes
        .iter()
        .rev()
        .find_map(|s| match &s.shape {
            Shape::LineSegment { points, stroke } if stroke.color == divider => Some(points[1]),
            _ => None,
        })
        .unwrap();
    corner + vec2(-12.0, 15.0)
}

fn selected(app: &VectorcraftApp) -> Vec<usize> {
    app.session.active().unwrap().selection.guides.clone()
}

fn flags(app: &VectorcraftApp) -> Vec<(bool, bool)> {
    app.session.active().unwrap().doc.guides.iter().map(|g| (g.visible, g.locked)).collect()
}

#[test]
fn ruler_guide_eyes_and_locks_are_individual_and_undoable() {
    let (mut app, art) = scene();
    let ctx = egui::Context::default();
    let out = frame(&mut app, &ctx, vec![], Modifiers::NONE);
    let rows = guide_positions(&out);
    assert_eq!(rows.len(), 3);
    click(&mut app, &ctx, rows[0] + vec2(12.0, 0.0), Modifiers::NONE);
    assert_eq!(selected(&app), [0]);
    let before = app.session.active().unwrap().history.undo.len();
    click(&mut app, &ctx, pos2(1.5 * COLUMN, rows[0].y), Modifiers::NONE);
    assert_eq!(flags(&app), [(true, true), (true, false), (true, false)]);
    assert!(selected(&app).is_empty(), "locking drops the guide from selection");
    assert_eq!(app.session.active().unwrap().history.undo.len(), before + 1);
    app.run("edit.undo", json!({})).unwrap();
    assert_eq!(selected(&app), [0]);
    assert_eq!(flags(&app), [(true, false); 3]);
    app.run("edit.redo", json!({})).unwrap();
    assert!(selected(&app).is_empty());
    click(&mut app, &ctx, pos2(1.5 * COLUMN, rows[0].y), Modifiers::NONE);
    click(&mut app, &ctx, rows[1] + vec2(12.0, 0.0), Modifiers::NONE);
    assert_eq!(selected(&app), [1]);
    click(&mut app, &ctx, pos2(0.5 * COLUMN, rows[1].y), Modifiers::NONE);
    assert!(selected(&app).is_empty(), "hiding drops the guide from selection");
    assert_eq!(flags(&app), [(true, false), (false, false), (true, false)]);
    app.run("edit.undo", json!({})).unwrap();
    assert_eq!(flags(&app), [(true, false); 3]);
    assert_eq!(selected(&app), [1]);
    app.run("edit.redo", json!({})).unwrap();
    assert!(!app.session.active().unwrap().doc.guides[1].visible);
    click(&mut app, &ctx, rows[1] + vec2(12.0, 0.0), Modifiers::NONE);
    assert!(selected(&app).is_empty(), "hidden rows cannot be selected");
    let node = app.session.active().unwrap().doc.node(art).unwrap();
    assert!(node.visible && !node.locked, "the columns did not touch artwork");
}

#[test]
fn guide_row_selection_range_toggle_and_global_controls() {
    let (mut app, _) = scene();
    let ctx = egui::Context::default();
    let out = frame(&mut app, &ctx, vec![], Modifiers::NONE);
    let rows = guide_positions(&out);
    let group = text_positions(&out, crate::i18n::tr(crate::i18n::current(), "Guides"))[0];
    click(&mut app, &ctx, rows[0] + vec2(12.0, 0.0), Modifiers::NONE);
    click(&mut app, &ctx, rows[2] + vec2(12.0, 0.0), Modifiers::COMMAND);
    assert_eq!(selected(&app), [0, 2]);
    click(&mut app, &ctx, rows[2] + vec2(12.0, 0.0), Modifiers::COMMAND);
    assert_eq!(selected(&app), [0]);
    app.run("guide.setProps", json!({"index": 1, "locked": true})).unwrap();
    click(&mut app, &ctx, rows[2] + vec2(12.0, 0.0), Modifiers::SHIFT);
    assert_eq!(selected(&app), [0, 2], "the range skips the protected middle guide");
    click(&mut app, &ctx, rows[1] + vec2(12.0, 0.0), Modifiers::NONE);
    assert_eq!(selected(&app), [0, 2], "a locked row cannot be selected");
    // The existing rightmost selection column selects one guide, just like a Node row's art.
    click(&mut app, &ctx, pos2(300.0 - SQUARE_X, rows[2].y), Modifiers::NONE);
    assert_eq!(selected(&app), [2]);
    assert!(app.session.active().unwrap().highlighted_rows().is_empty());
    click(&mut app, &ctx, pos2(1.5 * COLUMN, group.y), Modifiers::NONE);
    assert!(app.session.guides_locked());
    assert!(selected(&app).is_empty());
    click(&mut app, &ctx, rows[0] + vec2(12.0, 0.0), Modifiers::NONE);
    assert!(selected(&app).is_empty());
    click(&mut app, &ctx, pos2(1.5 * COLUMN, rows[1].y), Modifiers::NONE);
    assert!(!app.session.active().unwrap().doc.guides[1].locked, "a protected row can be unlocked under the global lock");
    assert!(app.session.guides_locked(), "the individual setter does not release the global lock");
    click(&mut app, &ctx, pos2(1.5 * COLUMN, group.y), Modifiers::NONE);
    click(&mut app, &ctx, pos2(0.5 * COLUMN, group.y), Modifiers::NONE);
    assert!(!app.ui.view.guides);
    assert_eq!(flags(&app), [(true, false); 3], "global hide preserves individual flags");
    click(&mut app, &ctx, rows[0] + vec2(12.0, 0.0), Modifiers::NONE);
    assert!(selected(&app).is_empty());
    click(&mut app, &ctx, pos2(0.5 * COLUMN, group.y), Modifiers::NONE);
    click(&mut app, &ctx, rows[0] + vec2(12.0, 0.0), Modifiers::NONE);
    assert_eq!(selected(&app), [0]);
}

#[test]
fn trash_prefers_guides_over_stale_node_highlights_and_remaps_on_undo() {
    let (mut app, art) = scene();
    app.run("layer.setCurrent", json!({"id": art.0})).unwrap();
    app.run("guide.select", json!({"indexes": [1]})).unwrap();
    assert_eq!(app.session.active().unwrap().highlighted_rows(), [art], "a canvas/command selection can leave Node highlights");
    let ctx = egui::Context::default();
    let out = frame(&mut app, &ctx, vec![], Modifiers::NONE);
    let before = app.session.active().unwrap().history.undo.len();
    click(&mut app, &ctx, trash(&out, &ctx), Modifiers::NONE);
    assert_eq!(app.session.active().unwrap().doc.guides.iter().map(|g| g.pos).collect::<Vec<_>>(), [40.0, 120.0]);
    assert!(app.session.active().unwrap().doc.node(art).is_some());
    assert!(selected(&app).is_empty());
    assert_eq!(app.session.active().unwrap().history.undo.len(), before + 1);
    app.run("edit.undo", json!({})).unwrap();
    assert_eq!(selected(&app), [1]);
    assert_eq!(flags(&app).len(), 3);
    app.run("edit.redo", json!({})).unwrap();
    assert_eq!(flags(&app).len(), 2);
    // After deletion index 1 is the old index 2. Its row controls must address that survivor.
    let rows = guide_positions(&frame(&mut app, &ctx, vec![], Modifiers::NONE));
    click(&mut app, &ctx, pos2(0.5 * COLUMN, rows[1].y), Modifiers::NONE);
    let doc = &app.session.active().unwrap().doc;
    assert_eq!((doc.guides[1].pos, doc.guides[1].visible), (120.0, false));
    assert!(doc.guides[0].visible);
}

#[test]
fn dragging_selected_guide_rows_to_trash_is_one_delete_step() {
    let (mut app, art) = scene();
    let ctx = egui::Context::default();
    let out = frame(&mut app, &ctx, vec![], Modifiers::NONE);
    let rows = guide_positions(&out);
    click(&mut app, &ctx, rows[0] + vec2(12.0, 0.0), Modifiers::NONE);
    click(&mut app, &ctx, rows[2] + vec2(12.0, 0.0), Modifiers::COMMAND);
    let before = app.session.active().unwrap().history.undo.len();
    drag(&mut app, &ctx, rows[0] + vec2(12.0, 0.0), trash(&out, &ctx));
    let st = app.session.active().unwrap();
    assert_eq!(st.doc.guides.iter().map(|g| g.pos).collect::<Vec<_>>(), [80.0]);
    assert!(st.doc.node(art).is_some());
    assert_eq!(st.history.undo.len(), before + 1);
    app.run("edit.undo", json!({})).unwrap();
    assert_eq!(selected(&app), [0, 2]);
    assert_eq!(flags(&app).len(), 3);
    let out = frame(&mut app, &ctx, vec![], Modifiers::NONE);
    drag(&mut app, &ctx, pos2(300.0 - SQUARE_X, rows[2].y), trash(&out, &ctx));
    assert_eq!(app.session.active().unwrap().doc.guides.iter().map(|g| g.pos).collect::<Vec<_>>(), [80.0]);
    app.run("edit.undo", json!({})).unwrap();
    assert_eq!(selected(&app), [0, 2], "the selection square carries the same guide rows to trash");
}

#[test]
fn keyboard_and_guide_context_menu_delete_use_the_guide_command() {
    let (mut app, art) = scene();
    let ctx = egui::Context::default();
    let rows = guide_positions(&frame(&mut app, &ctx, vec![], Modifiers::NONE));
    click(&mut app, &ctx, rows[1] + vec2(12.0, 0.0), Modifiers::NONE);
    let delete = Event::Key { key: egui::Key::Delete, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE };
    frame(&mut app, &ctx, vec![delete], Modifiers::NONE);
    assert_eq!(flags(&app).len(), 2);
    assert!(app.session.active().unwrap().doc.node(art).is_some());
    app.run("edit.undo", json!({})).unwrap();
    let at = rows[2] + vec2(12.0, 0.0);
    for e in [
        Event::PointerMoved(at),
        Event::PointerButton { pos: at, button: egui::PointerButton::Secondary, pressed: true, modifiers: Modifiers::NONE },
        Event::PointerButton { pos: at, button: egui::PointerButton::Secondary, pressed: false, modifiers: Modifiers::NONE },
    ] {
        frame(&mut app, &ctx, vec![e], Modifiers::NONE);
    }
    // Popups need a layout frame before their pointer targets are stable.
    frame(&mut app, &ctx, vec![], Modifiers::NONE);
    let out = frame(&mut app, &ctx, vec![], Modifiers::NONE);
    let label = crate::i18n::tr(crate::i18n::current(), "Delete Selection");
    let menu = text_positions(&out, label).into_iter().next().unwrap();
    click(&mut app, &ctx, menu + vec2(12.0, 0.0), Modifiers::NONE);
    assert_eq!(app.session.active().unwrap().doc.guides.iter().map(|g| g.pos).collect::<Vec<_>>(), [40.0, 80.0]);
    assert!(app.session.active().unwrap().doc.node(art).is_some());
}

#[test]
fn dragging_guide_columns_groups_the_flags_and_preserves_node_rows() {
    for eye in [true, false] {
        let (mut app, art) = scene();
        let ctx = egui::Context::default();
        let rows = guide_positions(&frame(&mut app, &ctx, vec![], Modifiers::NONE));
        let x = COLUMN * if eye { 0.5 } else { 1.5 };
        let before = app.session.active().unwrap().history.undo.len();
        for events in [
            vec![Event::PointerMoved(pos2(x, rows[0].y))],
            vec![button(pos2(x, rows[0].y), true, Modifiers::NONE)],
            vec![Event::PointerMoved(pos2(x, rows[0].y + 6.0))],
            vec![Event::PointerMoved(pos2(x, rows[1].y))],
            vec![Event::PointerMoved(pos2(x, rows[2].y))],
            vec![button(pos2(x, rows[2].y), false, Modifiers::NONE)],
            vec![],
        ] {
            frame(&mut app, &ctx, events, Modifiers::NONE);
        }
        assert_eq!(flags(&app), if eye { [(false, false); 3] } else { [(true, true); 3] });
        assert_eq!(app.session.active().unwrap().history.undo.len(), before + 1);
        assert!(!app.session.in_interaction());
        app.run("edit.undo", json!({})).unwrap();
        assert_eq!(flags(&app), [(true, false); 3]);
        let node = app.session.active().unwrap().doc.node(art).unwrap();
        assert!(node.visible && !node.locked);
    }
}

#[test]
fn converted_path_guides_keep_the_legacy_node_columns() {
    let (mut app, art) = scene();
    app.run("select.set", json!({"ids": [art.0]})).unwrap();
    app.run("view.guides.make", json!({})).unwrap();
    app.run("select.none", json!({})).unwrap();
    let ctx = egui::Context::default();
    let out = frame(&mut app, &ctx, vec![], Modifiers::NONE);
    let mut circles = out
        .shapes
        .iter()
        .filter_map(|s| match &s.shape {
            Shape::Circle(c) if c.radius == 5.0 => Some(c.center),
            _ => None,
        })
        .collect::<Vec<_>>();
    circles.sort_by(|a, b| a.y.total_cmp(&b.y));
    assert_eq!(circles.len(), 2, "only the layer and converted Path have Node target circles");
    click(&mut app, &ctx, pos2(0.5 * COLUMN, circles[1].y), Modifiers::NONE);
    assert!(!app.session.active().unwrap().doc.node(art).unwrap().visible);
    assert_eq!(flags(&app), [(true, false); 3]);
    app.run("edit.undo", json!({})).unwrap();
    click(&mut app, &ctx, pos2(1.5 * COLUMN, circles[1].y), Modifiers::NONE);
    assert!(app.session.active().unwrap().doc.node(art).unwrap().locked);
    assert_eq!(flags(&app), [(true, false); 3]);
    app.run("edit.undo", json!({})).unwrap();
    let rows = guide_positions(&frame(&mut app, &ctx, vec![], Modifiers::NONE));
    click(&mut app, &ctx, rows[0] + vec2(12.0, 0.0), Modifiers::NONE);
    assert_eq!(selected(&app), [0]);
    click(&mut app, &ctx, pos2(150.0, circles[1].y), Modifiers::NONE);
    assert!(selected(&app).is_empty(), "clicking a Node row leaves guide selection");
    assert_eq!(app.session.active().unwrap().highlighted_rows(), [art]);
    let out = frame(&mut app, &ctx, vec![], Modifiers::NONE);
    click(&mut app, &ctx, trash(&out, &ctx), Modifiers::NONE);
    assert!(app.session.active().unwrap().doc.node(art).is_none());
    assert_eq!(flags(&app).len(), 3, "ordinary Node trash deletion preserves ruler guides");
}

#[test]
fn guide_group_respects_panel_filter_layers_only_and_narrow_clipping() {
    let (mut app, _) = scene();
    let ctx = egui::Context::default();
    let out = frame(&mut app, &ctx, vec![], Modifiers::NONE);
    let group = text_positions(&out, crate::i18n::tr(crate::i18n::current(), "Guides"))[0];
    click(&mut app, &ctx, pos2(2.0 * COLUMN + 15.0, group.y), Modifiers::NONE);
    assert!(guide_positions(&frame(&mut app, &ctx, vec![], Modifiers::NONE)).is_empty());
    // The ordinary search widget supplies the filter and opens a matching collapsed group.
    click(&mut app, &ctx, pos2(60.0, 12.0), Modifiers::NONE);
    let out = frame(&mut app, &ctx, vec![Event::Text("guide".into())], Modifiers::NONE);
    assert_eq!(guide_positions(&out).len(), 3);
    app.ui.layers_panel.layers_only = true;
    let out = frame(&mut app, &ctx, vec![], Modifiers::NONE);
    assert!(text_positions(&out, &guide_name()).is_empty());
    app.ui.layers_panel.layers_only = false;
    let out = frame_size(&mut app, &ctx, vec![], Modifiers::NONE, vec2(150.0, 480.0));
    let clips = out
        .shapes
        .iter()
        .filter_map(|s| match &s.shape {
            Shape::Text(t) if t.galley.text() == guide_name() => Some(s.clip_rect),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(clips.len(), 3);
    assert!(clips.iter().all(|r| r.right() <= 150.0 - 44.0 && r.width() < 30.0), "guide names stay clear of the right columns: {clips:?}");
}

#[test]
fn offscreen_guide_rows_are_virtualized_and_scrolled_rows_select_their_current_index() {
    let (mut app, _) = scene();
    for i in 3..48 {
        app.run("guide.add", json!({"vertical": true, "pos": i * 5 + 1})).unwrap();
    }
    let ctx = egui::Context::default();
    let size = vec2(300.0, 220.0);
    let out = frame_size(&mut app, &ctx, vec![], Modifiers::NONE, size);
    let rows = guide_positions(&out);
    assert!(!rows.is_empty() && rows.len() < 10, "only the visible range is painted: {}", rows.len());
    let wheel =
        Event::MouseWheel { unit: egui::MouseWheelUnit::Point, delta: vec2(0.0, -300.0), modifiers: Modifiers::NONE, phase: egui::TouchPhase::Move };
    frame_size(&mut app, &ctx, vec![Event::PointerMoved(rows[0]), wheel], Modifiers::NONE, size);
    // Settle the existing ScrollArea's smooth wheel movement without wall-clock waits.
    let mut out = frame_size(&mut app, &ctx, vec![], Modifiers::NONE, size);
    for _ in 0..10 {
        out = frame_size(&mut app, &ctx, vec![], Modifiers::NONE, size);
    }
    let at = text_positions(&out, &guide_name()).into_iter().find(|p| p.y > 50.0 && p.y < 160.0).unwrap() + vec2(12.0, 0.0);
    for e in [Event::PointerMoved(at), button(at, true, Modifiers::NONE), button(at, false, Modifiers::NONE)] {
        frame_size(&mut app, &ctx, vec![e], Modifiers::NONE, size);
    }
    let selected = selected(&app);
    assert_eq!(selected.len(), 1);
    assert!(selected[0] > 2 && selected[0] < 48, "the scrolled row keeps its engine index: {selected:?}");
}

#[test]
fn layers_menu_delete_prefers_guides_over_stale_node_highlights() {
    let (mut app, art) = scene();
    app.run("layer.setCurrent", json!({"id": art.0})).unwrap();
    app.run("guide.select", json!({"indexes": [0]})).unwrap();
    let ctx = egui::Context::default();
    let mut menu_frame = |events: Vec<Event>| {
        let mut out = ctx.run_ui(egui::RawInput { events, ..Default::default() }, |ui| menu(&mut app, ui));
        out.textures_delta.clear();
        out
    };
    let out = menu_frame(vec![]);
    let label = crate::i18n::tr(crate::i18n::current(), "Delete Selection");
    let at = text_positions(&out, label)[0] + vec2(12.0, 0.0);
    for e in [Event::PointerMoved(at), button(at, true, Modifiers::NONE), button(at, false, Modifiers::NONE)] {
        menu_frame(vec![e]);
    }
    assert_eq!(app.session.active().unwrap().doc.guides.iter().map(|g| g.pos).collect::<Vec<_>>(), [80.0, 120.0]);
    assert!(app.session.active().unwrap().doc.node(art).is_some());
    app.run("edit.undo", json!({})).unwrap();
    assert_eq!(flags(&app).len(), 3);
}

// Regressions from the source-only guide Layers review. Existing cases above stay intact.

const REVIEW_PATH_NAME: &str = "Original path guide";

fn review_mixed_scene() -> (VectorcraftApp, NodeId) {
    let (mut app, art) = scene();
    app.run("layer.setProps", json!({"ids": [art.0], "name": REVIEW_PATH_NAME})).unwrap();
    app.run("select.set", json!({"ids": [art.0]})).unwrap();
    app.run("view.guides.make", json!({})).unwrap();
    app.run("select.none", json!({})).unwrap();
    (app, art)
}

fn review_positions(app: &VectorcraftApp) -> Vec<f64> {
    app.session.active().unwrap().doc.guides.iter().map(|g| g.pos).collect()
}

fn review_key(key: egui::Key, pressed: bool, modifiers: Modifiers) -> Event {
    Event::Key { key, physical_key: None, pressed, repeat: false, modifiers }
}

/// Whole-app ordering matters here: shortcuts run before the dock's Layers panel.
struct ReviewWholeApp {
    app: VectorcraftApp,
    ctx: egui::Context,
    time: f64,
}

impl ReviewWholeApp {
    fn new(mut app: VectorcraftApp) -> Self {
        app.ui.dock = true;
        app.ui.dock_collapsed = false;
        app.ui.dock_tab = crate::state::DockTab::Layers;
        let mut h = Self { app, ctx: egui::Context::default(), time: 0.0 };
        for _ in 0..3 {
            h.frame(vec![], Modifiers::NONE);
        }
        h
    }

    fn frame(&mut self, mut events: Vec<Event>, mods: Modifiers) -> egui::FullOutput {
        self.time += 0.1;
        events.insert(0, Event::ModifiersChanged(mods));
        let mut out = self.ctx.run_ui(
            egui::RawInput {
                events,
                time: Some(self.time),
                screen_rect: Some(egui::Rect::from_min_size(Pos2::ZERO, vec2(1600.0, 850.0))),
                ..Default::default()
            },
            |ui| {
                self.app.logic(ui.ctx());
                self.app.ui(ui);
            },
        );
        out.textures_delta.clear();
        out
    }

    fn click(&mut self, at: Pos2) {
        for e in [Event::PointerMoved(at), button(at, true, Modifiers::NONE), button(at, false, Modifiers::NONE)] {
            self.frame(vec![e], Modifiers::NONE);
        }
    }

    fn key(&mut self, key: egui::Key, mods: Modifiers) {
        self.frame(vec![review_key(key, true, mods)], mods);
        self.frame(vec![review_key(key, false, mods)], mods);
    }

    fn search_point(&self) -> Pos2 {
        let id = egui::Id::new("layers-search").with("edit");
        self.ctx
            .viewport(|vp| vp.prev_pass.widgets.layers().flat_map(|(_, w)| w.iter()).find(|w| w.id == id).map(|w| w.rect.center()))
            .expect("the whole-app Layers search field is laid out")
    }
}

fn review_highlighted_path_key(key: egui::Key) {
    let (app, path) = review_mixed_scene();
    let mut h = ReviewWholeApp::new(app);
    let out = h.frame(vec![], Modifiers::NONE);
    let guides = guide_positions(&out);
    assert_eq!(guides.len(), 3);
    let node = text_positions(&out, REVIEW_PATH_NAME)[0];
    h.click(guides[0] + vec2(12.0, 0.0));
    assert_eq!(selected(&h.app), [0]);
    assert!(h.app.session.active().unwrap().highlighted_rows().is_empty());
    h.click(node + vec2(12.0, 0.0));
    let st = h.app.session.active().unwrap();
    assert!(st.selection.objects.is_empty() && st.selection.guides.is_empty());
    assert_eq!(st.highlighted_rows(), [path]);
    assert!(!h.ctx.egui_wants_keyboard_input());
    let before = st.history.undo.len();
    h.key(key, Modifiers::NONE);
    let st = h.app.session.active().unwrap();
    assert!(st.doc.node(path).is_none(), "{key:?} must remove the highlighted converted path-guide row");
    assert_eq!(review_positions(&h.app), [40.0, 80.0, 120.0]);
    assert_eq!(st.history.undo.len(), before + 1, "row deletion is one undo step");
    h.app.run("edit.undo", json!({})).unwrap();
    let st = h.app.session.active().unwrap();
    assert!(matches!(st.doc.node(path).unwrap().kind, vectorcraft_engine::doc::NodeKind::Path { guide: true, .. }));
    assert_eq!(review_positions(&h.app), [40.0, 80.0, 120.0]);
}

#[test]
fn whole_app_delete_removes_highlighted_converted_path_guide_row() {
    review_highlighted_path_key(egui::Key::Delete);
}

#[test]
fn whole_app_backspace_removes_highlighted_converted_path_guide_row() {
    review_highlighted_path_key(egui::Key::Backspace);
}

#[test]
fn whole_app_guide_delete_and_backspace_preserve_the_converted_path() {
    for key in [egui::Key::Delete, egui::Key::Backspace] {
        let (app, path) = review_mixed_scene();
        let mut h = ReviewWholeApp::new(app);
        let out = h.frame(vec![], Modifiers::NONE);
        let at = guide_positions(&out)[1] + vec2(12.0, 0.0);
        h.click(at);
        assert_eq!(selected(&h.app), [1]);
        assert!(!h.ctx.egui_wants_keyboard_input());
        let before = h.app.session.active().unwrap().history.undo.len();
        h.key(key, Modifiers::NONE);
        assert_eq!(review_positions(&h.app), [40.0, 120.0], "{key:?} removes the selected ruler guide");
        let st = h.app.session.active().unwrap();
        assert!(st.doc.node(path).is_some());
        assert!(st.selection.guides.is_empty());
        assert_eq!(st.history.undo.len(), before + 1);
        h.app.run("edit.undo", json!({})).unwrap();
        assert_eq!(review_positions(&h.app), [40.0, 80.0, 120.0]);
        assert_eq!(selected(&h.app), [1]);
    }
}

/// Three named original Nodes, returned in the same top-to-bottom order as their rows.
fn review_node_range_scene() -> (VectorcraftApp, [NodeId; 3]) {
    let (mut app, bottom) = scene();
    app.run("layer.setProps", json!({"ids": [bottom.0], "name": "Range bottom"})).unwrap();
    let mut ids = vec![bottom];
    for (x, name) in [(45, "Range middle"), (80, "Range top")] {
        let id = NodeId(app.run("shape.rectangle", json!({"x": x, "y": 10, "width": 20, "height": 30})).unwrap()["id"].as_u64().unwrap());
        app.run("layer.setProps", json!({"ids": [id.0], "name": name})).unwrap();
        ids.push(id);
    }
    app.run("select.none", json!({})).unwrap();
    (app, [ids[2], ids[1], ids[0]])
}

fn review_node_range_points(out: &egui::FullOutput) -> [Pos2; 3] {
    ["Range top", "Range middle", "Range bottom"].map(|name| text_positions(out, name)[0] + vec2(12.0, 0.0))
}

#[test]
fn repeated_shift_guide_range_keeps_the_anchor_like_node_rows() {
    let (mut nodes, ids) = review_node_range_scene();
    let node_ctx = egui::Context::default();
    let points = review_node_range_points(&frame(&mut nodes, &node_ctx, vec![], Modifiers::NONE));
    click(&mut nodes, &node_ctx, points[0], Modifiers::NONE);
    click(&mut nodes, &node_ctx, points[2], Modifiers::SHIFT);
    assert_eq!(nodes.session.active().unwrap().highlighted_rows(), ids);
    click(&mut nodes, &node_ctx, points[1], Modifiers::SHIFT);
    assert_eq!(nodes.session.active().unwrap().highlighted_rows(), [ids[0], ids[1]], "existing Node rows retain the first clicked anchor");

    let (mut app, _) = scene();
    let ctx = egui::Context::default();
    let rows = guide_positions(&frame(&mut app, &ctx, vec![], Modifiers::NONE));
    click(&mut app, &ctx, rows[0] + vec2(12.0, 0.0), Modifiers::NONE);
    click(&mut app, &ctx, rows[2] + vec2(12.0, 0.0), Modifiers::SHIFT);
    assert_eq!(selected(&app), [0, 1, 2]);
    click(&mut app, &ctx, rows[1] + vec2(12.0, 0.0), Modifiers::SHIFT);
    assert_eq!(selected(&app), [0, 1], "a second Shift range contracts from the original guide anchor");
}

fn review_additive_range(mods: Modifiers) {
    let toggle = Modifiers { shift: false, ..mods };
    let (mut nodes, ids) = review_node_range_scene();
    let node_ctx = egui::Context::default();
    let points = review_node_range_points(&frame(&mut nodes, &node_ctx, vec![], Modifiers::NONE));
    click(&mut nodes, &node_ctx, points[0], Modifiers::NONE);
    click(&mut nodes, &node_ctx, points[2], toggle);
    assert_eq!(nodes.session.active().unwrap().highlighted_rows(), [ids[0], ids[2]]);
    click(&mut nodes, &node_ctx, points[1], mods);
    let mut highlighted = nodes.session.active().unwrap().highlighted_rows();
    highlighted.sort_by_key(|id| id.0);
    let mut all_nodes = ids;
    all_nodes.sort_by_key(|id| id.0);
    assert_eq!(highlighted, all_nodes, "existing Node additive ranges preserve the outside row");

    let (mut app, _) = scene();
    let ctx = egui::Context::default();
    let rows = guide_positions(&frame(&mut app, &ctx, vec![], Modifiers::NONE));
    click(&mut app, &ctx, rows[0] + vec2(12.0, 0.0), Modifiers::NONE);
    click(&mut app, &ctx, rows[2] + vec2(12.0, 0.0), toggle);
    assert_eq!(selected(&app), [0, 2]);
    click(&mut app, &ctx, rows[1] + vec2(12.0, 0.0), mods);
    let mut guides = selected(&app);
    guides.sort_unstable();
    assert_eq!(guides, [0, 1, 2], "an additive guide range must keep guide 0 outside the new range");
}

#[test]
fn command_shift_guide_range_adds_like_node_rows() {
    review_additive_range(Modifiers { command: true, shift: true, ..Default::default() });
}

#[test]
fn control_shift_guide_range_adds_like_node_rows() {
    // egui's logical command flag accompanies a physical Ctrl modifier.
    review_additive_range(Modifiers { ctrl: true, command: true, shift: true, ..Default::default() });
}

#[test]
fn mixed_node_and_ruler_row_clicks_replace_the_other_selection_kind() {
    let (mut app, path) = review_mixed_scene();
    let ctx = egui::Context::default();
    let out = frame(&mut app, &ctx, vec![], Modifiers::NONE);
    let guides = guide_positions(&out);
    let node = text_positions(&out, REVIEW_PATH_NAME)[0];
    click(&mut app, &ctx, guides[1] + vec2(12.0, 0.0), Modifiers::NONE);
    assert_eq!(selected(&app), [1]);
    assert!(app.session.active().unwrap().selection.objects.is_empty());
    assert!(app.session.active().unwrap().highlighted_rows().is_empty());
    click(&mut app, &ctx, pos2(300.0 - SQUARE_X, node.y), Modifiers::NONE);
    assert_eq!(app.session.active().unwrap().selection.objects, [path]);
    assert!(selected(&app).is_empty(), "the legacy Node art square leaves ruler selection");
    click(&mut app, &ctx, guides[2] + vec2(12.0, 0.0), Modifiers::COMMAND);
    assert_eq!(selected(&app), [2], "command-click starts ruler selection without carrying the Node");
    assert!(app.session.active().unwrap().selection.objects.is_empty());
    assert!(app.session.active().unwrap().highlighted_rows().is_empty());
    click(&mut app, &ctx, node + vec2(12.0, 0.0), Modifiers::COMMAND);
    assert!(selected(&app).is_empty());
    assert!(app.session.active().unwrap().selection.objects.is_empty());
    assert_eq!(app.session.active().unwrap().highlighted_rows(), [path]);
}

#[test]
fn filtered_or_collapsed_guide_selection_keeps_the_toolbar_target_and_undo() {
    for mode in ["layers-only", "node-search", "collapsed"] {
        let (mut app, path) = review_mixed_scene();
        let ctx = egui::Context::default();
        let out = frame(&mut app, &ctx, vec![], Modifiers::NONE);
        let guides = guide_positions(&out);
        click(&mut app, &ctx, guides[1] + vec2(12.0, 0.0), Modifiers::NONE);
        match mode {
            "layers-only" => app.ui.layers_panel.layers_only = true,
            "node-search" => {
                click(&mut app, &ctx, pos2(60.0, 12.0), Modifiers::NONE);
                frame(&mut app, &ctx, vec![Event::Text("Original".into())], Modifiers::NONE);
            }
            "collapsed" => {
                let group = text_positions(&out, crate::i18n::tr(crate::i18n::current(), "Guides"))[0];
                click(&mut app, &ctx, pos2(2.0 * COLUMN + 15.0, group.y), Modifiers::NONE);
            }
            _ => unreachable!(),
        }
        let out = frame(&mut app, &ctx, vec![], Modifiers::NONE);
        assert!(text_positions(&out, &guide_name()).is_empty(), "{mode}: ruler rows are omitted");
        if mode == "node-search" {
            assert_eq!(text_positions(&out, REVIEW_PATH_NAME).len(), 1, "only the named path matches the query");
            assert!(text_positions(&out, crate::i18n::tr(crate::i18n::current(), "Guides")).is_empty());
        }
        if mode == "layers-only" {
            assert!(text_positions(&out, REVIEW_PATH_NAME).is_empty(), "Show Layers Only also omits path-guide Nodes");
        }
        assert_eq!(selected(&app), [1], "{mode}: row filtering retains the selected guide");
        let before = app.session.active().unwrap().history.undo.len();
        click(&mut app, &ctx, trash(&out, &ctx), Modifiers::NONE);
        assert_eq!(review_positions(&app), [40.0, 120.0], "{mode}: trash acts on the retained guide selection");
        assert!(app.session.active().unwrap().doc.node(path).is_some());
        assert_eq!(app.session.active().unwrap().history.undo.len(), before + 1);
        app.run("edit.undo", json!({})).unwrap();
        assert_eq!(review_positions(&app), [40.0, 80.0, 120.0]);
        assert_eq!(selected(&app), [1], "{mode}: Undo restores the retained selection");
    }
}

#[test]
fn whole_app_search_delete_and_backspace_preserve_selected_guides() {
    let (app, path) = review_mixed_scene();
    let mut h = ReviewWholeApp::new(app);
    let out = h.frame(vec![], Modifiers::NONE);
    h.click(guide_positions(&out)[1] + vec2(12.0, 0.0));
    assert_eq!(selected(&h.app), [1]);
    let search = h.search_point();
    h.click(search);
    assert!(h.ctx.text_edit_focused());
    let before = h.app.session.active().unwrap().history.undo.len();
    for key in [egui::Key::Delete, egui::Key::Backspace] {
        h.frame(vec![Event::Text("guide".into())], Modifiers::NONE);
        assert_eq!(h.ctx.data(|d| d.get_temp::<String>(egui::Id::new("layers-search"))).as_deref(), Some("guide"));
        h.key(egui::Key::A, Modifiers::COMMAND);
        h.key(key, Modifiers::NONE);
        assert_eq!(h.ctx.data(|d| d.get_temp::<String>(egui::Id::new("layers-search"))).as_deref(), Some(""));
        assert_eq!(selected(&h.app), [1], "{key:?} edits search text instead of removing guides");
        assert_eq!(review_positions(&h.app), [40.0, 80.0, 120.0]);
        assert!(h.app.session.active().unwrap().doc.node(path).is_some());
        assert_eq!(h.app.session.active().unwrap().history.undo.len(), before);
    }
}

/// Explicit headless time lets scroll animation settle without a wall-clock wait.
fn review_panel_frame(app: &mut VectorcraftApp, ctx: &egui::Context, mut events: Vec<Event>, size: egui::Vec2) -> egui::FullOutput {
    let time = ctx.input(|i| i.time) + 0.1;
    events.insert(0, Event::ModifiersChanged(Modifiers::NONE));
    let mut out = ctx.run_ui(
        egui::RawInput { events, time: Some(time), screen_rect: Some(egui::Rect::from_min_size(Pos2::ZERO, size)), ..Default::default() },
        |ui| show(app, ui),
    );
    out.textures_delta.clear();
    out
}

fn review_visible_guides(out: &egui::FullOutput) -> Vec<Pos2> {
    let mut rows = out
        .shapes
        .iter()
        .filter_map(|s| match &s.shape {
            Shape::Text(t) if t.galley.text() == guide_name() => {
                let at = t.pos + vec2(12.0, t.galley.size().y / 2.0);
                s.clip_rect.contains(at).then_some(at)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    rows.sort_by(|a, b| a.y.total_cmp(&b.y));
    rows
}

fn review_panel_click(app: &mut VectorcraftApp, ctx: &egui::Context, at: Pos2, size: egui::Vec2) {
    for e in [Event::PointerMoved(at), button(at, true, Modifiers::NONE), button(at, false, Modifiers::NONE)] {
        review_panel_frame(app, ctx, vec![e], size);
    }
}

fn review_scroll_to_end(app: &mut VectorcraftApp, ctx: &egui::Context, size: egui::Vec2) -> egui::FullOutput {
    let out = review_panel_frame(app, ctx, vec![], size);
    let at = review_visible_guides(&out)[0];
    let wheel =
        Event::MouseWheel { unit: egui::MouseWheelUnit::Point, delta: vec2(0.0, -2000.0), modifiers: Modifiers::NONE, phase: egui::TouchPhase::Move };
    review_panel_frame(app, ctx, vec![Event::PointerMoved(at), wheel], size);
    let mut out = review_panel_frame(app, ctx, vec![], size);
    for _ in 0..15 {
        out = review_panel_frame(app, ctx, vec![], size);
    }
    out
}

#[test]
fn virtualized_mixed_rows_target_exact_guides_after_removal_and_undo() {
    for row_size in [ROW_SMALL, ROW_LARGE] {
        let (mut app, path) = review_mixed_scene();
        for i in 3..24 {
            app.run("guide.add", json!({"vertical": true, "pos": i * 5 + 1})).unwrap();
        }
        app.ui.layers_panel.row_size = row_size;
        let original = review_positions(&app);
        let target = original.len() - 3;
        let ctx = egui::Context::default();
        let size = vec2(300.0, 260.0);
        let out = review_scroll_to_end(&mut app, &ctx, size);
        let rows = review_visible_guides(&out);
        let painted = text_positions(&out, &guide_name()).len();
        assert!(rows.len() >= 3 && painted < original.len(), "only the clipped range is painted");
        assert!(painted <= (size.y / row_size).ceil() as usize + 1);
        // At the end, the final row is index len-1 and the third-last is len-3.
        // This oracle uses the document length, not any row's bound command/index.
        review_panel_click(&mut app, &ctx, *rows.last().unwrap(), size);
        assert_eq!(selected(&app), [original.len() - 1]);
        let at = rows[rows.len() - 3];
        review_panel_click(&mut app, &ctx, at, size);
        assert_eq!(selected(&app), [target], "a scrolled pointer targets the exact third-last guide");
        assert_eq!(app.session.active().unwrap().doc.guides[target].pos, original[target]);

        let before = app.session.active().unwrap().history.undo.len();
        review_panel_click(&mut app, &ctx, pos2(0.5 * COLUMN, at.y), size);
        let mut expected = vec![(true, false); original.len()];
        expected[target].0 = false;
        assert_eq!(flags(&app), expected, "the scrolled eye changes only the exact guide");
        assert_eq!(app.session.active().unwrap().history.undo.len(), before + 1);
        app.run("edit.undo", json!({})).unwrap();
        assert_eq!(selected(&app), [target]);
        review_panel_click(&mut app, &ctx, pos2(1.5 * COLUMN, at.y), size);
        expected[target] = (true, true);
        assert_eq!(flags(&app), expected, "the scrolled lock changes only the exact guide");
        app.run("edit.undo", json!({})).unwrap();
        assert_eq!(flags(&app), vec![(true, false); original.len()]);

        app.run("guide.remove", json!({"index": 0})).unwrap();
        assert_eq!(selected(&app), [target - 1], "existing command helpers remap the retained selection");
        assert_eq!(review_positions(&app), original[1..]);
        let out = review_scroll_to_end(&mut app, &ctx, size);
        let rows = review_visible_guides(&out);
        review_panel_click(&mut app, &ctx, rows[rows.len() - 3], size);
        assert_eq!(selected(&app), [target - 1], "the survivor's scrolled row uses its new document index");
        assert_eq!(app.session.active().unwrap().doc.guides[target - 1].pos, original[target]);
        app.run("edit.undo", json!({})).unwrap();
        assert_eq!(review_positions(&app), original);
        assert_eq!(selected(&app), [target]);

        let out = review_scroll_to_end(&mut app, &ctx, size);
        let rows = review_visible_guides(&out);
        review_panel_click(&mut app, &ctx, rows[rows.len() - 3], size);
        assert_eq!(selected(&app), [target], "Undo restores the original row index");
        let before = app.session.active().unwrap().history.undo.len();
        review_panel_click(&mut app, &ctx, trash(&out, &ctx), size);
        let remaining: Vec<f64> = original.iter().enumerate().filter_map(|(i, &pos)| (i != target).then_some(pos)).collect();
        assert_eq!(review_positions(&app), remaining, "toolbar trash deletes exactly the scrolled selected guide");
        assert_eq!(app.session.active().unwrap().history.undo.len(), before + 1);
        let node = app.session.active().unwrap().doc.node(path).unwrap();
        assert!(node.visible && !node.locked, "legacy path-guide columns and ownership are unchanged");
        app.run("edit.undo", json!({})).unwrap();
        assert_eq!(review_positions(&app), original);
        assert_eq!(selected(&app), [target]);
    }
}

#[test]
fn shift_guide_range_keeps_survivor_anchor_after_context_removal() {
    let (mut app, art) = scene();
    let ctx = egui::Context::default();
    let rows = guide_positions(&frame(&mut app, &ctx, vec![], Modifiers::NONE));
    click(&mut app, &ctx, rows[1] + vec2(12.0, 0.0), Modifiers::NONE);
    click(&mut app, &ctx, rows[2] + vec2(12.0, 0.0), Modifiers::SHIFT);
    assert_eq!(selected(&app), [1, 2]);
    let undo = app.session.active().unwrap().history.undo.len();

    // Delete A through its real context menu without replacing the B/C selection.
    let at = rows[0] + vec2(12.0, 0.0);
    for e in [
        Event::PointerMoved(at),
        Event::PointerButton { pos: at, button: egui::PointerButton::Secondary, pressed: true, modifiers: Modifiers::NONE },
        Event::PointerButton { pos: at, button: egui::PointerButton::Secondary, pressed: false, modifiers: Modifiers::NONE },
    ] {
        frame(&mut app, &ctx, vec![e], Modifiers::NONE);
    }
    frame(&mut app, &ctx, vec![], Modifiers::NONE);
    let out = frame(&mut app, &ctx, vec![], Modifiers::NONE);
    let menu = text_positions(&out, crate::i18n::tr(crate::i18n::current(), "Delete Selection"))[0];
    click(&mut app, &ctx, menu + vec2(12.0, 0.0), Modifiers::NONE);
    assert_eq!(review_positions(&app), [80.0, 120.0]);
    assert_eq!(selected(&app), [0, 1], "the command remapped both surviving guides");
    assert_eq!(app.session.active().unwrap().history.undo.len(), undo + 1);

    let out = frame(&mut app, &ctx, vec![], Modifiers::NONE);
    let rows = guide_positions(&out);
    click(&mut app, &ctx, rows[0] + vec2(12.0, 0.0), Modifiers::SHIFT);
    assert_eq!(selected(&app), [0], "Shift back to B contracts from B, even after A was removed");
    let out = frame(&mut app, &ctx, vec![], Modifiers::NONE);
    click(&mut app, &ctx, trash(&out, &ctx), Modifiers::NONE);
    assert_eq!(review_positions(&app), [120.0], "C was outside the contracted range and must survive");
    assert!(app.session.active().unwrap().doc.node(art).is_some());
    assert_eq!(app.session.active().unwrap().history.undo.len(), undo + 2);
    app.run("edit.undo", json!({})).unwrap();
    assert_eq!(review_positions(&app), [80.0, 120.0]);
    assert_eq!(selected(&app), [0]);
    app.run("edit.undo", json!({})).unwrap();
    assert_eq!(review_positions(&app), [40.0, 80.0, 120.0]);
    assert_eq!(selected(&app), [1, 2]);
    app.run("edit.redo", json!({})).unwrap();
    app.run("edit.redo", json!({})).unwrap();
    assert_eq!(review_positions(&app), [120.0]);
    assert!(app.session.active().unwrap().doc.node(art).is_some());
}

#[test]
fn identical_guide_rows_keep_the_existing_range_fallback_after_removal() {
    let (mut app, art) = scene();
    app.run("guide.move", json!({"index": 1, "pos": 40})).unwrap();
    app.run("guide.remove", json!({"index": 2})).unwrap();
    app.run("guide.add", json!({"vertical": true, "pos": 40})).unwrap();
    let ctx = egui::Context::default();
    let rows = guide_positions(&frame(&mut app, &ctx, vec![], Modifiers::NONE));
    click(&mut app, &ctx, rows[2] + vec2(12.0, 0.0), Modifiers::NONE);
    click(&mut app, &ctx, rows[0] + vec2(12.0, 0.0), Modifiers::SHIFT);
    assert_eq!(selected(&app), [0, 1, 2]);
    app.run("guide.remove", json!({"index": 0})).unwrap();
    assert_eq!(selected(&app), [0, 1]);
    let rows = guide_positions(&frame(&mut app, &ctx, vec![], Modifiers::NONE));
    click(&mut app, &ctx, rows[1] + vec2(12.0, 0.0), Modifiers::SHIFT);
    assert_eq!(selected(&app), [1], "ambiguous guide values must not replace the last-row fallback with the first match");
    let out = frame(&mut app, &ctx, vec![], Modifiers::NONE);
    click(&mut app, &ctx, trash(&out, &ctx), Modifiers::NONE);
    assert_eq!(review_positions(&app), [40.0]);
    assert!(app.session.active().unwrap().doc.node(art).is_some());
}

// Compare complete document/selection state across column edits and journal replay.
fn fr2_guide_column_state(app: &VectorcraftApp) -> Value {
    let st = app.session.doc().unwrap();
    json!({"doc": st.doc.as_ref(), "selection": st.selection})
}

fn fr2_assert_guide_column_replay(app: &VectorcraftApp, phase: &str) {
    let mut replay = Session::new();
    for (command, params) in &app.session.journal {
        replay.execute(command, params).unwrap();
    }
    let st = replay.doc().unwrap();
    assert_eq!(
        json!({"doc": st.doc.as_ref(), "selection": st.selection}),
        fr2_guide_column_state(app),
        "{phase}: a released guide-column gesture and later Undo/Redo must replay the visible document and selection"
    );
}

#[test]
fn guide_column_drags_preserve_journal_replay_through_undo_and_redo() {
    for eye in [true, false] {
        let x = COLUMN * if eye { 0.5 } else { 1.5 };
        // Positive control: one ordinary property click, Undo and Redo use the same
        // widgets, command path and full-document replay oracle without grouping.
        let (mut single, single_art) = scene();
        let single_ctx = egui::Context::default();
        let single_rows = guide_positions(&frame(&mut single, &single_ctx, vec![], Modifiers::NONE));
        let single_before = fr2_guide_column_state(&single);
        let single_undo = single.session.doc().unwrap().history.undo.len();
        click(&mut single, &single_ctx, pos2(x, single_rows[0].y), Modifiers::NONE);
        assert_eq!(flags(&single), if eye { [(false, false), (true, false), (true, false)] } else { [(true, true), (true, false), (true, false)] });
        assert_eq!(single.session.doc().unwrap().history.undo.len(), single_undo + 1);
        let single_changed = fr2_guide_column_state(&single);
        fr2_assert_guide_column_replay(&single, "single click");
        single.run("edit.undo", json!({})).unwrap();
        assert_eq!(fr2_guide_column_state(&single), single_before);
        fr2_assert_guide_column_replay(&single, "single click Undo");
        single.run("edit.redo", json!({})).unwrap();
        assert_eq!(fr2_guide_column_state(&single), single_changed);
        fr2_assert_guide_column_replay(&single, "single click Redo");
        assert!(single.session.doc().unwrap().doc.node(single_art).is_some());

        let (mut app, art) = scene();
        let ctx = egui::Context::default();
        let rows = guide_positions(&frame(&mut app, &ctx, vec![], Modifiers::NONE));
        assert_eq!(rows.len(), 3);
        let before = fr2_guide_column_state(&app);
        let undo = app.session.doc().unwrap().history.undo.len();
        // Same actual eye/lock drag sequence as the already-qualified column test.
        for events in [
            vec![Event::PointerMoved(pos2(x, rows[0].y))],
            vec![button(pos2(x, rows[0].y), true, Modifiers::NONE)],
            vec![Event::PointerMoved(pos2(x, rows[0].y + 6.0))],
            vec![Event::PointerMoved(pos2(x, rows[1].y))],
            vec![Event::PointerMoved(pos2(x, rows[2].y))],
            vec![button(pos2(x, rows[2].y), false, Modifiers::NONE)],
            vec![],
        ] {
            frame(&mut app, &ctx, events, Modifiers::NONE);
        }
        assert_eq!(flags(&app), if eye { [(false, false); 3] } else { [(true, true); 3] });
        let st = app.session.doc().unwrap();
        assert_eq!(st.history.undo.len(), undo + 1, "the existing one-step drag contract stays intact");
        assert!(st.undo_group.is_none() && !app.session.in_interaction(), "the released group has ended");
        let changed = fr2_guide_column_state(&app);
        fr2_assert_guide_column_replay(&app, "released drag before Undo");
        app.run("edit.undo", json!({})).unwrap();
        assert_eq!(fr2_guide_column_state(&app), before, "ordinary Undo restores all three guide flags");
        assert_eq!(app.session.doc().unwrap().history.undo.len(), undo);
        assert_eq!(app.session.doc().unwrap().history.redo.len(), 1);
        fr2_assert_guide_column_replay(&app, "drag Undo");
        app.run("edit.redo", json!({})).unwrap();
        assert_eq!(fr2_guide_column_state(&app), changed);
        fr2_assert_guide_column_replay(&app, "drag Redo");
        assert!(app.session.doc().unwrap().doc.node(art).is_some());
    }
}

// Begin a held column gesture before testing command and history boundaries.
fn fr2_start_column(app: &mut VectorcraftApp, ctx: &egui::Context, x: f32, row: Pos2) {
    // Cross the click/drag tolerance while remaining in the first column cell.
    let dragged = pos2(x + COLUMN * 0.4, row.y);
    for e in [Event::PointerMoved(pos2(x, row.y)), button(pos2(x, row.y), true, Modifiers::NONE), Event::PointerMoved(dragged)] {
        frame(app, ctx, vec![e], Modifiers::NONE);
    }
}

fn fr2_finish_column(app: &mut VectorcraftApp, ctx: &egui::Context, x: f32, rows: &[Pos2]) {
    for events in [
        vec![Event::PointerMoved(pos2(x, rows[1].y))],
        vec![Event::PointerMoved(pos2(x, rows[2].y))],
        vec![button(pos2(x, rows[2].y), false, Modifiers::NONE)],
        vec![],
    ] {
        frame(app, ctx, events, Modifiers::NONE);
    }
}

#[test]
fn guide_column_recorded_action_plays_without_nested_batch_and_undoes_once() {
    use crate::panels::actions::{Action, ActionSet, play};

    for eye in [true, false] {
        let (mut source, _) = scene();
        let ctx = egui::Context::default();
        let rows = guide_positions(&frame(&mut source, &ctx, vec![], Modifiers::NONE));
        let start = source.session.journal.len();
        let x = COLUMN * if eye { 0.5 } else { 1.5 };
        fr2_start_column(&mut source, &ctx, x, rows[0]);
        fr2_finish_column(&mut source, &ctx, x, &rows);
        let steps = source.session.journal_for_action(start);
        let expected = if eye { json!({"indexes": [0, 1, 2], "visible": false}) } else { json!({"indexes": [0, 1, 2], "locked": true}) };
        assert_eq!(steps, vec![("guide.setProps".into(), expected)], "record the existing drag as one ordinary property command");

        let (mut target, art) = scene();
        let before = fr2_guide_column_state(&target);
        let undo = target.session.doc().unwrap().history.undo.len();
        let art_before = serde_json::to_value(target.session.doc().unwrap().doc.node(art).unwrap()).unwrap();
        target.ui.action_sets = vec![ActionSet { name: "Guide Controls".into(), actions: vec![Action { name: "Guide Column".into(), steps }] }];
        play(&mut target, 0, 0).unwrap(); // Existing player supplies the single outer batch.
        assert_eq!(flags(&target), if eye { [(false, false); 3] } else { [(true, true); 3] });
        assert_eq!(target.session.doc().unwrap().history.undo.len(), undo + 1);
        assert_eq!(serde_json::to_value(target.session.doc().unwrap().doc.node(art).unwrap()).unwrap(), art_before);
        let changed = fr2_guide_column_state(&target);
        fr2_assert_guide_column_replay(&target, "Actions play");
        target.run("edit.undo", json!({})).unwrap();
        assert_eq!(fr2_guide_column_state(&target), before);
        fr2_assert_guide_column_replay(&target, "Actions Undo");
        target.run("edit.redo", json!({})).unwrap();
        assert_eq!(fr2_guide_column_state(&target), changed);
        fr2_assert_guide_column_replay(&target, "Actions Redo");
    }
}

#[test]
fn guide_column_intervening_command_keeps_its_own_journal_history_and_undo() {
    let (mut app, art) = scene();
    let ctx = egui::Context::default();
    let rows = guide_positions(&frame(&mut app, &ctx, vec![], Modifiers::NONE));
    let before = fr2_guide_column_state(&app);
    let prefix = app.session.journal.clone();
    let undo = app.session.doc().unwrap().history.undo.len();
    let x = COLUMN * 0.5;
    fr2_start_column(&mut app, &ctx, x, rows[0]);
    assert_eq!(flags(&app), [(false, false), (true, false), (true, false)]);
    let first = fr2_guide_column_state(&app);
    let params = json!({"ids": [art.0], "name": "Intervening guide control"});
    app.run("layer.setProps", params.clone()).unwrap();
    let intervening = fr2_guide_column_state(&app);
    assert_eq!(app.session.doc().unwrap().doc.node(art).unwrap().name.as_deref(), Some("Intervening guide control"));
    fr2_finish_column(&mut app, &ctx, x, &rows);
    assert_eq!(flags(&app), [(false, false); 3]);
    let changed = fr2_guide_column_state(&app);
    assert_eq!(app.session.doc().unwrap().history.undo.len(), undo + 3, "the two Guide segments and intervening Node edit remain separate");
    assert_eq!(&app.session.journal[..prefix.len()], prefix.as_slice());
    assert_eq!(
        &app.session.journal[prefix.len()..],
        [
            ("guide.setProps".into(), json!({"indexes": [0], "visible": false})),
            ("layer.setProps".into(), params),
            ("guide.setProps".into(), json!({"indexes": [1, 2], "visible": false}))
        ]
    );
    fr2_assert_guide_column_replay(&app, "intervening command before Undo");
    for expected in [&intervening, &first, &before] {
        app.run("edit.undo", json!({})).unwrap();
        assert_eq!(&fr2_guide_column_state(&app), expected);
        fr2_assert_guide_column_replay(&app, "intervening command Undo");
    }
    for expected in [&first, &intervening, &changed] {
        app.run("edit.redo", json!({})).unwrap();
        assert_eq!(&fr2_guide_column_state(&app), expected);
        fr2_assert_guide_column_replay(&app, "intervening command Redo");
    }
}

fn fr2_guide_documents(app: &VectorcraftApp) -> Value {
    json!({
        "active": app.session.active_index(),
        "documents": app.session.documents().iter().map(|st| json!({"doc": st.doc.as_ref(), "selection": st.selection})).collect::<Vec<_>>()
    })
}

fn fr2_assert_guide_documents_replay(app: &VectorcraftApp) {
    let mut replay = Session::new();
    for (command, params) in &app.session.journal {
        replay.execute(command, params).unwrap();
    }
    let actual = json!({
        "active": replay.active_index(),
        "documents": replay.documents().iter().map(|st| json!({"doc": st.doc.as_ref(), "selection": st.selection})).collect::<Vec<_>>()
    });
    assert_eq!(actual, fr2_guide_documents(app));
}

#[test]
fn guide_column_document_change_preserves_old_segment_and_consumes_late_release() {
    let (mut app, _) = scene();
    let ctx = egui::Context::default();
    let rows = guide_positions(&frame(&mut app, &ctx, vec![], Modifiers::NONE));
    let prefix = app.session.journal.clone();
    let old_undo = app.session.doc().unwrap().history.undo.len();
    let x = COLUMN * 0.5;
    fr2_start_column(&mut app, &ctx, x, rows[0]);
    assert_eq!(flags(&app), [(false, false), (true, false), (true, false)]);
    let original = serde_json::to_value(app.session.doc().unwrap().doc.as_ref()).unwrap();
    app.run("file.new", json!({"name": "Guide Column B", "width": 320, "height": 240})).unwrap();
    let new_base = fr2_guide_documents(&app);
    app.run("guide.add", json!({"vertical": true, "pos": 60})).unwrap();
    let journal = app.session.journal.clone();
    let new_undo = app.session.doc().unwrap().history.undo.len();
    let out = frame(&mut app, &ctx, vec![], Modifiers::NONE);
    let new_row = guide_positions(&out)[0];
    for e in [Event::PointerMoved(pos2(x, new_row.y)), button(pos2(x, new_row.y), false, Modifiers::NONE)] {
        frame(&mut app, &ctx, vec![e], Modifiers::NONE);
    }
    frame(&mut app, &ctx, vec![], Modifiers::NONE);
    assert_eq!(flags(&app), [(true, false)], "the old pressed row must not toggle the new document's guide");
    assert_eq!(app.session.journal, journal, "late events and cleanup preserve all prior and new-tab records");
    assert_eq!(&app.session.journal[..prefix.len()], prefix.as_slice());
    assert_eq!(app.session.journal[prefix.len()], ("guide.setProps".into(), json!({"indexes": [0], "visible": false})));
    let old = &app.session.documents()[0];
    assert_eq!(serde_json::to_value(old.doc.as_ref()).unwrap(), original);
    assert_eq!(old.history.undo.len(), old_undo + 1);
    assert!(old.undo_group.is_none());
    assert_eq!(app.session.doc().unwrap().history.undo.len(), new_undo);
    let final_state = fr2_guide_documents(&app);
    fr2_assert_guide_documents_replay(&app);
    app.run("edit.undo", json!({})).unwrap();
    assert_eq!(fr2_guide_documents(&app), new_base, "Undo affects only the new tab's independent guide creation");
    fr2_assert_guide_documents_replay(&app);
    app.run("edit.redo", json!({})).unwrap();
    assert_eq!(fr2_guide_documents(&app), final_state);
    fr2_assert_guide_documents_replay(&app);
}

#[test]
fn guide_column_noop_rows_and_history_limits_preserve_the_original_undo_prefix() {
    for limit in [1, 2] {
        let (mut app, art) = scene();
        app.run("guide.setProps", json!({"index": 1, "visible": false})).unwrap(); // Middle row already has the drag's value.
        let st = app.session.doc_mut().unwrap();
        st.history.limit = limit;
        let excess = st.history.undo.len().saturating_sub(limit);
        drop(st.history.undo.drain(..excess));
        let ctx = egui::Context::default();
        let rows = guide_positions(&frame(&mut app, &ctx, vec![], Modifiers::NONE));
        let before = fr2_guide_column_state(&app);
        let x = COLUMN * 0.5;
        fr2_start_column(&mut app, &ctx, x, rows[0]);
        assert_eq!(flags(&app), [(false, false), (false, false), (true, false)], "the first row changed before the history snapshot");
        let first_history = app.session.doc().unwrap().history.undo.clone();
        fr2_finish_column(&mut app, &ctx, x, &rows);
        assert_eq!(flags(&app), [(false, false); 3]);
        let st = app.session.doc().unwrap();
        assert_eq!(st.history.undo.len(), first_history.len());
        for (actual, expected) in st.history.undo.iter().zip(&first_history) {
            assert_eq!(actual.label, expected.label);
            assert!(Arc::ptr_eq(&actual.doc, &expected.doc), "extra rows must not evict another prefix entry at the history limit");
            assert_eq!(actual.selection, expected.selection);
        }
        let node = st.doc.node(art).unwrap();
        assert!(node.visible && !node.locked);
        let changed = fr2_guide_column_state(&app);
        fr2_assert_guide_column_replay(&app, "limited history before Undo");
        app.run("edit.undo", json!({})).unwrap();
        assert_eq!(fr2_guide_column_state(&app), before, "one Undo includes every newly changed row and preserves the already-hidden row");
        fr2_assert_guide_column_replay(&app, "limited history Undo");
        app.run("edit.redo", json!({})).unwrap();
        assert_eq!(fr2_guide_column_state(&app), changed);
        fr2_assert_guide_column_replay(&app, "limited history Redo");
    }
}

// Real keyboard nudges keep independent history during a held Guide column drag.
// Three guides and one rectangle exercise an intervening edit and an equal-value row.
fn fr2_review_guide_shortcut_frame(app: &mut VectorcraftApp, ctx: &egui::Context, events: Vec<Event>) {
    let mut events = events;
    events.insert(0, Event::ModifiersChanged(Modifiers::NONE));
    let mut out = ctx.run_ui(
        egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(Pos2::ZERO, vec2(300.0, 480.0))), events, ..Default::default() },
        |ui| {
            crate::shortcuts::handle(app, ui.ctx());
            show(app, ui);
        },
    );
    out.textures_delta.clear();
}

#[test]
fn guide_column_noop_after_keyboard_nudge_keeps_the_next_real_undo_step() {
    for eye in [true, false] {
        let (mut app, art) = scene();
        let prop = if eye { "visible" } else { "locked" };
        let value = !eye;
        app.run("guide.setProps", json!({"index": 1, prop: value})).unwrap();
        app.run("select.none", json!({})).unwrap();
        app.run("select.set", json!({"ids": [art.0]})).unwrap();
        assert!(!app.session.tool_busy());
        let initial = fr2_guide_column_state(&app);
        let undo = app.session.doc().unwrap().history.undo.len();
        let ctx = egui::Context::default();
        let rows = guide_positions(&frame(&mut app, &ctx, vec![], Modifiers::NONE));
        assert_eq!(rows.len(), 3);
        let x = COLUMN * if eye { 0.5 } else { 1.5 };
        fr2_start_column(&mut app, &ctx, x, rows[0]);
        assert_eq!(flags(&app), if eye { [(false, false), (false, false), (true, false)] } else { [(true, true), (true, true), (true, false)] });
        assert_eq!(app.session.doc().unwrap().history.undo.len(), undo + 1);
        let before_nudge = fr2_guide_column_state(&app);
        // Ordinary interleaving: the pointer remains down over the Guide column,
        // while Right nudges the already-selected artwork via the real handler.
        let start = app.session.journal.len();
        fr2_review_guide_shortcut_frame(
            &mut app,
            &ctx,
            vec![Event::Key { key: egui::Key::ArrowRight, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE }],
        );
        assert!(app.session.journal[start..].iter().any(|(id, _)| id == "object.nudge"), "the keyboard control actually ran the intervening edit");
        assert_eq!(app.session.doc().unwrap().history.undo.len(), undo + 2);
        let after_nudge = fr2_guide_column_state(&app);
        assert_ne!(after_nudge, before_nudge);
        fr2_assert_guide_column_replay(&app, "keyboard nudge during held Guide drag");
        // The middle guide already has the drag's target value: its command is a no-op.
        frame(
            &mut app,
            &ctx,
            vec![
                Event::Key { key: egui::Key::ArrowRight, physical_key: None, pressed: false, repeat: false, modifiers: Modifiers::NONE },
                Event::PointerMoved(pos2(x, rows[1].y)),
            ],
            Modifiers::NONE,
        );
        assert_eq!(fr2_guide_column_state(&app), after_nudge);
        assert_eq!(app.session.doc().unwrap().history.undo.len(), undo + 2);
        // The last row really changes, so it must own an undo entry after the nudge.
        for events in [vec![Event::PointerMoved(pos2(x, rows[2].y))], vec![button(pos2(x, rows[2].y), false, Modifiers::NONE)], vec![]] {
            frame(&mut app, &ctx, events, Modifiers::NONE);
        }
        assert_eq!(flags(&app), if eye { [(false, false); 3] } else { [(true, true); 3] });
        let changed = fr2_guide_column_state(&app);
        fr2_assert_guide_column_replay(&app, "mixed flags and released drag");
        assert_eq!(app.session.doc().unwrap().history.undo.len(), undo + 3, "the real Guide change after a no-op must not discard its undo entry");
        let st = app.session.doc().unwrap();
        assert!(st.interaction.is_none() && st.undo_group.is_none());
        // Semantic snapshots keep the oracle independent of coalescing representation.
        for expected in [&after_nudge, &before_nudge, &initial] {
            app.run("edit.undo", json!({})).unwrap();
            assert_eq!(&fr2_guide_column_state(&app), expected);
            fr2_assert_guide_column_replay(&app, "independent Guide/nudge Undo");
        }
        for expected in [&before_nudge, &after_nudge, &changed] {
            app.run("edit.redo", json!({})).unwrap();
            assert_eq!(&fr2_guide_column_state(&app), expected);
            fr2_assert_guide_column_replay(&app, "independent Guide/nudge Redo");
        }
    }
}

// Exercise real Delete while a Guide eye-column drag remains held.
fn fr3_actual_guide_eye(h: &ReviewWholeApp, name: Pos2) -> Pos2 {
    let widgets = h.ctx.viewport(|vp| vp.prev_pass.widgets.layers().flat_map(|(_, rows)| rows.iter()).map(|w| (w.rect, w.sense)).collect::<Vec<_>>());
    let rows =
        widgets.iter().filter(|(r, sense)| r.height() == h.app.ui.layers_panel.row() && r.contains(name) && sense.senses_drag()).collect::<Vec<_>>();
    assert_eq!(rows.len(), 1, "the painted Guide name resolves to one actual row");
    let row = rows[0].0;
    let eyes = widgets
        .iter()
        .filter(|(r, sense)| {
            r.left() == row.left() && r.top() == row.top() && r.bottom() == row.bottom() && r.width() < row.width() && sense.senses_drag()
        })
        .collect::<Vec<_>>();
    assert_eq!(eyes.len(), 1, "the real row's leftmost narrow response is its eye");
    eyes[0].0.center()
}

fn fr3_begin_held_guide_eye(h: &mut ReviewWholeApp, eye: Pos2) {
    let eye_rects = h.ctx.viewport(|vp| {
        vp.prev_pass
            .widgets
            .layers()
            .flat_map(|(_, rows)| rows.iter())
            .filter(|w| w.rect.center() == eye && w.sense.senses_drag())
            .map(|w| w.rect)
            .collect::<Vec<_>>()
    });
    assert_eq!(eye_rects.len(), 1, "gesture setup identifies the same actual eye response");
    // Cross the drag threshold while remaining inside the same eye response.
    let held = eye + vec2(10.0, 0.0);
    assert!(eye_rects[0].contains(held), "threshold movement stays within the first row's eye response");
    for e in [Event::PointerMoved(eye), button(eye, true, Modifiers::NONE), Event::PointerMoved(held)] {
        h.frame(vec![e], Modifiers::NONE);
    }
    assert!(h.ctx.input(|i| i.pointer.primary_down()));
    assert!(h.ctx.data(|d| d.get_temp::<GuideColumnDrag>(key("guide-column")).is_some()), "the actual eye started a column drag");
}

#[test]
fn guide_eye_drag_after_real_delete_addresses_the_surviving_row() {
    for delete_first in [false, true] {
        let (app, art) = scene(); // Original 3 guides at 40, 80, 120 and one rectangle.
        let mut h = ReviewWholeApp::new(app);
        let out = h.frame(vec![], Modifiers::NONE);
        let rows = guide_positions(&out);
        assert_eq!(rows.len(), 3);
        h.click(rows[0] + vec2(12.0, 0.0));
        assert_eq!(selected(&h.app), [0], "Delete's target is a different editable Guide");
        let out = h.frame(vec![], Modifiers::NONE);
        let rows = guide_positions(&out);
        let eye = fr3_actual_guide_eye(&h, rows[1]);
        let undo = h.app.session.doc().unwrap().history.undo.len();
        fr3_begin_held_guide_eye(&mut h, eye);
        assert_eq!(flags(&h.app), [(true, false), (false, false), (true, false)]);
        assert_eq!(selected(&h.app), [0], "hiding guide 1 leaves guide 0 selected");
        assert_eq!(h.app.session.doc().unwrap().history.undo.len(), undo + 1);
        if delete_first {
            // Full app dispatches shortcuts before the Layers panel. No direct engine substitute.
            assert!(!h.ctx.egui_wants_keyboard_input());
            let journal = h.app.session.journal.len();
            h.key(egui::Key::Delete, Modifiers::NONE);
            assert!(
                h.app.session.journal[journal..].iter().any(|(id, _)| id == "edit.clear" || id == "guide.remove"),
                "a real Delete command was dispatched"
            );
            assert_eq!(review_positions(&h.app), [80.0, 120.0], "the actual key removed original guide 0");
            assert!(selected(&h.app).is_empty());
            assert!(h.ctx.input(|i| i.pointer.primary_down()), "Delete did not release the column gesture");
        }
        let out = h.frame(vec![], Modifiers::NONE);
        let current_rows = guide_positions(&out);
        assert_eq!(current_rows.len(), if delete_first { 2 } else { 3 });
        let target = fr3_actual_guide_eye(&h, *current_rows.last().unwrap());
        for e in [Event::PointerMoved(target), button(target, false, Modifiers::NONE)] {
            h.frame(vec![e], Modifiers::NONE);
        }
        h.frame(vec![], Modifiers::NONE);
        let st = h.app.session.doc().unwrap();
        assert!(st.doc.node(art).is_some(), "the original artwork survives");
        let survivor = st.doc.guides.iter().find(|g| g.pos == 120.0).unwrap();
        assert!(
            !survivor.visible,
            "delete_first={delete_first}: continuing across the actual last Guide row must apply the eye drag to original guide 2"
        );
        assert!(!h.ctx.input(|i| i.pointer.primary_down()));
        assert!(h.ctx.data(|d| d.get_temp::<GuideColumnDrag>(key("guide-column")).is_none()));
        assert_eq!(st.history.undo.len(), undo + if delete_first { 3 } else { 1 }, "a structural Delete keeps its own history boundary");
        fr2_assert_guide_column_replay(&h.app, "released column after optional real Delete");
    }
}

/// Disabling an already-held Layers column must preserve state until release.
#[test]
fn guide_column_drag_preserves_disabled_panel_state_and_recovers() {
    fn protected_state(app: &VectorcraftApp) -> Value {
        let st = app.session.active().unwrap();
        let entries = |v: &[vectorcraft_engine::HistoryEntry]| -> Vec<Value> {
            v.iter().map(|e| json!({"label": e.label, "doc": e.doc.as_ref(), "selection": e.selection})).collect()
        };
        json!({"doc": st.doc.as_ref(), "selection": st.selection,
            "undo": entries(&st.history.undo), "redo": entries(&st.history.redo), "limit": st.history.limit,
            "interaction": st.interaction.is_some(), "undo_group": st.undo_group.is_some(), "journal": &app.session.journal})
    }
    fn enabled_frame(app: &mut VectorcraftApp, ctx: &egui::Context, events: Vec<Event>, enabled: bool) -> egui::FullOutput {
        let mut events = events;
        events.insert(0, Event::ModifiersChanged(Modifiers::NONE));
        let mut out = ctx.run_ui(
            egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(Pos2::ZERO, vec2(300.0, 480.0))), events, ..Default::default() },
            |ui| {
                if !enabled {
                    ui.disable();
                }
                assert_eq!(ui.is_enabled(), enabled);
                show(app, ui);
            },
        );
        out.textures_delta.clear();
        out
    }
    for disabled in [false, true] {
        let (mut app, art) = scene();
        let ctx = egui::Context::default();
        let rows = guide_positions(&frame(&mut app, &ctx, vec![], Modifiers::NONE));
        assert_eq!(rows.len(), 3);
        click(&mut app, &ctx, rows[0] + vec2(12.0, 0.0), Modifiers::NONE);
        assert_eq!(selected(&app), [0]);
        let initial = fr2_guide_column_state(&app);
        let undo = app.session.doc().unwrap().history.undo.len();
        let rows = guide_positions(&frame(&mut app, &ctx, vec![], Modifiers::NONE));
        let x = 0.5 * COLUMN;
        fr2_start_column(&mut app, &ctx, x, rows[1]);
        assert_eq!(flags(&app), [(true, false), (false, false), (true, false)]);
        assert_eq!(selected(&app), [0]);
        assert_eq!(app.session.doc().unwrap().history.undo.len(), undo + 1);
        assert!(ctx.input(|i| i.pointer.primary_down()));
        assert!(ctx.data(|d| d.get_temp::<GuideColumnDrag>(key("guide-column")).is_some()));
        let after_first = fr2_guide_column_state(&app);
        let held = protected_state(&app);
        let out = enabled_frame(&mut app, &ctx, vec![], !disabled);
        assert_eq!(guide_positions(&out), rows, "disabling preserves the actual painted Guide row positions");
        assert_eq!(protected_state(&app), held, "changing enabled state alone does not edit the held segment");
        let target = pos2(x, rows[2].y);
        enabled_frame(&mut app, &ctx, vec![Event::PointerMoved(target)], !disabled);
        assert!(ctx.input(|i| i.pointer.primary_down()), "the original pointer is still held");
        if disabled {
            assert_eq!(protected_state(&app), held, "disabled continuation preserves the document, selection and complete history/journal");
        } else {
            assert_eq!(flags(&app), [(true, false), (false, false), (false, false)]);
            assert_eq!(app.session.doc().unwrap().history.undo.len(), undo + 1, "enabled continuation remains one owned Undo step");
        }
        enabled_frame(&mut app, &ctx, vec![button(target, false, Modifiers::NONE)], !disabled);
        enabled_frame(&mut app, &ctx, vec![], !disabled);
        assert!(!ctx.input(|i| i.pointer.primary_down()));
        assert!(ctx.data(|d| d.get_temp::<GuideColumnDrag>(key("guide-column")).is_none()), "a real release cleans up even while disabled");
        if disabled {
            assert_eq!(protected_state(&app), held, "disabled release does not mutate the background");
            frame(&mut app, &ctx, vec![], Modifiers::NONE);
            fr2_start_column(&mut app, &ctx, x, rows[2]);
            frame(&mut app, &ctx, vec![button(target, false, Modifiers::NONE)], Modifiers::NONE);
            frame(&mut app, &ctx, vec![], Modifiers::NONE);
            assert_eq!(flags(&app), [(true, false), (false, false), (false, false)], "a fresh enabled gesture works after the disabled release");
            assert_eq!(app.session.doc().unwrap().history.undo.len(), undo + 2);
        }
        assert_eq!(selected(&app), [0]);
        assert!(app.session.doc().unwrap().doc.node(art).is_some());
        let changed = fr2_guide_column_state(&app);
        fr2_assert_guide_column_replay(&app, "disabled-panel boundary and enabled control");
        let expected_undo = if disabled { vec![after_first.clone(), initial] } else { vec![initial] };
        for expected in &expected_undo {
            app.run("edit.undo", json!({})).unwrap();
            assert_eq!(&fr2_guide_column_state(&app), expected);
            fr2_assert_guide_column_replay(&app, "disabled-panel Undo");
        }
        let expected_redo = if disabled { vec![after_first, changed] } else { vec![changed] };
        for expected in &expected_redo {
            app.run("edit.redo", json!({})).unwrap();
            assert_eq!(&fr2_guide_column_state(&app), expected);
            fr2_assert_guide_column_replay(&app, "disabled-panel Redo");
        }
    }
}

#[test]
fn guide_column_drag_preserves_rows_behind_keyboard_opened_preferences() {
    fn protected_state(app: &VectorcraftApp) -> Value {
        let st = app.session.active().unwrap();
        let entries = |v: &[vectorcraft_engine::HistoryEntry]| -> Vec<Value> {
            v.iter().map(|e| json!({"label": e.label, "doc": e.doc.as_ref(), "selection": e.selection})).collect()
        };
        json!({"doc": st.doc.as_ref(), "selection": st.selection,
            "undo": entries(&st.history.undo), "redo": entries(&st.history.redo), "limit": st.history.limit,
            "interaction": st.interaction.is_some(), "undo_group": st.undo_group.is_some(), "journal": &app.session.journal})
    }
    fn preferences_is_rendered(ctx: &egui::Context, out: &egui::FullOutput) -> bool {
        fn heading(shape: &egui::Shape, bounds: egui::Rect) -> bool {
            match shape {
                egui::Shape::Text(t) if t.galley.text() == "Preferences" => bounds.contains_rect(egui::Rect::from_min_size(t.pos, t.galley.size())),
                egui::Shape::Vec(v) => v.iter().any(|s| heading(s, bounds)),
                _ => false,
            }
        }
        ctx.memory(|m| m.area_rect(egui::Id::new("dialog-preferences")))
            .is_some_and(|bounds| bounds.width() > 0.0 && bounds.height() > 0.0 && out.shapes.iter().any(|s| heading(&s.shape, bounds)))
    }
    for modal in [false, true] {
        let (mut app, art) = scene();
        app.session.prefs.interface_language = "en".into();
        let mut h = ReviewWholeApp::new(app);
        let out = h.frame(vec![], Modifiers::NONE);
        let rows = guide_positions(&out);
        assert_eq!(rows.len(), 3);
        h.click(rows[0] + vec2(12.0, 0.0));
        assert_eq!(selected(&h.app), [0]);
        let initial = fr2_guide_column_state(&h.app);
        let undo = h.app.session.doc().unwrap().history.undo.len();
        let out = h.frame(vec![], Modifiers::NONE);
        let rows = guide_positions(&out);
        let eye = fr3_actual_guide_eye(&h, rows[1]);
        fr3_begin_held_guide_eye(&mut h, eye);
        assert_eq!(flags(&h.app), [(true, false), (false, false), (true, false)]);
        assert_eq!(selected(&h.app), [0]);
        assert_eq!(h.app.session.doc().unwrap().history.undo.len(), undo + 1);
        let after_first = fr2_guide_column_state(&h.app);
        let held = protected_state(&h.app);
        let out = h.frame(vec![], Modifiers::NONE);
        let held_rows = guide_positions(&out);
        let target = fr3_actual_guide_eye(&h, *held_rows.last().unwrap());
        if modal {
            assert!(!h.ctx.egui_wants_keyboard_input(), "the held column leaves the actual Preferences shortcut available");
            let sc = crate::shortcuts::parse(crate::menus::shortcut_of("edit.preferences").unwrap()).unwrap();
            h.key(sc.logical_key, sc.modifiers);
            assert_eq!(h.app.ui.dialog.as_ref().unwrap().kind, "preferences");
            let mut out = h.frame(vec![], Modifiers::NONE);
            for _ in 0..6 {
                if preferences_is_rendered(&h.ctx, &out) {
                    break;
                }
                out = h.frame(vec![], Modifiers::NONE);
            }
            assert!(preferences_is_rendered(&h.ctx, &out), "the real full-app Preferences window must be painted");
            let bounds = h.ctx.memory(|m| m.area_rect(egui::Id::new("dialog-preferences"))).unwrap();
            assert!(!bounds.contains(target), "the target is a visible background Guide row, outside Preferences");
            assert_eq!(guide_positions(&out), held_rows, "the rendered background rows keep their measured positions");
            assert!(h.ctx.input(|i| i.pointer.primary_down()), "opening the dialog did not fabricate a pointer release");
            assert!(h.ctx.data(|d| d.get_temp::<GuideColumnDrag>(key("guide-column")).is_some()));
            assert_eq!(protected_state(&h.app), held, "opening Preferences alone preserves Guide state and complete history/journal");
        }
        h.frame(vec![Event::PointerMoved(target)], Modifiers::NONE);
        assert!(h.ctx.input(|i| i.pointer.primary_down()), "the continuation is the same held gesture");
        if modal {
            assert_eq!(protected_state(&h.app), held, "a retained column gesture must not edit disabled background Guide rows");
            assert_eq!(flags(&h.app), [(true, false), (false, false), (true, false)]);
            assert_eq!(h.app.ui.dialog.as_ref().unwrap().kind, "preferences");
        } else {
            assert_eq!(flags(&h.app), [(true, false), (false, false), (false, false)], "the ordinary enabled continuation reaches original Guide 2");
            assert_eq!(h.app.session.doc().unwrap().history.undo.len(), undo + 1, "an uninterrupted owned segment remains one Undo");
        }
        h.frame(vec![button(target, false, Modifiers::NONE)], Modifiers::NONE);
        h.frame(vec![], Modifiers::NONE);
        assert!(!h.ctx.input(|i| i.pointer.primary_down()));
        assert!(h.ctx.data(|d| d.get_temp::<GuideColumnDrag>(key("guide-column")).is_none()), "release cleans up the cached column");
        assert!(h.app.session.doc().unwrap().doc.node(art).is_some());
        if modal {
            assert_eq!(protected_state(&h.app), held, "release while Preferences is open must not mutate the background");
            assert_eq!(h.app.ui.dialog.as_ref().unwrap().kind, "preferences", "releasing the held column leaves the modal open");
            h.key(egui::Key::Escape, Modifiers::NONE);
            assert!(h.app.ui.dialog.is_none());
            assert_eq!(protected_state(&h.app), held, "closing Preferences preserves both the completed Guide edit and its history");
            let out = h.frame(vec![], Modifiers::NONE);
            let rows = guide_positions(&out);
            let eye = fr3_actual_guide_eye(&h, *rows.last().unwrap());
            fr3_begin_held_guide_eye(&mut h, eye);
            h.frame(vec![button(eye + vec2(10.0, 0.0), false, Modifiers::NONE)], Modifiers::NONE);
            h.frame(vec![], Modifiers::NONE);
            assert_eq!(flags(&h.app), [(true, false), (false, false), (false, false)], "a fresh enabled gesture works after modal dismissal");
            assert_eq!(h.app.session.doc().unwrap().history.undo.len(), undo + 2, "the new gesture keeps its own Undo step");
            let after_recovery = fr2_guide_column_state(&h.app);
            fr2_assert_guide_column_replay(&h.app, "modal release and recovery");
            for expected in [&after_first, &initial] {
                h.app.run("edit.undo", json!({})).unwrap();
                assert_eq!(&fr2_guide_column_state(&h.app), expected);
                fr2_assert_guide_column_replay(&h.app, "modal recovery Undo");
            }
            for expected in [&after_first, &after_recovery] {
                h.app.run("edit.redo", json!({})).unwrap();
                assert_eq!(&fr2_guide_column_state(&h.app), expected);
                fr2_assert_guide_column_replay(&h.app, "modal recovery Redo");
            }
        } else {
            let changed = fr2_guide_column_state(&h.app);
            fr2_assert_guide_column_replay(&h.app, "ordinary held column positive");
            h.app.run("edit.undo", json!({})).unwrap();
            assert_eq!(fr2_guide_column_state(&h.app), initial);
            fr2_assert_guide_column_replay(&h.app, "ordinary held column Undo");
            h.app.run("edit.redo", json!({})).unwrap();
            assert_eq!(fr2_guide_column_state(&h.app), changed);
            fr2_assert_guide_column_replay(&h.app, "ordinary held column Redo");
        }
    }
}
