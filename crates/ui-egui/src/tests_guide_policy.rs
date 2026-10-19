//! Global ruler-guide policy through full app frames. Original tiny scenes; no seeded selections.
//! Parent hook in lib.rs: #[cfg(test)] mod tests_guide_policy;
//! Prepared from source review only: these regressions have not been compiled or executed.

use egui::{Color32, Event, Key, Modifiers, PointerButton, Pos2, Rect, Shape, vec2};
use serde_json::{Value, json};
use vectorcraft_doc::NodeId;
use vectorcraft_engine::{HistoryEntry, Session};
use vectorcraft_geom::Point;

use crate::{VectorcraftApp, canvas::Xf, theme::Tokens};

const GUIDE_COLOR: Color32 = Color32::from_rgb(18, 171, 52);

struct App {
    app: VectorcraftApp,
    ctx: egui::Context,
    time: f64,
    art: NodeId,
}

impl App {
    fn new() -> Self {
        let mut app = VectorcraftApp::new(Session::new(), Default::default());
        for (key, value) in [("guideColor", json!("#12ab34")), ("guideStyle", json!("lines")), ("keyboardIncrement", json!(1))] {
            app.run("prefs.set", json!({"key": key, "value": value})).unwrap();
        }
        let art = original_document(&mut app, "GuidePolicyA", 40.0);
        // Unrelated snapping must not alter the guide-drag setup.
        app.ui.view.smart_guides = false;
        app.ui.view.snap_to_point = false;
        app.ui.view.snap_to_grid = false;
        app.ui.view.snap_to_pixel = false;
        app.select_tool("selection");
        let mut a = Self { app, ctx: egui::Context::default(), time: 0.0, art };
        for _ in 0..3 {
            a.frame(vec![], Modifiers::NONE);
        }
        assert!(a.app.canvas_rect.is_some(), "whole-window layout reached the canvas");
        assert!(a.app.ui.view.guides);
        assert!(!a.app.session.guides_locked());
        assert_ne!(Tokens::get(&a.ctx).selection, GUIDE_COLOR);
        a
    }

