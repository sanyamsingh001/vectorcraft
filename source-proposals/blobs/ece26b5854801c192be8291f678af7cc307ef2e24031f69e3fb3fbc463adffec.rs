//! A typed hex color must survive a pointer focus transfer to the earlier R field.
//! One full-application regression shared unchanged by main and the private Color composition.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::Arc;

use egui::{Event, Key, Modifiers, Pos2, Rect, Shape, vec2};
use serde_json::json;
use vectorcraft_color::{Color, Paint};
use vectorcraft_doc::{Document, NodeId, Selection};
use vectorcraft_engine::{HistoryEntry, Session};
use vectorcraft_ui_egui::VectorcraftApp;

struct AppFrames {
    app: VectorcraftApp,
    ctx: egui::Context,
    time: f64,
}

impl AppFrames {
    fn frame(&mut self, events: Vec<Event>) -> Vec<(String, Rect)> {
        self.time += 0.05;
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1600.0, 1000.0))),
            time: Some(self.time),
            focused: true,
            events,
            ..Default::default()
        };
        let app = &mut self.app;
        let mut output = self.ctx.run_ui(input, |ui| {
            app.logic(ui.ctx());
            app.ui(ui);
        });
        output.textures_delta.clear();
        fn collect(shape: &Shape, clip: Rect, out: &mut Vec<(String, Rect)>) {
            match shape {
                Shape::Text(text) => {
                    let rect = text.visual_bounding_rect();
                    if clip.contains(rect.center()) {
                        out.push((text.galley.text().to_string(), rect));
                    }
                }
                Shape::Vec(shapes) => shapes.iter().for_each(|shape| collect(shape, clip, out)),
                _ => {}
            }
        }
        let mut text = vec![];
        for shape in &output.shapes {
            collect(&shape.shape, shape.clip_rect, &mut text);
        }
        text
    }

    fn settle(&mut self) {
        for _ in 0..6 {
            self.frame(vec![]);
        }
    }

    fn label(&mut self, wanted: &str) -> Rect {
        let text = self.frame(vec![]);
        let dialog = self.ctx.memory(|m| m.area_rect(egui::Id::new(("dialog", "colorPicker"))));
        let dialog = dialog.expect("SETUP_COLOR_HEX: Color Picker is actually rendered");
        let found: Vec<_> = text
            .iter()
            .filter(|(label, rect)| {
                label == wanted
                    && dialog.contains(rect.center())
                    && self.ctx.layer_id_at(rect.center()).is_some_and(|layer| layer.order == egui::Order::Foreground)
            })
            .map(|(_, rect)| *rect)
            .collect();
        assert_eq!(found.len(), 1, "SETUP_COLOR_HEX: one visible foreground {wanted:?}, got {found:?}");
        found[0]
    }

    fn click(&mut self, at: Pos2) {
        self.frame(vec![Event::ModifiersChanged(Modifiers::NONE), Event::PointerMoved(at)]);
        for pressed in [true, false] {
            self.frame(vec![Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Modifiers::NONE }]);
        }
        self.frame(vec![]);
    }

    fn select_all_text(&mut self) {
        for pressed in [true, false] {
            self.frame(vec![
                Event::ModifiersChanged(Modifiers::COMMAND),
                Event::Key { key: Key::A, physical_key: None, pressed, repeat: false, modifiers: Modifiers::COMMAND },
            ]);
        }
        self.frame(vec![Event::ModifiersChanged(Modifiers::NONE)]);
    }

    fn focused_at(&self, at: Pos2) -> egui::Id {
        let id = self.ctx.memory(|m| m.focused()).expect("SETUP_COLOR_HEX: pointer gives a field focus");
        let response = self.ctx.read_response(id).expect("SETUP_COLOR_HEX: focused field has an actual response");
        assert!(response.rect.contains(at), "SETUP_COLOR_HEX: focused field contains the clicked text at {at:?}");
        id
    }
}

