//! #2094: the shared egui shortcut route, not native-platform event delivery.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::Arc;

use egui::{Event, Key, Modifiers, PointerButton, Pos2, Rect, Shape, vec2};
use serde_json::{Value, json};
use vectorcraft_color::{Color, Paint};
use vectorcraft_doc::{Document, LineCap, LineJoin, NodeId, NodeKind, Selection};
use vectorcraft_engine::{HistoryEntry, Session};
use vectorcraft_ui_egui::{VectorcraftApp, menus};

type Entry = (String, Arc<Document>, Arc<Selection>);

#[derive(Clone, Debug, PartialEq)]
struct State {
    doc: Arc<Document>,
    selection: Selection,
    undo: Vec<Entry>,
    redo: Vec<Entry>,
    history_limit: usize,
    history_byte_budget: usize,
    dirty: bool,
    interaction: bool,
}

fn state(app: &VectorcraftApp) -> State {
    let st = app.session.doc().unwrap();
    let entries = |items: &[HistoryEntry]| items.iter().map(|e| (e.label.clone(), e.doc.clone(), e.selection.clone())).collect();
    State {
        doc: st.doc.clone(),
        selection: st.selection.clone(),
        undo: entries(&st.history.undo),
        redo: entries(&st.history.redo),
        history_limit: st.history.limit,
        history_byte_budget: st.history.byte_budget,
        dirty: st.is_dirty(),
        interaction: app.session.in_interaction(),
    }
}

struct Frames {
    app: VectorcraftApp,
    ctx: egui::Context,
    time: f64,
}

impl Frames {
    fn frame(&mut self, events: Vec<Event>) -> Vec<(String, Rect)> {
        self.time += 0.05;
        let raw = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1600.0, 1000.0))),
            time: Some(self.time),
            events,
            ..Default::default()
        };
        let mut out = self.ctx.run_ui(raw, |ui| {
            self.app.logic(ui.ctx());
            self.app.ui(ui);
        });
        out.textures_delta.clear();
        fn collect(shape: &Shape, clip: Rect, text: &mut Vec<(String, Rect)>) {
            match shape {
                Shape::Text(t) => {
                    let rect = t.visual_bounding_rect();
                    if clip.contains(rect.center()) {
                        text.push((t.galley.text().to_string(), rect));
                    }
                }
                Shape::Vec(shapes) => shapes.iter().for_each(|shape| collect(shape, clip, text)),
                _ => {}
            }
        }
        let mut text = vec![];
        for shape in &out.shapes {
            collect(&shape.shape, shape.clip_rect, &mut text);
        }
        text
    }

    fn settle(&mut self) {
        for _ in 0..6 {
            self.frame(vec![]);
        }
    }

    fn key(&mut self, key: Key, modifiers: Modifiers) {
        for pressed in [true, false] {
            self.frame(vec![Event::ModifiersChanged(modifiers), Event::Key { key, physical_key: None, pressed, repeat: false, modifiers }]);
        }
        self.frame(vec![Event::ModifiersChanged(Modifiers::NONE)]);
    }

    fn click(&mut self, at: Pos2) {
        self.frame(vec![Event::ModifiersChanged(Modifiers::NONE), Event::PointerMoved(at)]);
        for pressed in [true, false] {
            self.frame(vec![Event::PointerButton { pos: at, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE }]);
        }
        self.settle();
    }

    fn menu_outlines(&mut self) {
        let text = self.frame(vec![]);
        let titles: Vec<_> = text.iter().filter(|(s, rect)| s == "Type" && rect.center().y < 80.0).collect();
        assert_eq!(titles.len(), 1, "OUTLINE_SETUP: one visible Type menu title");
        self.click(titles[0].1.center());
        assert!(egui::Popup::is_any_open(&self.ctx), "OUTLINE_SETUP: pointer opens Type menu");
        let text = self.frame(vec![]);
        let entries: Vec<_> = text
            .iter()
            .filter(|(s, rect)| {
                s == "Create Outlines" && self.ctx.layer_id_at(rect.center()).is_some_and(|layer| layer.order == egui::Order::Foreground)
            })
            .collect();
        assert_eq!(entries.len(), 1, "OUTLINE_SETUP: one visible Create Outlines menu entry");
        self.click(entries[0].1.center());
        assert!(!egui::Popup::is_any_open(&self.ctx), "OUTLINE_MENU_CONTROL: selecting the command closes its menu");
    }
}