    fn frame(&mut self, mut events: Vec<Event>, modifiers: Modifiers) -> egui::FullOutput {
        self.time += 0.1;
        events.insert(0, Event::ModifiersChanged(modifiers));
        let mut raw = egui::RawInput {
            events,
            time: Some(self.time),
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1600.0, 900.0))),
            ..Default::default()
        };
        self.app.raw_input_hook(&mut raw);
        let app = &mut self.app;
        let mut out = self.ctx.run_ui(raw, |ui| {
            app.logic(ui.ctx());
            app.ui(ui);
        });
        out.textures_delta.clear();
        assert!(!self.app.ui.status.starts_with("Internal error:"), "{}", self.app.ui.status);
        out
    }

    fn idle(&mut self) -> egui::FullOutput {
        self.frame(vec![], Modifiers::NONE)
    }

    fn screen(&self, p: Point) -> Pos2 {
        let rect = self.app.canvas_rect.unwrap();
        let at = Xf::new(rect, self.app.view().unwrap()).to_screen(p);
        assert!(rect.contains(at), "the actual canvas contains the pointer target");
        at
    }

    fn click(&mut self, at: Pos2) -> egui::FullOutput {
        self.frame(vec![Event::PointerMoved(at)], Modifiers::NONE);
        self.frame(vec![button(at, true)], Modifiers::NONE);
        self.frame(vec![button(at, false)], Modifiers::NONE);
        self.idle()
    }

    fn click_guide(&mut self) -> egui::FullOutput {
        let x = self.app.session.doc().unwrap().doc.guides[0].pos;
        self.click(self.screen(Point::new(x, 80.0)))
    }

    fn clear_from_canvas(&mut self) -> egui::FullOutput {
        let out = self.click(self.screen(Point::new(120.0, 180.0)));
        let sel = &self.app.session.doc().unwrap().selection;
        assert!(sel.guides.is_empty() && sel.objects.is_empty());
        out
    }

    fn key(&mut self, key: Key, modifiers: Modifiers) -> egui::FullOutput {
        assert!(self.app.ui.dialog.is_none() && !self.app.ui.palette_open);
        assert!(!self.app.session.tool_busy(), "ordinary command shortcuts must be reachable");
        assert!(!self.ctx.egui_wants_keyboard_input(), "a text field must not swallow the policy key");
        let event = |pressed| Event::Key { key, physical_key: None, pressed, repeat: false, modifiers };
        self.frame(vec![event(true)], modifiers);
        self.frame(vec![event(false)], modifiers);
        self.idle()
    }

    fn tab(&mut self, title: &str, index: usize) {
        let out = self.idle();
        let at = out.shapes.iter().find_map(|s| tab_text(&s.shape, title)).expect("the current whole-window output contains the tab title");
        self.click(at);
        assert_eq!(self.app.session.active_index(), Some(index), "the actual tab click switched documents");
    }

    fn move_guide(&mut self) {
        let out = self.click_guide();
        assert_eq!(self.app.session.doc().unwrap().selection.guides, [0], "real canvas pick before the policy transition");
        assert_eq!(self.guide_colors(&out), [Tokens::get(&self.ctx).selection], "the positive selection paints the guide");
        let (from, to) = (self.screen(Point::new(40.0, 80.0)), self.screen(Point::new(60.0, 80.0)));
        self.frame(vec![Event::PointerMoved(from)], Modifiers::NONE);
        self.frame(vec![button(from, true)], Modifiers::NONE);
        self.frame(vec![Event::PointerMoved(to)], Modifiers::NONE);
        self.frame(vec![button(to, false)], Modifiers::NONE);
        self.idle();
        assert!((self.app.session.doc().unwrap().doc.guides[0].pos - 60.0).abs() < 0.01);
        assert_eq!(self.app.session.doc().unwrap().history.undo.last().unwrap().label, "Move Guide");
        assert_eq!(self.app.session.doc().unwrap().selection.guides, [0]);
    }

    fn policy_then_undo(&mut self, lock: bool) {
        self.move_guide();
        self.key(Key::Semicolon, if lock { Modifiers::COMMAND | Modifiers::ALT } else { Modifiers::COMMAND });
        assert!(self.app.session.doc().unwrap().selection.guides.is_empty(), "the active selection was cleared before restoration");
        self.key(Key::Z, Modifiers::COMMAND);
        let st = self.app.session.doc().unwrap();
        assert!((st.doc.guides[0].pos - 40.0).abs() < 0.01, "the actual Undo restored the moved guide");
        assert_eq!(st.history.redo.last().unwrap().label, "Move Guide");
        self.assert_policy(lock);
    }

    fn policy_then_tab(&mut self, lock: bool) {
        original_document(&mut self.app, "GuidePolicyB", 80.0);
        self.idle();
        self.tab("GuidePolicyA", 0);
        self.click_guide();
        assert_eq!(self.app.session.doc().unwrap().selection.guides, [0], "A's selection came from the canvas");
        self.tab("GuidePolicyB", 1);
        self.key(Key::Semicolon, if lock { Modifiers::COMMAND | Modifiers::ALT } else { Modifiers::COMMAND });
        self.tab("GuidePolicyA", 0);
        self.assert_policy(lock);
    }

    fn assert_policy(&self, lock: bool) {
        assert_eq!(self.app.ui.view.guides, lock, "Lock keeps display on; Hide turns it off");
        assert_eq!(self.app.session.guides_locked(), lock);
        let st = self.app.session.doc().unwrap();
        assert_eq!(st.doc.guides.len(), 1);
        assert!(st.doc.guides[0].visible && !st.doc.guides[0].locked, "global policy did not change individual flags");
    }

    /// Complete document/history contents for both tabs, including history selections.
    /// Live selection may be cleared as part of protecting a key; that is not a document edit.
    fn document_history(&self) -> Value {
        let docs: Vec<_> = self
            .app
            .session
            .documents()
            .iter()
            .map(|st| {
                json!({
                    "uid": st.uid,
                    "doc": st.doc.as_ref(),
                    "undo": history(&st.history.undo),
                    "redo": history(&st.history.redo),
                })
            })
            .collect();
        json!({"active": self.app.session.active_index(), "documents": docs})
    }

    /// The actual artboard-guide line, identified by its current screen geometry and clip.
    fn guide_colors(&self, out: &egui::FullOutput) -> Vec<Color32> {
        let st = self.app.session.doc().unwrap();
        let x = st.doc.guides[0].pos;
        let (a, b) = (self.screen(Point::new(x, 0.0)), self.screen(Point::new(x, 240.0)));
        let mut colors = vec![];
        for clipped in &out.shapes {
            if clipped.clip_rect.contains(a.lerp(b, 0.5)) {
                guide_lines(&clipped.shape, a, b, &mut colors);
            }
        }
        colors
    }

    fn hidden_key_is_inert(&mut self, key: Key) {
        self.assert_policy(false);
        let out = self.idle();
        assert!(self.guide_colors(&out).is_empty(), "global Hide suppresses the actual guide line");
        let before = self.document_history();
        let out = self.key(key, Modifiers::NONE);
        assert_eq!(self.document_history(), before, "{key:?} must not edit a globally hidden guide restored through ordinary UI");
        assert!(self.guide_colors(&out).is_empty());
        self.assert_policy(false);
    }

    fn locked_selection_is_inert(&mut self) {
        self.assert_policy(true);
        let restored = self.app.session.doc().unwrap().selection.guides.clone();
        let out = self.idle();
        let painted = self.guide_colors(&out);
        let before = self.document_history();
        for (key, modifiers) in
            [(Key::ArrowRight, Modifiers::NONE), (Key::ArrowRight, Modifiers::ALT), (Key::Delete, Modifiers::NONE), (Key::Backspace, Modifiers::NONE)]
        {
            self.key(key, modifiers);
            assert_eq!(self.document_history(), before, "{key:?}/{modifiers:?} must not edit/copy/delete globally locked guides");
        }
        // Probe a fresh real pick too, before reporting the restored selection/highlight.
        let out = self.click_guide();
        let picked = self.app.session.doc().unwrap().selection.guides.clone();
        assert_eq!(self.document_history(), before, "a locked guide click must not create edit history");
        assert!(restored.is_empty(), "global Lock must prune the guide selection restored by Undo/tab switching: {restored:?}");
        assert_eq!(painted, [GUIDE_COLOR], "a globally locked guide stays visible without stale selected highlighting");
        assert!(picked.is_empty(), "global Lock also rejects a fresh canvas pick");
        assert_eq!(self.guide_colors(&out), [GUIDE_COLOR]);
        self.assert_policy(true);
    }

    fn editable_keyboard_positive_control(&mut self) {
        assert!(self.app.ui.view.guides && !self.app.session.guides_locked());
        let out = self.clear_from_canvas();
        assert_eq!(self.guide_colors(&out), [GUIDE_COLOR]);
        let out = self.click_guide();
        let st = self.app.session.doc().unwrap();
        assert_eq!(st.selection.guides, [0], "Show/unlock restored real canvas picking");
        assert!(st.selection.objects.is_empty(), "the keys target only the guide");
        assert_eq!(self.guide_colors(&out), [Tokens::get(&self.ctx).selection]);
        let x = st.doc.guides[0].pos;
        let art = serde_json::to_value(st.doc.node(self.art).unwrap()).unwrap();
        let undo = st.history.undo.len();
        self.key(Key::ArrowRight, Modifiers::NONE);
        let st = self.app.session.doc().unwrap();
        assert!((st.doc.guides[0].pos - (x + 1.0)).abs() < 0.01, "positive control: the same ordinary arrow nudges an editable guide");
        assert_eq!(st.history.undo.len(), undo + 1);
        assert_eq!(st.history.undo.last().unwrap().label, "Move Guide");
        self.key(Key::Delete, Modifiers::NONE);
        let st = self.app.session.doc().unwrap();
        assert!(st.doc.guides.is_empty(), "positive control: ordinary Delete removes the editable guide");
        assert_eq!(st.history.undo.len(), undo + 2);
        assert_eq!(st.history.undo.last().unwrap().label, "Delete Guide");
        assert_eq!(serde_json::to_value(st.doc.node(self.art).unwrap()).unwrap(), art, "the original rectangle survives unchanged");
    }
}

