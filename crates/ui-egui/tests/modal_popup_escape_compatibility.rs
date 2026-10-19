//! Private compatibility checks for the composed modal Escape, Warp and Average and Join fixes.
//! Uses original application frames and the existing model/semantic-focus oracles.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::Arc;

use egui::{Event, Key, Modifiers, Pos2, Rect, Shape, vec2};
use serde_json::{Map, Value, json};
use vectorcraft_doc::{Document, Selection};
use vectorcraft_engine::{HistoryEntry, Session};
use vectorcraft_ui_egui::{VectorcraftApp, menus};

#[derive(Debug, PartialEq)]
struct HistoryState {
    undo: Vec<(String, Arc<Document>, Arc<Selection>)>,
    redo: Vec<(String, Arc<Document>, Arc<Selection>)>,
    limit: usize,
    byte_budget: usize,
}

#[derive(Debug, PartialEq)]
struct InteractionState {
    label: String,
    doc: Arc<Document>,
    selection: Arc<Selection>,
    preview: Option<(String, Value)>,
}

#[derive(Debug, PartialEq)]
struct ModelState {
    doc: Arc<Document>,
    selection: Selection,
    history: HistoryState,
    dirty: bool,
    interaction: Option<InteractionState>,
    remembered_offset_options: Value,
    last_effect: Option<(String, Value)>,
}

fn model(app: &VectorcraftApp) -> ModelState {
    let st = app.session.doc().unwrap();
    let entries = |items: &[HistoryEntry]| items.iter().map(|e| (e.label.clone(), e.doc.clone(), e.selection.clone())).collect();
    ModelState {
        doc: st.doc.clone(),
        selection: st.selection.clone(),
        history: HistoryState {
            undo: entries(&st.history.undo),
            redo: entries(&st.history.redo),
            limit: st.history.limit,
            byte_budget: st.history.byte_budget,
        },
        dirty: st.is_dirty(),
        interaction: st.interaction.as_ref().map(|i| InteractionState {
            label: i.label.clone(),
            doc: i.doc.clone(),
            selection: i.selection.clone(),
            preview: i.preview.clone(),
        }),
        remembered_offset_options: app.ui.offset_path.clone(),
        last_effect: app.last_effect.clone(),
    }
}

struct AppFrames {
    app: VectorcraftApp,
    ctx: egui::Context,
    time: f64,
}

fn texts(output: &egui::FullOutput) -> Vec<(String, Rect)> {
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
    let mut out = vec![];
    for shape in &output.shapes {
        collect(&shape.shape, shape.clip_rect, &mut out);
    }
    out
}

impl AppFrames {
    fn frame(&mut self, events: Vec<Event>) -> egui::FullOutput {
        self.time += 0.05;
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1600.0, 1000.0))),
            time: Some(self.time),
            events,
            ..Default::default()
        };
        let app = &mut self.app;
        let mut output = self.ctx.run_ui(input, |ui| {
            app.logic(ui.ctx());
            app.ui(ui);
        });
        output.textures_delta.clear();
        output
    }

    fn settle(&mut self) {
        for _ in 0..6 {
            let _ = self.frame(vec![]);
        }
    }

    fn key(&mut self, key: Key, pressed: bool) -> egui::FullOutput {
        self.frame(vec![
            Event::ModifiersChanged(Modifiers::NONE),
            Event::Key { key, physical_key: None, pressed, repeat: false, modifiers: Modifiers::NONE },
        ])
    }

    fn tap(&mut self, key: Key) {
        let _ = self.key(key, true);
        let _ = self.key(key, false);
    }

    fn tab_to(&mut self, label: &str, typ: egui::WidgetType) -> egui::Id {
        // As in the existing Enter controls, identify the actual focus announcement and
        // topmost response. A popup Area containing the same label is not the option.
        for _ in 0..64 {
            let mut target = None;
            for pressed in [true, false] {
                let output = self.key(Key::Tab, pressed);
                let named = output.platform_output.events.iter().any(|event| {
                    let egui::output::OutputEvent::FocusGained(info) = event else { return false };
                    info.enabled
                        && info.typ == typ
                        && match typ {
                            egui::WidgetType::ComboBox => info.current_text_value.as_deref() == Some(label),
                            egui::WidgetType::Button => info.label.as_deref() == Some(label),
                            _ => false,
                        }
                });
                if !named {
                    continue;
                }
                let id = self.ctx.memory(|m| m.focused()).expect("SETUP: announced widget has focus");
                let response = self.ctx.read_response(id).expect("SETUP: current focused response");
                let matches: Vec<_> = texts(&output)
                    .into_iter()
                    .filter(|(text, rect)| {
                        text == label
                            && response.rect.contains(rect.center())
                            && response.interact_rect.contains(rect.center())
                            && self.ctx.layer_id_at(rect.center()) == Some(response.layer_id)
                    })
                    .collect();
                assert!(response.has_focus(), "SETUP: semantic target owns focus");
                assert_eq!(matches.len(), 1, "SETUP: one visible focused {label}");
                target = Some((id, matches[0].1.center(), response.layer_id));
            }
            if let Some((id, point, layer)) = target {
                assert_eq!(self.ctx.memory(|m| m.focused()), Some(id), "SETUP: focus survives Tab release");
                let response = self.ctx.read_response(id).unwrap();
                assert!(response.has_focus() && response.rect.contains(point) && response.interact_rect.contains(point));
                assert_eq!(response.layer_id, layer);
                assert_eq!(self.ctx.layer_id_at(point), Some(layer));
                return id;
            }
        }
        panic!("SETUP: real Tab traversal did not focus {typ:?} {label}");
    }

    fn click_text(&mut self, label: &str) {
        let output = self.frame(vec![]);
        let matches: Vec<_> = texts(&output)
            .into_iter()
            .filter(|(text, rect)| text == label && self.ctx.layer_id_at(rect.center()).is_some_and(|layer| layer.order == egui::Order::Foreground))
            .collect();
        assert_eq!(matches.len(), 1, "SETUP: one visible foreground {label}");
        let at = matches[0].1.center();
        let _ = self.frame(vec![Event::ModifiersChanged(Modifiers::NONE), Event::PointerMoved(at)]);
        for pressed in [true, false] {
            let _ = self.frame(vec![Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Modifiers::NONE }]);
        }
        let _ = self.frame(vec![]);
    }

    fn open_combo(&mut self, label: &str) -> egui::Id {
        assert!(!egui::Popup::is_any_open(&self.ctx), "SETUP: list starts closed");
        let id = self.tab_to(label, egui::WidgetType::ComboBox);
        self.tap(Key::Enter);
        assert!(self.app.ui.dialog.is_some(), "COMPAT_ENTER_OPEN_KEEPS_DIALOG");
        assert!(egui::ComboBox::is_open(&self.ctx, id), "SETUP: Enter opens the named combo");
        id
    }
}

