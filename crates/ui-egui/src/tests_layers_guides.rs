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
            Shape::Text(t) if t.galley.text() == name => Some(t.pos + vec2(0.0, t.galley.size().y / 2.0)),
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