fn fixture() -> (Frames, NodeId, NodeId) {
    // Inter is already bundled and OFL-licensed in ASSETS.md; no system-only font or added asset.
    let face = vectorcraft_text::FontDb::global().face("Inter", "Regular").expect("OUTLINE_SETUP: bundled Inter Regular");
    assert_eq!(face.family, "Inter");
    let mut app = VectorcraftApp::new(Session::new(), Default::default());
    app.run("file.new", json!({"width": 600, "height": 400})).unwrap();
    let untouched = app.run("shape.rectangle", json!({"x": 400, "y": 250, "width": 40, "height": 30})).unwrap();
    let text = app
        .run("text.create", json!({"x": 100, "y": 140, "text": "bo", "font": "Inter", "style": "Regular", "size": 48, "color": "#3366cc"}))
        .unwrap();
    let id = NodeId(text["id"].as_u64().unwrap());
    app.run("text.setStyle", json!({"tracking": 80})).unwrap();
    app.run("paint.setStroke", json!({"color": "#993333"})).unwrap();
    app.run("stroke.set", json!({"weight": 1.5, "cap": "round", "join": "bevel"})).unwrap();
    app.run("object.move", json!({"dx": 10, "dy": 0})).unwrap();
    app.run("edit.undo", json!({})).unwrap();
    app.select_tool("selection");
    app.session.doc_mut().unwrap().mark_saved();
    let mut h = Frames { app, ctx: egui::Context::default(), time: 0.0 };
    h.settle();
    assert_eq!(menus::shortcut_of("type.createOutlines"), Some("Cmd+Shift+O"), "OUTLINE_SETUP: actual default chord");
    assert!(menus::enabled(&h.app, "type.createOutlines"));
    assert_eq!(h.app.session.doc().unwrap().selection.objects, [id]);
    assert!(matches!(h.app.session.doc().unwrap().doc.node(id).unwrap().kind, NodeKind::Text(_)));
    assert!(h.app.ui.dialog.is_none() && !h.ctx.egui_wants_keyboard_input());
    let before = state(&h.app);
    assert!(!before.dirty && !before.interaction && !before.undo.is_empty() && before.redo.len() == 1);
    (h, id, NodeId(untouched["id"].as_u64().unwrap()))
}

fn outline_art(h: &Frames, before: &State, text: NodeId, untouched: NodeId) -> Value {
    let after = state(&h.app);
    assert!(after.doc.node(text).is_none(), "OUTLINE_DISPATCH: selected text must become outlines");
    assert_eq!(after.selection.objects.len(), 1);
    let group = after.doc.node(after.selection.objects[0]).unwrap();
    let glyphs = group.children().expect("OUTLINE_ART: a group of glyphs");
    assert_eq!(glyphs.len(), 2, "OUTLINE_ART: original lowercase b and o, not empty or missing-font art");
    let mut art = vec![];
    for glyph in glyphs {
        assert!(matches!(glyph.kind, NodeKind::Compound { .. }), "OUTLINE_ART: both glyphs retain their counters");
        let bounds = glyph.geometric_bounds().expect("OUTLINE_ART: actual glyph geometry");
        assert!(bounds.width() > 0.0 && bounds.height() > 0.0);
        assert_eq!(glyph.appearance.fill_paint(), Paint::solid(Color::rgb8(51, 102, 204)));
        let stroke = glyph.appearance.stroke().expect("OUTLINE_ART: character stroke kept");
        assert_eq!(stroke.paint, Paint::solid(Color::rgb8(153, 51, 51)));
        assert_eq!((stroke.width, stroke.cap, stroke.join), (1.5, LineCap::Round, LineJoin::Bevel));
        let mut paths = vec![];
        glyph.walk(&mut |node| {
            if let Some(path) = node.path_data() {
                paths.push(path.clone());
            }
        });
        assert!(!paths.is_empty() && paths.iter().all(|path| !path.is_empty()));
        art.push(json!({"paths": paths, "appearance": glyph.appearance}));
    }
    assert_eq!(after.doc.node(untouched), before.doc.node(untouched), "unselected art is unchanged");
    assert_eq!(after.undo.len(), before.undo.len() + 1, "OUTLINE_HISTORY: exactly one edit");
    assert_eq!(&after.undo[..before.undo.len()], before.undo.as_slice());
    assert_eq!(after.undo.last().unwrap().0, "Create Outlines");
    assert!(after.redo.is_empty() && after.dirty && !after.interaction);
    assert_eq!((after.history_limit, after.history_byte_budget), (before.history_limit, before.history_byte_budget));
    json!({"glyphs": art, "opacity": group.opacity, "blend": group.blend, "name": group.name})
}