fn assert_kept(h: &AppFrames, kind: &str, live: &ModelState, fields: &Map<String, Value>) {
    assert!(h.app.ui.dialog.is_some(), "COMPAT_FIRST_ESCAPE_KEEPS_DIALOG");
    let d = h.app.ui.dialog.as_ref().unwrap();
    assert_eq!(d.kind, kind);
    assert_eq!(&d.fields, fields, "Escape preserves the latest options");
    assert_eq!(&model(&h.app), live, "Escape preserves the live art, selection, both history stacks and saved state");
    assert!(Arc::ptr_eq(&h.app.session.doc().unwrap().doc, &live.doc), "Escape retains the exact live document snapshot");
    assert!(!egui::Popup::is_any_open(&h.ctx), "COMPAT_FIRST_ESCAPE_CLOSES_LIST");
}

fn assert_cancelled(h: &AppFrames, before: &ModelState) {
    assert!(h.app.ui.dialog.is_none(), "COMPAT_SECOND_ESCAPE_CANCELS_DIALOG");
    assert_eq!(&model(&h.app), before, "Cancel restores the complete pre-dialog state");
    assert!(Arc::ptr_eq(&h.app.session.doc().unwrap().doc, &before.doc), "Cancel restores the exact saved snapshot");
    assert!(!egui::Popup::is_any_open(&h.ctx));
}

fn dismiss_then_cancel(h: &mut AppFrames, kind: &str, live: &ModelState, fields: &Map<String, Value>, before: &ModelState) {
    let _ = h.key(Key::Escape, true);
    assert_kept(h, kind, live, fields);
    let _ = h.key(Key::Escape, false);
    let _ = h.frame(vec![]);
    assert_kept(h, kind, live, fields);
    let _ = h.key(Key::Escape, true);
    assert_cancelled(h, before);
    let _ = h.key(Key::Escape, false);
    let _ = h.frame(vec![]);
    assert_cancelled(h, before);
}

