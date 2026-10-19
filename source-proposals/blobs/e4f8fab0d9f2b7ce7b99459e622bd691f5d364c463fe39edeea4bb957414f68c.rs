//! Original Layers-only visual observation. No private measurement counters or product UI changes.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use egui::{Color32, Rect, Shape, vec2};
use serde_json::{Value, json};
use std::path::Path;
use vectorcraft_doc::NodeId;
use vectorcraft_engine::{HistoryEntry, Session};
use vectorcraft_ui_egui::{VectorcraftApp, panels, theme};

fn state(app: &VectorcraftApp) -> Value {
    let st = app.session.doc().unwrap();
    let entries = |items: &[HistoryEntry]| {
        items.iter().map(|e| json!({"label": e.label, "document": e.doc.as_ref(), "selection": e.selection.as_ref()})).collect::<Vec<_>>()
    };
    json!({
        "document": st.doc.as_ref(), "selection": st.selection, "dirty": st.is_dirty(),
        "undo": entries(&st.history.undo), "redo": entries(&st.history.redo),
        "interaction": st.interaction.is_some(), "undo_group": st.undo_group.is_some(), "journal": app.session.journal
    })
}

struct Frames {
    harness: egui_kittest::Harness<'static, VectorcraftApp>,
    time: f64,
}
impl Frames {
    fn new(app: VectorcraftApp) -> Self {
        let mut harness = egui_kittest::Harness::builder()
            .with_size(vec2(420.0, 500.0)).with_pixels_per_point(1.0).with_step_dt(0.05).wgpu()
            .build_ui_state(|ui, app: &mut VectorcraftApp| { app.logic(ui.ctx()); panels::layers::show(app, ui); }, app);
        harness.input_mut().max_texture_side = Some(8192);
        Self { harness, time: 0.0 }
    }
    fn settle(&mut self) {
        for _ in 0..6 {
            self.time += 0.05;
            self.harness.input_mut().time = Some(self.time);
            self.harness.input_mut().events.clear();
            self.harness.run_steps(1);
        }
    }
    fn observe(&mut self, directory: &Path, file: &str, selected: bool) -> Result<Value, Box<dyn std::error::Error>> {
        self.settle();
        let before = state(self.harness.state());
        let mut large = vec![];
        let mut small = vec![];
        let mut rows = vec![];
        let labels = ["Observation layer", "Rectangle group", "Chosen rectangle", "Sibling rectangle", "Separate rectangle"];
        let mut painted_labels = vec![];
        // Same real painted primitives used by the existing Layers selection-square tests.
        for clipped in &self.harness.output().shapes {
            match &clipped.shape {
                Shape::Rect(r) if r.fill != Color32::TRANSPARENT && clipped.clip_rect.contains(r.rect.center()) => {
                    if r.rect.size() == vec2(7.0, 7.0) { large.push(r.rect); }
                    if r.rect.size() == vec2(4.0, 4.0) { small.push(r.rect); }
                }
                Shape::Circle(c) if c.radius == 5.0 && clipped.clip_rect.contains(c.center) => rows.push(c.center),
                Shape::Text(t) if labels.contains(&t.galley.text()) && clipped.clip_rect.contains(t.visual_bounding_rect().center()) => {
                    painted_labels.push(t.galley.text().to_owned());
                }
                _ => {}
            }
        }
        assert_eq!(rows.len(), 5, "the real expanded layer/group/three path rows are visible");
        for label in labels { assert_eq!(painted_labels.iter().filter(|s| s.as_str() == label).count(), 1, "visible row: {label}"); }
        assert_eq!((large.len(), small.len()), if selected { (1, 2) } else { (0, 0) });
        let image = self.harness.render().map_err(std::io::Error::other)?;
        assert_eq!(image.dimensions(), (420, 500));
        image.save(directory.join(file))?;
        assert_eq!(state(self.harness.state()), before, "rendering cannot change document, selection, history, saved state or journal");
        let rect = |r: &Rect| json!([r.left(), r.top(), r.right(), r.bottom()]);
        Ok(json!({"file": file, "dimensions": [420,500], "selected": selected,
            "large_selection_squares": large.iter().map(rect).collect::<Vec<_>>(),
            "small_descendant_squares": small.iter().map(rect).collect::<Vec<_>>(),
            "visible_labels": painted_labels, "state": before}))
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source_text = include_str!("../source.json");
    assert!(!source_text.contains("UNBOUND"), "parent must bind actual validation, CI, bootstrap and lock receipts");
    let source: Value = serde_json::from_str(source_text)?;
    assert_eq!(source["status"], "BOUND_TO_CURRENT_SOURCE_WITH_ACTUAL_RECEIPTS", "preparation has no actual receipt binding");
    let hash = |v: &Value, n| v.as_str().is_some_and(|s| s.len() == n && s.bytes().all(|b| b.is_ascii_hexdigit()));
    assert!(hash(&source["candidate_tree"], 40));
    assert!(hash(&source["source_sha256"], 64));
    assert!(hash(&source["resolved_capture_lock_sha256"], 64));
    assert_eq!(source["base"], "3f33e854d4d41431bb9db6bef70be4ca2b6b70e4");
    assert_eq!(source["candidate_tree"], "1aa27b1f39a8d17c739a4e1df9fd7c674229017e");
    assert_eq!(source["source_sha256"], "51d2615abe5058028b4ca5b4461d0b5de0e9d1ba005a420058038b1a754fd611");
    for receipt in ["focused_result", "full_ci_result", "bootstrap_result", "lock_audit"] {
        assert!(hash(&source["actual_prerequisites"][receipt]["sha256"], 64), "receipt hash: {receipt}");
        assert!(source["actual_prerequisites"][receipt]["path"].as_str().is_some_and(|p| !p.is_empty()), "receipt path: {receipt}");
    }
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("screens");
    std::fs::create_dir(&directory)?;

    let mut app = VectorcraftApp::new(Session::new(), Default::default());
    app.session.prefs.interface_language = "en".into();
    app.ui.brightness = theme::Brightness::MediumDark;
    app.run("file.new", json!({"width": 240, "height": 180}))?;
    let layer = app.session.doc()?.doc.layers[0].id;
    app.run("layer.setProps", json!({"ids": [layer.0], "name": "Observation layer"}))?;
    let separate = app.run("shape.rectangle", json!({"x": 150, "y": 100, "width": 45, "height": 30}))?["id"].as_u64().unwrap();
    app.run("object.setProps", json!({"name": "Separate rectangle"}))?;
    let chosen = app.run("shape.rectangle", json!({"x": 20, "y": 20, "width": 40, "height": 30}))?["id"].as_u64().unwrap();
    app.run("object.setProps", json!({"name": "Chosen rectangle"}))?;
    let sibling = app.run("shape.rectangle", json!({"x": 80, "y": 30, "width": 35, "height": 50}))?["id"].as_u64().unwrap();
    app.run("object.setProps", json!({"name": "Sibling rectangle"}))?;
    app.run("select.set", json!({"ids": [chosen, sibling]}))?;
    let group = app.run("object.group", json!({}))?["id"].as_u64().unwrap();
    app.run("object.setProps", json!({"name": "Rectangle group"}))?;
    app.run("select.none", json!({}))?;
    app.run("ui.layersExpand", json!({"open": true}))?;
    // Existing panel options only: stable row/icon/selection appearance, not thumbnail rendering.
    app.ui.layers_panel.thumb_layers = false;
    app.ui.layers_panel.thumb_groups = false;
    app.ui.layers_panel.thumb_objects = false;
    app.select_tool("selection");
    app.session.doc_mut()?.mark_saved();
    let document = app.session.doc()?.doc.clone();
    assert_eq!(document.node_count(), 5);
    let mut f = Frames::new(app);
    let empty = f.observe(&directory, "01-empty-selection.png", false)?;
    assert!(f.harness.state().session.doc()?.selection.objects.is_empty());
    f.harness.state_mut().run("select.set", json!({"ids": [chosen]}))?;
    let selected = f.observe(&directory, "02-selected-child.png", true)?;
    assert_eq!(f.harness.state().session.doc()?.selection.objects, vec![NodeId(chosen)]);
    assert_eq!(f.harness.state().session.doc()?.doc, document);
    assert!(!f.harness.state().session.doc()?.is_dirty());
    assert_eq!(empty["state"]["undo"], selected["state"]["undo"]);
    assert_eq!(empty["state"]["redo"], selected["state"]["redo"]);
    let result = json!({"status": "ORIGINAL_OUTPUTS_WRITTEN_PERSONAL_REVIEW_PENDING", "source": source,
        "scope": "Real Layers panel only; selection changes use public commands, not pointer events. No timings or baseline pixel equivalence claim.",
        "ids": {"layer": layer.0, "group": group, "chosen": chosen, "sibling": sibling, "separate": separate},
        "images": [empty, selected], "personal_original_image_review": null});
    std::fs::write(directory.join("model.json"), serde_json::to_vec_pretty(&result)?)?;
    println!("LAYERS_EMPTY_SELECTION_ORIGINALS_WRITTEN_REVIEW_PENDING");
    Ok(())
}