fn original_document(app: &mut VectorcraftApp, name: &str, x: f64) -> NodeId {
    app.run("file.new", json!({"name": name, "width": 320, "height": 240})).unwrap();
    let art = NodeId(app.run("shape.rectangle", json!({"x": 200, "y": 150, "width": 20, "height": 20})).unwrap()["id"].as_u64().unwrap());
    app.run("select.none", json!({})).unwrap();
    app.run("guide.add", json!({"vertical": true, "pos": x, "artboard": 0})).unwrap();
    art
}

fn button(pos: Pos2, pressed: bool) -> Event {
    Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE }
}

fn history(entries: &[HistoryEntry]) -> Vec<Value> {
    entries.iter().map(|e| json!({"label": e.label, "doc": e.doc.as_ref(), "selection": e.selection})).collect()
}

fn tab_text(shape: &Shape, title: &str) -> Option<Pos2> {
    match shape {
        Shape::Text(t) if t.galley.text().starts_with(title) && t.galley.text().contains(" @ ") => {
            Some(Rect::from_min_size(t.pos, t.galley.size()).center())
        }
        Shape::Vec(shapes) => shapes.iter().find_map(|s| tab_text(s, title)),
        _ => None,
    }
}

fn guide_lines(shape: &Shape, a: Pos2, b: Pos2, colors: &mut Vec<Color32>) {
    match shape {
        Shape::LineSegment { points, stroke }
            if ((points[0] - a).length() < 0.1 && (points[1] - b).length() < 0.1)
                || ((points[0] - b).length() < 0.1 && (points[1] - a).length() < 0.1) =>
        {
            colors.push(stroke.color);
        }
        Shape::Vec(shapes) => shapes.iter().for_each(|s| guide_lines(s, a, b, colors)),
        _ => {}
    }
}