fn history(items: &[HistoryEntry]) -> Vec<(String, Arc<Document>, Arc<Selection>)> {
    items.iter().map(|entry| (entry.label.clone(), entry.doc.clone(), entry.selection.clone())).collect()
}

fn fill(app: &VectorcraftApp, id: NodeId) -> Color {
    app.session.doc().unwrap().doc.node(id).unwrap().appearance.fill().unwrap().paint.color().unwrap()
}

#[test]
fn color_hex_pointer_handoff_to_earlier_red_field_commits_on_ok() {
    let mut app = VectorcraftApp::new(Session::new(), Default::default());
    app.run("file.new", json!({"width": 640, "height": 480})).unwrap();
    let target = NodeId(app.run("shape.rectangle", json!({"x": 80, "y": 80, "width": 120, "height": 70})).unwrap()["id"].as_u64().unwrap());
    app.run("paint.setFill", json!({"color": "#336699"})).unwrap();
    let unrelated = NodeId(app.run("shape.rectangle", json!({"x": 280, "y": 100, "width": 90, "height": 60})).unwrap()["id"].as_u64().unwrap());
    app.run("paint.setFill", json!({"color": "#aa33cc"})).unwrap();
    app.run("select.set", json!({"ids": [target.0]})).unwrap();
    app.select_tool("selection");
    app.session.doc_mut().unwrap().mark_saved();
    let mut h = AppFrames { app, ctx: egui::Context::default(), time: 0.0 };
    h.settle();
    let st = h.app.session.doc().unwrap();
    let before_doc = st.doc.clone();
    let before_selection = st.selection.clone();
    let before_undo = history(&st.history.undo);
    let before_redo = history(&st.history.redo);
    let unrelated_before = before_doc.node(unrelated).unwrap().clone();
    assert!(!st.is_dirty() && !h.app.session.in_interaction(), "SETUP_COLOR_HEX: saved idle document");
    assert_eq!(before_selection.objects.iter().copied().collect::<Vec<_>>(), [target]);
    assert_eq!(fill(&h.app, target), Color::rgb8(51, 102, 153), "SETUP_COLOR_HEX: literal starting color");

    // Open through the public command; all focus, text entry and acceptance below are real events.
    h.app.run("ui.colorPicker", json!({"stroke": false})).unwrap();
    h.settle();
    let hex_at = h.label("336699").center();
    h.click(hex_at);
    let hex_focus = h.focused_at(hex_at);
    h.select_all_text();
    h.frame(vec![Event::Text("00FF00".into())]);
    h.settle();
    h.label("00FF00");
    assert_eq!(h.ctx.memory(|m| m.focused()), Some(hex_focus), "SETUP_COLOR_HEX: typed hex still owns focus");
    assert_eq!(h.app.session.doc().unwrap().doc, before_doc, "SETUP_COLOR_HEX: typing is still uncommitted");
    assert_eq!(history(&h.app.session.doc().unwrap().history.undo), before_undo);
    assert_eq!(history(&h.app.session.doc().unwrap().history.redo), before_redo);
    eprintln!("COLOR_HEX_POINTER_HANDOFF_READY: selected hex text is 00FF00; no Tab or Enter was sent");

    // R is rendered before # in both layouts. Locate its actual value, not its radio or a guessed id.
    let red_label = h.label("R");
    let red_value = h.label("51");
    assert!((red_label.center().y - red_value.center().y).abs() < 3.0, "SETUP_COLOR_HEX: 51 is on the R row");
    assert!(red_value.left() > red_label.right(), "SETUP_COLOR_HEX: click the numeric R box, not the radio");
    let red_at = red_value.center();
    h.click(red_at);
    let red_focus = h.focused_at(red_at);
    assert_ne!(red_focus, hex_focus, "SETUP_COLOR_HEX: real pointer moved focus from hex to R");
    eprintln!("COLOR_HEX_POINTER_HANDOFF_R_FOCUSED: earlier R field owns focus after pointer press/release");

    let ok_at = h.label("OK").center();
    h.click(ok_at);
    h.settle();
    assert!(h.app.ui.dialog.is_none(), "SETUP_COLOR_HEX: actual OK click closes the dialog");
    eprintln!("COLOR_HEX_POINTER_HANDOFF_OK_CLICKED: actual OK accepted the dialog");
    let after = h.app.session.doc().unwrap();
    assert_eq!(after.doc.node(unrelated).unwrap(), &unrelated_before, "unrelated artwork is unchanged");
    assert_eq!(after.selection, before_selection, "the selected object is unchanged");
    assert!(!h.app.session.in_interaction(), "OK settles the preview interaction");
    assert_eq!(
        fill(&h.app, target),
        Color::rgb8(0, 255, 0),
        "COLOR_HEX_POINTER_HANDOFF_COMMITS_GREEN: the typed 00FF00 must survive clicking R before actual OK"
    );

    // Independent literal oracle: the sole document change is this fill, not a command replay.
    let mut expected = (*before_doc).clone();
    expected.node_mut(target).unwrap().appearance.fill_mut().unwrap().paint = Paint::solid(Color::rgb8(0, 255, 0));
    assert_eq!(&*after.doc, &expected, "only the selected fill changes in the complete document");
    let after_undo = history(&after.history.undo);
    assert_eq!(after_undo.len(), before_undo.len() + 1, "one color edit, not one per focus/preview frame");
    assert_eq!(&after_undo[..before_undo.len()], before_undo.as_slice(), "all earlier Undo entries are retained");
    assert_eq!(after_undo.last().unwrap().0, "Fill Color");
    assert_eq!(after_undo.last().unwrap().1, before_doc);
    assert_eq!(&*after_undo.last().unwrap().2, &before_selection);
    assert!(after.history.redo.is_empty() && after.is_dirty());

    h.app.run("edit.undo", json!({})).unwrap();
    let undone = h.app.session.doc().unwrap();
    assert_eq!(undone.doc, before_doc, "one Undo restores the whole original document");
    assert_eq!(undone.selection, before_selection);
    assert_eq!(history(&undone.history.undo), before_undo);
    assert_eq!(undone.history.redo.len(), 1);
    assert!(!undone.is_dirty());
    h.app.run("edit.redo", json!({})).unwrap();
    let redone = h.app.session.doc().unwrap();
    assert_eq!(&*redone.doc, &expected, "one Redo restores the literal green result");
    assert_eq!(redone.selection, before_selection);
    assert_eq!(history(&redone.history.undo), after_undo);
    assert!(redone.history.redo.is_empty() && redone.is_dirty());
    eprintln!("COLOR_HEX_POINTER_HANDOFF_GREEN_POSTCONDITIONS: exact document and one-step Undo/Redo verified");
}