#[test]
fn modal_popup_escape_keyboard_warp_style_keeps_preview_then_cancels() {
    let mut app = VectorcraftApp::new(Session::new(), Default::default());
    app.run("file.new", json!({"width": 400, "height": 300})).unwrap();
    let id = app.run("shape.rectangle", json!({"x": 50, "y": 60, "width": 120, "height": 80})).unwrap()["id"].as_u64().unwrap();
    app.run("object.move", json!({"dx": 10, "dy": 0})).unwrap();
    let moved = app.session.doc().unwrap().doc.clone();
    app.run("edit.undo", json!({})).unwrap();
    app.session.doc_mut().unwrap().mark_saved();
    let mut h = AppFrames { app, ctx: egui::Context::default(), time: 0.0 };
    h.settle();
    let before = model(&h.app);
    assert!(!before.dirty && before.interaction.is_none());
    assert!(!before.history.undo.is_empty() && before.history.redo.len() == 1);
    h.app.run("effect.dialog", json!({"effect": "warp.arc", "item": null})).unwrap();
    h.settle();
    assert_eq!(h.app.ui.dialog.as_ref().unwrap().str("__effect"), "warp.arc");
    assert!(h.app.ui.dialog.as_ref().unwrap().bool("preview"));

    let style = h.open_combo("Arc");
    h.tab_to("Arc Lower", egui::WidgetType::Button);
    h.tap(Key::Enter);
    assert!(h.app.ui.dialog.is_some(), "COMPAT_ENTER_CHOICE_KEEPS_DIALOG");
    assert!(!egui::ComboBox::is_open(&h.ctx, style));
    assert_eq!(h.app.ui.dialog.as_ref().unwrap().str("__effect"), "warp.arcLower");
    assert!(h.app.ui.dialog.as_ref().unwrap().bool("__warp_style_changed"));
    assert_eq!(model(&h.app).history, before.history);

    // Use the actual checkbox transition, never assign Preview or dialog fields.
    h.click_text("Preview");
    assert!(!h.app.ui.dialog.as_ref().unwrap().bool("preview"));
    assert_eq!(model(&h.app), before);
    h.click_text("Preview");
    h.settle();
    let live = model(&h.app);
    assert!(live.dirty && live.interaction.is_some(), "SETUP: actual Warp preview");
    assert_ne!(live.doc, before.doc);
    assert_eq!(live.history, before.history);
    assert_eq!(live.selection, before.selection);
    assert_eq!(live.last_effect, before.last_effect);
    let effects = &live.doc.node(vectorcraft_doc::NodeId(id)).unwrap().appearance.effects;
    assert_eq!(effects.len(), 1);
    assert_eq!(effects[0].id, "warp.arcLower");
    let d = h.app.ui.dialog.as_ref().unwrap();
    assert!(d.bool("preview"));
    let (kind, fields) = (d.kind.clone(), d.fields.clone());
    h.open_combo("Arc Lower");
    assert_eq!(model(&h.app), live, "reopening Style does not edit the preview/history");
    assert_eq!(h.app.ui.dialog.as_ref().unwrap().fields, fields);

    dismiss_then_cancel(&mut h, &kind, &live, &fields, &before);
    h.app.run("edit.redo", json!({})).unwrap();
    assert_eq!(h.app.session.doc().unwrap().doc, moved, "the original Redo remains usable");
}

#[test]
fn modal_popup_escape_keyboard_average_kind_keeps_options_then_cancels() {
    let mut app = VectorcraftApp::new(Session::new(), Default::default());
    app.run("file.new", json!({"width": 400, "height": 300})).unwrap();
    let a = app.run("shape.line", json!({"x1": 20, "y1": 100, "x2": 100, "y2": 100})).unwrap()["id"].clone();
    let b = app.run("shape.line", json!({"x1": 140, "y1": 140, "x2": 240, "y2": 100})).unwrap()["id"].clone();
    app.select_tool("directSelection");
    app.run("object.move", json!({"dx": 5, "dy": 0})).unwrap();
    let moved = app.session.doc().unwrap().doc.clone();
    app.run("edit.undo", json!({})).unwrap();
    app.run("select.anchorsMany", json!({"items": [{"id": a, "anchors": [[0, 1]]}, {"id": b, "anchors": [[0, 0]]}]})).unwrap();
    app.session.doc_mut().unwrap().mark_saved();
    let mut h = AppFrames { app, ctx: egui::Context::default(), time: 0.0 };
    h.settle();
    let before = model(&h.app);
    assert!(!before.dirty && before.interaction.is_none());
    assert!(!before.history.undo.is_empty() && before.history.redo.len() == 1);
    assert!(menus::enabled(&h.app, "path.averageJoin"));
    menus::invoke(&mut h.app, "path.averageJoin", json!({}));
    h.settle();
    assert_eq!(h.app.ui.dialog.as_ref().unwrap().kind, "averageJoin");
    assert_eq!(h.app.ui.dialog.as_ref().unwrap().str("kind"), "corner");
    assert_eq!(model(&h.app), before);

    let combo = h.open_combo("Corner");
    h.tab_to("Smooth", egui::WidgetType::Button);
    h.tap(Key::Enter);
    assert!(h.app.ui.dialog.is_some(), "COMPAT_ENTER_CHOICE_KEEPS_DIALOG");
    assert!(!egui::ComboBox::is_open(&h.ctx, combo));
    assert_eq!(h.app.ui.dialog.as_ref().unwrap().str("kind"), "smooth");
    assert_eq!(model(&h.app), before, "choosing Kind is neither a commit nor a preview");
    let fields = h.app.ui.dialog.as_ref().unwrap().fields.clone();
    h.open_combo("Smooth");
    assert_eq!(h.app.ui.dialog.as_ref().unwrap().fields, fields);
    assert_eq!(model(&h.app), before);

    dismiss_then_cancel(&mut h, "averageJoin", &before, &fields, &before);
    h.app.run("edit.redo", json!({})).unwrap();
    assert_eq!(h.app.session.doc().unwrap().doc, moved, "the original Redo remains usable");
}
