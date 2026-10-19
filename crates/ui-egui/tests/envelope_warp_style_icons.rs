//! Envelope style choices use real pointer/keyboard input and retain preview/history semantics.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::Arc;

use egui::{Event, Key, Modifiers, Pos2, Rect, Shape, vec2};
use serde_json::{Value, json};
use vectorcraft_doc::{Document, Node, NodeId, NodeKind, Selection, live::EnvelopeKind};
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
        assert!(self.app.ui.dialog.is_some(), "ENVELOPE_STYLE_ENTER_KEEPS_DIALOG");
        assert!(egui::ComboBox::is_open(&self.ctx, id), "SETUP: Enter opens the named combo");
        id
    }
}

fn choose_style(h: &mut AppFrames, keyboard: bool) {
    if keyboard {
        let combo = h.open_combo("Arc");
        h.tab_to("Arc Lower", egui::WidgetType::Button);
        h.tap(Key::Enter);
        assert!(!egui::ComboBox::is_open(&h.ctx, combo), "choosing Style closes only its list");
    } else {
        h.click_text("Arc");
        assert!(egui::Popup::is_any_open(&h.ctx));
        h.click_text("Arc Lower");
        assert!(!egui::Popup::is_any_open(&h.ctx));
    }
    assert!(h.app.ui.dialog.is_some(), "choosing Style must not accept the dialog");
    assert_eq!(h.app.ui.dialog.as_ref().unwrap().str("style"), "arcLower");
}

fn assert_envelope(app: &VectorcraftApp, original: &Node, reset: bool) {
    let st = app.session.doc().unwrap();
    assert_eq!(st.selection.objects.len(), 1);
    let node = st.doc.node(st.selection.objects[0]).unwrap();
    let NodeKind::Envelope { content, kind, .. } = &node.kind else { panic!("expected preview envelope") };
    assert_eq!(content.len(), 1);
    assert_eq!(content[0].as_ref(), original, "Style keeps the original content geometry, identity and appearance");
    let expected = if reset { (-30.0, 20.0, -15.0, false) } else { (50.0, 0.0, 0.0, true) };
    let EnvelopeKind::Warp { style, bend, h, v, horizontal } = &**kind else { panic!("expected Warp") };
    assert_eq!(style, "arcLower");
    assert_eq!((*bend, *h, *v, *horizontal), expected, "Style retains the other Envelope parameters");
}

#[test]
fn envelope_warp_style_pointer_and_keyboard_preserve_make_reset_preview_history() {
    for reset in [false, true] {
        for keyboard in [false, true] {
            let mut app = VectorcraftApp::new(Session::new(), Default::default());
            app.session.prefs.interface_language = "en".into();
            app.run("file.new", json!({"width": 400, "height": 300})).unwrap();
            let id = app.run("shape.rectangle", json!({"x": 50, "y": 60, "width": 120, "height": 80})).unwrap()["id"].as_u64().unwrap();
            let original = app.session.doc().unwrap().doc.node(NodeId(id)).unwrap().clone();
            if reset {
                app.run("object.envelope.makeWithWarp", json!({"style": "arc", "bend": -30, "h": 20, "v": -15, "horizontal": false})).unwrap();
            }
            app.run("object.move", json!({"dx": 10, "dy": 0})).unwrap();
            let moved = app.session.doc().unwrap().doc.clone();
            app.run("edit.undo", json!({})).unwrap();
            app.session.doc_mut().unwrap().mark_saved();
            let mut h = AppFrames { app, ctx: egui::Context::default(), time: 0.0 };
            h.settle();
            let before = model(&h.app);
            assert!(!before.dirty && before.interaction.is_none() && before.history.redo.len() == 1);
            for accept in [false, true] {
                menus::invoke(&mut h.app, "object.envelope.makeWithWarp", json!({}));
                h.settle();
                let d = h.app.ui.dialog.as_ref().unwrap();
                assert_eq!(d.kind, "envelopeWarp");
                assert_eq!(d.bool("reset"), reset);
                let mut expected_fields = d.fields.clone();
                choose_style(&mut h, keyboard);
                expected_fields.insert("style".into(), json!("arcLower"));
                for key in ["style", "reset", "horizontal", "bend", "h", "v", "preview"] {
                    assert_eq!(h.app.ui.dialog.as_ref().unwrap().fields[key], expected_fields[key]);
                }
                assert_envelope(&h.app, &original, reset);
                assert_eq!(model(&h.app).history, before.history, "opening and choosing Style does not commit");
                if reset {
                    assert_eq!(h.app.session.doc().unwrap().selection, before.selection, "Reset retains its target");
                }
                h.click_text("Preview");
                assert!(!h.app.ui.dialog.as_ref().unwrap().bool("preview"));
                assert_eq!(model(&h.app), before, "actual Preview off restores the baseline");
                h.click_text("Preview");
                h.settle();
                assert_envelope(&h.app, &original, reset);
                h.click_text(if accept { "OK" } else { "Cancel" });
                assert!(h.app.ui.dialog.is_none() && !h.app.session.in_interaction());
                if accept {
                    assert_envelope(&h.app, &original, reset);
                    let st = h.app.session.doc().unwrap();
                    let accepted = st.doc.clone();
                    let accepted_selection = st.selection.clone();
                    assert_eq!(st.history.undo.len(), before.history.undo.len() + 1, "one Undo for the accepted style");
                    assert!(st.history.redo.is_empty());
                    h.app.run("edit.undo", json!({})).unwrap();
                    assert_eq!(h.app.session.doc().unwrap().doc, before.doc);
                    assert_eq!(h.app.session.doc().unwrap().selection, before.selection);
                    h.app.run("edit.redo", json!({})).unwrap();
                    assert_eq!(h.app.session.doc().unwrap().doc, accepted);
                    assert_eq!(h.app.session.doc().unwrap().selection, accepted_selection);
                } else {
                    assert_eq!(model(&h.app), before, "Cancel restores document, selection, history and saved state");
                    h.app.run("edit.redo", json!({})).unwrap();
                    assert_eq!(h.app.session.doc().unwrap().doc, moved, "Cancel retains the original usable Redo");
                    h.app.run("edit.undo", json!({})).unwrap();
                    assert_eq!(model(&h.app), before);
                }
            }
        }
    }
}