// Saved floating layouts remain a supported public UiState configuration. They draw
// after the canvas; a new window.panel.float command selects shared docking instead.
impl AppFrames {
    fn floating_color_text(&mut self, wanted: &str) -> Rect {
        let text = self.frame(vec![]);
        assert!(self.app.ui.docking.is_none(), "SETUP_HEX_OWNER: saved floating layout remains active");
        let area = vectorcraft_ui_egui::floating::area_id("color");
        let panel = self.ctx.memory(|m| m.area_rect(area)).expect("SETUP_HEX_OWNER: floating Color is rendered");
        let found: Vec<_> = text
            .iter()
            .filter(|(label, rect)| {
                label == wanted && panel.contains(rect.center()) && self.ctx.layer_id_at(rect.center()).is_some_and(|layer| layer.id == area)
            })
            .map(|(_, rect)| *rect)
            .collect();
        assert_eq!(found.len(), 1, "SETUP_HEX_OWNER: one visible floating Color {wanted:?}, got {found:?}");
        found[0]
    }
}

#[test]
fn color_hex_floating_selection_change_keeps_original_object_colors() {
    let mut app = VectorcraftApp::new(Session::new(), Default::default());
    app.run("file.new", json!({"width": 640, "height": 480})).unwrap();
    let a = NodeId(app.run("shape.rectangle", json!({"x": 80, "y": 80, "width": 120, "height": 70})).unwrap()["id"].as_u64().unwrap());
    app.run("paint.setFill", json!({"color": "#336699"})).unwrap();
    let b = NodeId(app.run("shape.rectangle", json!({"x": 400, "y": 320, "width": 100, "height": 80})).unwrap()["id"].as_u64().unwrap());
    app.run("paint.setFill", json!({"color": "#AA33CC"})).unwrap();
    app.run("select.set", json!({"ids": [a.0]})).unwrap();
    app.select_tool("selection");
    app.run("paint.toggleActive", json!({"fill": true})).unwrap();
    app.run("ui.colorPanel", json!({"mode": "rgb", "options": true})).unwrap();

    // Layout-only setup, equivalent to the existing saved floating_panels preference.
    // No egui focus, draft, widget memory, tool state or interaction is seeded.
    assert!(app.ui.docking.is_none() && app.ui.floating_panels.is_empty());
    app.ui.floating_panels.push(vectorcraft_ui_egui::state::FloatingPanels {
        panels: vec!["color".into()],
        pos: [100.0, 140.0],
        ..Default::default()
    });
    app.session.doc_mut().unwrap().mark_saved();
    let mut h = AppFrames { app, ctx: egui::Context::default(), time: 0.0 };
    h.settle();
    assert!(h.app.session.fill_active && !h.app.ui.color_hide_options);
    assert_eq!(h.app.ui.color_mode, Some(vectorcraft_ui_egui::panels::color::Mode::Rgb));
    assert!(h.app.ui.dialog.is_none() && h.app.ui.docking.is_none());
    let st = h.app.session.doc().unwrap();
    let document_uid = st.uid;
    let before_doc = st.doc.clone();
    let before_selection = Selection { objects: vec![a], ..Default::default() };
    let expected_selection = Selection { objects: vec![b], ..Default::default() };
    let before_undo = history(&st.history.undo);
    let before_redo = history(&st.history.redo);
    assert_eq!(st.selection, before_selection, "SETUP_HEX_OWNER: only A is selected");
    assert!(!st.is_dirty() && !h.app.session.in_interaction(), "SETUP_HEX_OWNER: saved idle document");
    assert_eq!(fill(&h.app, a), Color::rgb8(51, 102, 153));
    assert_eq!(fill(&h.app, b), Color::rgb8(170, 51, 204));
    eprintln!("HEX_SELECTION_OWNER_SETUP_READY: saved floating Color; RGB Fill; distinct solid A/B; only A selected");

    let hex_at = h.floating_color_text("336699").center();
    h.click(hex_at);
    let hex_focus = h.focused_at(hex_at);
    h.select_all_text();
    h.frame(vec![Event::Text("00FF00".into())]);
    h.settle();
    h.floating_color_text("00FF00");
    assert_eq!(h.ctx.memory(|m| m.focused()), Some(hex_focus), "SETUP_HEX_OWNER: actual Hex field still focused");
    let pending = h.app.session.doc().unwrap();
    assert_eq!(pending.doc, before_doc, "SETUP_HEX_OWNER: typed draft has not changed either object");
    assert_eq!(pending.selection, before_selection);
    assert_eq!(history(&pending.history.undo), before_undo);
    assert_eq!(history(&pending.history.redo), before_redo);
    assert!(!pending.is_dirty() && !h.app.session.in_interaction());
    eprintln!("HEX_SELECTION_OWNER_DRAFT_READY: actual pointer/keyboard typed 00FF00 for A; no commit or Tab/Enter/Escape");

    // The center of B's literal rectangle is transformed through the actual laid-out canvas.
    // No guessed screen coordinate or fallback target; fail setup if the canvas is covered.
    let canvas = h.ctx.read_response(egui::Id::new("canvas")).expect("SETUP_HEX_OWNER: canvas response exists");
    let canvas_rect = h.app.canvas_rect.expect("SETUP_HEX_OWNER: canvas was laid out");
    let xf = vectorcraft_ui_egui::canvas::Xf::new(canvas_rect, h.app.view().unwrap());
    let b_at = xf.to_screen(vectorcraft_geom::Point::new(450.0, 360.0));
    let panel = h.ctx.memory(|m| m.area_rect(vectorcraft_ui_egui::floating::area_id("color"))).unwrap();
    assert!(canvas_rect.shrink(8.0).contains(b_at) && canvas.rect.contains(b_at));
    assert!(!panel.contains(b_at), "SETUP_HEX_OWNER: B's center is outside floating Color");
    assert_eq!(h.ctx.layer_id_at(b_at), Some(canvas.layer_id), "SETUP_HEX_OWNER: B is exposed on the canvas layer");
    h.frame(vec![Event::ModifiersChanged(Modifiers::NONE), Event::PointerMoved(b_at)]);
    let hovered = h.ctx.read_response(egui::Id::new("canvas")).unwrap();
    assert!(hovered.hovered(), "SETUP_HEX_OWNER: normal pointer reached the exposed canvas");
    assert_eq!(h.ctx.memory(|m| m.focused()), Some(hex_focus), "SETUP_HEX_OWNER: hovering B alone does not commit");
    assert_eq!(h.app.session.doc().unwrap().doc, before_doc);
    assert_eq!(h.app.session.doc().unwrap().selection, before_selection);
    eprintln!("HEX_SELECTION_OWNER_TARGET_READY: B={b:?} actual canvas center={b_at:?}; Hex still focused");

    h.frame(vec![Event::PointerButton { pos: b_at, button: egui::PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE }]);
    assert_eq!(h.app.session.doc().unwrap().selection, expected_selection, "SETUP_HEX_OWNER: actual canvas press selects B");
    assert_ne!(h.ctx.memory(|m| m.focused()), Some(hex_focus), "SETUP_HEX_OWNER: actual canvas press ends Hex focus");
    eprintln!("HEX_SELECTION_OWNER_TARGET_CAPTURED: normal primary press selected only B and removed Hex focus");

    // Release at the identical position, with no intervening move, drag, command or Escape.
    h.frame(vec![Event::PointerButton { pos: b_at, button: egui::PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE }]);
    h.settle();
    let after = h.app.session.doc().unwrap();
    assert_eq!(after.uid, document_uid, "SETUP_HEX_OWNER: same active document");
    assert_eq!(after.selection, expected_selection, "SETUP_HEX_OWNER: B stays selected after release");
    assert!(h.app.ui.docking.is_none() && h.app.ui.dialog.is_none());
    assert!(!h.ctx.input(|i| i.pointer.primary_down()));
    eprintln!("HEX_SELECTION_OWNER_RELEASED: exact-position release completed; selection B; late complete-state oracle next");

    // Independent unchanged-document oracle, not a replay of a paint command. History snapshots
    // include every label, document and selection on both Undo and Redo stacks.
    let after_undo = history(&after.history.undo);
    let after_redo = history(&after.history.redo);
    assert_eq!(
        (
            fill(&h.app, a),
            fill(&h.app, b),
            &*after.doc,
            &after.selection,
            &after_undo,
            &after_redo,
            (after.is_dirty(), h.app.session.in_interaction()),
        ),
        (Color::rgb8(51, 102, 153), Color::rgb8(170, 51, 204), &*before_doc, &expected_selection, &before_undo, &before_redo, (false, false),),
        "HEX_SELECTION_OWNER_PRESERVES_DOCUMENT: selecting B must not apply A's pending Hex draft to either object"
    );
    eprintln!("HEX_SELECTION_OWNER_UNCHANGED: both literal fills, complete document, selection B and complete Undo/Redo snapshots unchanged");
}