fn undo_redo(h: &mut Frames, before: &State, after: &State) {
    h.app.run("edit.undo", json!({})).unwrap();
    let undone = state(&h.app);
    assert_eq!(undone.doc, before.doc, "OUTLINE_HISTORY: Undo restores all original styled text and art");
    assert_eq!(undone.selection, before.selection);
    assert_eq!(undone.undo, before.undo);
    assert!(!undone.dirty && !undone.interaction);
    assert_eq!(undone.redo.len(), 1);
    assert_eq!(undone.redo[0].0, "Create Outlines");
    h.app.run("edit.redo", json!({})).unwrap();
    assert_eq!(&state(&h.app), after, "OUTLINE_HISTORY: Redo restores complete result and history");
}

#[test]
fn create_outlines_default_chord_matches_type_menu_and_history() {
    let (mut h, text, untouched) = fixture();
    let before = state(&h.app);
    h.menu_outlines();
    let menu_art = outline_art(&h, &before, text, untouched);
    let menu_after = state(&h.app);
    undo_redo(&mut h, &before, &menu_after);
    h.app.run("edit.undo", json!({})).unwrap();
    h.settle();
    assert_eq!(h.app.session.doc().unwrap().doc, before.doc);
    assert!(!h.ctx.egui_wants_keyboard_input(), "OUTLINE_SETUP: canvas owns the chord");
    // Ordinary non-Mac egui modifiers, not a claim about host Windows event delivery.
    h.key(Key::O, Modifiers { ctrl: true, command: true, shift: true, ..Modifiers::NONE });
    assert_eq!(outline_art(&h, &before, text, untouched), menu_art, "OUTLINE_SHORTCUT: same glyph geometry and style as the real menu");
    let shortcut_after = state(&h.app);
    undo_redo(&mut h, &before, &shortcut_after);
}

#[test]
fn create_outlines_chord_does_not_escape_focused_move_dialog() {
    let (mut h, text, _) = fixture();
    let before = state(&h.app);
    // Existing selection-tool Enter convention opens Move and focuses its first numeric field.
    h.key(Key::Enter, Modifiers::NONE);
    h.settle();
    assert_eq!(h.app.ui.dialog.as_ref().unwrap().kind, "move");
    assert!(h.ctx.egui_wants_keyboard_input(), "OUTLINE_SETUP: real dialog field owns keyboard focus");
    let fields = h.app.ui.dialog.as_ref().unwrap().fields.clone();
    let live = state(&h.app);
    h.key(Key::O, Modifiers { ctrl: true, command: true, shift: true, ..Modifiers::NONE });
    assert_eq!(state(&h.app), live, "OUTLINE_FOCUS_GATE: no document/selection/history change while the dialog owns input");
    assert_eq!(h.app.ui.dialog.as_ref().unwrap().fields, fields);
    assert!(matches!(h.app.session.doc().unwrap().doc.node(text).unwrap().kind, NodeKind::Text(_)));
    h.key(Key::Escape, Modifiers::NONE);
    h.settle();
    assert!(h.app.ui.dialog.is_none());
    assert_eq!(state(&h.app), before, "OUTLINE_FOCUS_GATE: Cancel restores the saved baseline and original Redo");
}