#[test]
fn hide_then_undo_blocks_delete_of_restored_guide_selection() {
    let mut a = App::new();
    a.policy_then_undo(false);
    a.hidden_key_is_inert(Key::Delete);
}

#[test]
fn hide_then_undo_blocks_nudge_of_restored_guide_selection() {
    let mut a = App::new();
    a.policy_then_undo(false);
    a.hidden_key_is_inert(Key::ArrowRight);
}

#[test]
fn hide_then_tab_switch_blocks_delete_of_restored_guide_selection() {
    let mut a = App::new();
    a.policy_then_tab(false);
    a.hidden_key_is_inert(Key::Delete);
}

#[test]
fn hide_then_tab_switch_blocks_nudge_of_restored_guide_selection() {
    let mut a = App::new();
    a.policy_then_tab(false);
    a.hidden_key_is_inert(Key::ArrowRight);
}

#[test]
fn lock_then_undo_prunes_restored_selection_and_selected_paint() {
    let mut a = App::new();
    a.policy_then_undo(true);
    a.locked_selection_is_inert();
}

#[test]
fn lock_then_tab_switch_prunes_restored_selection_and_selected_paint() {
    let mut a = App::new();
    a.policy_then_tab(true);
    a.locked_selection_is_inert();
}

#[test]
fn show_after_hidden_undo_restores_canvas_pick_nudge_and_delete() {
    let mut a = App::new();
    a.policy_then_undo(false);
    a.key(Key::Semicolon, Modifiers::COMMAND);
    assert!(a.app.ui.view.guides, "the ordinary Show shortcut ran");
    a.editable_keyboard_positive_control();
}

#[test]
fn unlock_restores_canvas_pick_nudge_and_delete() {
    let mut a = App::new();
    a.click_guide();
    assert_eq!(a.app.session.doc().unwrap().selection.guides, [0]);
    let out = a.key(Key::Semicolon, Modifiers::COMMAND | Modifiers::ALT);
    assert!(a.app.session.guides_locked(), "the ordinary Lock shortcut ran");
    assert!(a.app.session.doc().unwrap().selection.guides.is_empty());
    assert_eq!(a.guide_colors(&out), [GUIDE_COLOR]);
    a.key(Key::Semicolon, Modifiers::COMMAND | Modifiers::ALT);
    assert!(!a.app.session.guides_locked(), "the ordinary unlock shortcut ran");
    a.editable_keyboard_positive_control();
}
