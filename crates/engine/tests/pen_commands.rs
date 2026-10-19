//! Commands issued through the engine while a Pen preview is still under way.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
use serde_json::json;
use vectorcraft_doc::NodeId;
use vectorcraft_engine::cmd::fileio::{self, SaveMode};
use vectorcraft_engine::{Session, ViewInfo};
use vectorcraft_geom::PathData;
use vectorcraft_tools::{PointerEvent, PointerKind};

fn pointer(s: &mut Session, kind: PointerKind, x: f64, y: f64) {
    s.pointer(&PointerEvent::new(kind, x, y), ViewInfo::default()).unwrap();
}

fn pen_preview(created: Option<i64>) -> (Session, NodeId, PathData, usize) {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 800, "height": 600, "created": created})).unwrap();
    s.select_tool("pen", ViewInfo::default()).unwrap();
    pointer(&mut s, PointerKind::Down, 100.0, 300.0);
    pointer(&mut s, PointerKind::Up, 100.0, 300.0);
    let id = s.active().unwrap().selection.objects[0];
    let before = path(&s, id);
    let steps = s.active().unwrap().history.undo.len();
    pointer(&mut s, PointerKind::Down, 200.0, 300.0);
    pointer(&mut s, PointerKind::Drag, 230.0, 320.0);
    assert!(s.in_interaction() && s.tool_busy());
    (s, id, before, steps)
}

fn path(s: &Session, id: NodeId) -> PathData {
    s.active().unwrap().doc.node(id).unwrap().path_data().unwrap().clone()
}

fn saved_pen_dates(format: &str) {
    for created in [None, Some(1_000_000_000)] {
        let (mut s, id, before, steps) = pen_preview(created);
        let saved_path = path(&s, id);
        let modified = 2_000_000_000;
        // The real encoder and save completion run; the writer captures every byte in memory.
        let name = format!("pen-dates.{format}");
        let target = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(&name);
        let plan = fileio::save_plan(&s, SaveMode::SaveAs, &json!({"path": target, "modified": modified, "preview": false})).unwrap();
        let mut written = Vec::new();
        fileio::save_with(&mut s, plan, |_, bytes| {
            written.push(bytes.to_vec());
            Ok(())
        })
        .unwrap();
        assert_eq!(written.len(), 1);
        let mut opened = Session::new();
        let result = opened.execute("document.open", &json!({"name": name, "dataBase64": vectorcraft_format::base64_encode(&written[0])})).unwrap();
        if format == "ai" {
            assert_eq!(result["restored"], true, "the native editing attachment must reopen, not just the PDF page");
        }
        let dates = opened.active().unwrap().doc.metadata.clone();
        assert_eq!(dates.created, Some(created.unwrap_or(modified)));
        assert_eq!(dates.modified, Some(modified));
        assert_eq!(path(&opened, id), saved_path);
        assert_eq!(s.active().unwrap().doc.metadata, dates);
        assert!(!s.active().unwrap().is_dirty());
        assert!(s.in_interaction() && s.tool_busy(), "a save keeps the current gesture editable");
        assert_eq!(s.active().unwrap().history.undo.len(), steps, "date stamping is not an undo step");
        pointer(&mut s, PointerKind::Drag, 250.0, 350.0);
        pointer(&mut s, PointerKind::Up, 250.0, 350.0);
        let after = path(&s, id);
        assert_ne!(after, saved_path, "the drag after saving must still shape this anchor");
        assert_eq!(after.anchor_count(), 2);
        assert_eq!(s.active().unwrap().doc.metadata, dates, "the next preview must retain the saved dates");
        assert!(!s.in_interaction() && !s.tool_busy());
        assert!(s.active().unwrap().is_dirty(), "the later handle edit was not in the saved bytes");
        assert_eq!(s.active().unwrap().history.undo.len(), steps + 1);
        s.execute("edit.undo", &json!({})).unwrap();
        assert_eq!(path(&s, id), before);
        assert_eq!(s.active().unwrap().doc.metadata, dates, "Undo reverses the Pen edit, not a save date");
        s.execute("edit.redo", &json!({})).unwrap();
        assert_eq!(path(&s, id), after);
        assert_eq!(s.active().unwrap().doc.metadata, dates);
        assert_eq!(path(&opened, id), saved_path, "the saved editable document is the earlier snapshot");
    }
}

#[test]
fn pen_save_native_dates_survive_next_drag_up_and_undo() {
    saved_pen_dates("vectorcraft");
}

#[test]
fn pen_save_ai_editable_dates_survive_next_drag_up_and_undo() {
    saved_pen_dates("ai");
}

#[test]
fn pen_new_artboard_survives_next_drag_up_with_separate_undo_steps() {
    let (mut s, id, before, steps) = pen_preview(Some(1_000_000_000));
    let finished = path(&s, id);
    let r = s.execute("artboard.new", &json!({"name": "After Pen"})).unwrap();
    assert_eq!(r["index"], 1);
    assert_eq!(s.active().unwrap().doc.artboards.len(), 2);
    pointer(&mut s, PointerKind::Drag, 250.0, 350.0);
    pointer(&mut s, PointerKind::Up, 250.0, 350.0);
    assert_eq!(s.active().unwrap().doc.artboards.len(), 2, "the Pen preview must not discard New Artboard");
    assert_eq!(path(&s, id), finished, "New Artboard finishes the preceding tool gesture");
    assert!(!s.in_interaction() && !s.tool_busy());
    assert_eq!(s.active().unwrap().history.undo.len(), steps + 2, "Pen and New Artboard have separate undo boundaries");
    let n = s.journal.len();
    assert_eq!(s.journal[n - 2].0, "path.appendAnchor");
    assert_eq!(s.journal[n - 1].0, "artboard.new");
    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(s.active().unwrap().doc.artboards.len(), 1);
    assert_eq!(path(&s, id), finished);
    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(path(&s, id), before);
    s.execute("edit.redo", &json!({})).unwrap();
    assert_eq!(path(&s, id), finished);
    s.execute("edit.redo", &json!({})).unwrap();
    assert_eq!(s.active().unwrap().doc.artboards.len(), 2);
    assert_eq!(s.active().unwrap().doc.artboards[1].name, "After Pen");
    assert_eq!(path(&s, id), finished);
}

fn save_bytes(s: &mut Session, format: &str, mode: SaveMode, modified: serde_json::Value) -> Vec<u8> {
    let target = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("pen-controls.{format}"));
    let plan = fileio::save_plan(s, mode, &json!({"path": target, "modified": modified, "preview": false})).unwrap();
    let mut written = Vec::new();
    fileio::save_with(s, plan, |_, bytes| {
        written.push(bytes.to_vec());
        Ok(())
    })
    .unwrap();
    assert_eq!(written.len(), 1);
    written.pop().unwrap()
}

fn editable_saved_doc(format: &str, bytes: &[u8]) -> vectorcraft_doc::Document {
    if format == "ai" {
        let editing = vectorcraft_pdf::editing(bytes).unwrap();
        assert!(editing.intact);
        vectorcraft_format::load(&editing.data).unwrap()
    } else {
        vectorcraft_format::load(bytes).unwrap()
    }
}

#[test]
fn pen_save_copy_and_null_dates_keep_the_preview_and_original_dates() {
    for format in ["vectorcraft", "ai"] {
        for mode in [SaveMode::Copy, SaveMode::SaveAs] {
            let (mut s, id, _, steps) = pen_preview(Some(1_000_000_000));
            let dates = s.active().unwrap().doc.metadata.clone();
            let modified = if mode == SaveMode::Copy { json!(2_000_000_000) } else { serde_json::Value::Null };
            let bytes = save_bytes(&mut s, format, mode, modified);
            assert_eq!(editable_saved_doc(format, &bytes).metadata, dates);
            assert!(s.in_interaction() && s.tool_busy());
            assert_eq!(s.active().unwrap().history.undo.len(), steps);
            pointer(&mut s, PointerKind::Drag, 250.0, 350.0);
            pointer(&mut s, PointerKind::Up, 250.0, 350.0);
            assert_eq!(s.active().unwrap().doc.metadata, dates);
            assert_eq!(path(&s, id).anchor_count(), 2);
            assert_eq!(s.active().unwrap().history.undo.len(), steps + 1);
        }
    }
}

#[test]
fn pen_cancelling_after_a_save_keeps_dates_without_a_history_step() {
    for format in ["vectorcraft", "ai"] {
        let (mut s, id, before, steps) = pen_preview(None);
        let bytes = save_bytes(&mut s, format, SaveMode::SaveAs, json!(2_000_000_000));
        let saved = editable_saved_doc(format, &bytes);
        assert_eq!(saved.node(id).unwrap().path_data().unwrap().anchor_count(), 2);
        s.cancel_interaction().unwrap();
        pointer(&mut s, PointerKind::Up, 230.0, 320.0);
        assert_eq!(path(&s, id), before);
        assert_eq!(s.active().unwrap().doc.metadata, saved.metadata);
        assert_eq!(s.active().unwrap().history.undo.len(), steps);
        assert!(s.active().unwrap().is_dirty());
        assert!(!s.in_interaction() && !s.tool_busy());
    }
}

#[test]
fn pen_new_artboard_between_anchors_keeps_the_path_in_progress() {
    let (mut s, id, _, steps) = pen_preview(None);
    pointer(&mut s, PointerKind::Up, 230.0, 320.0);
    s.execute("artboard.new", &json!({})).unwrap();
    pointer(&mut s, PointerKind::Down, 350.0, 400.0);
    pointer(&mut s, PointerKind::Up, 350.0, 400.0);
    assert_eq!(s.active().unwrap().doc.artboards.len(), 2);
    assert_eq!(path(&s, id).anchor_count(), 3);
    assert_eq!(s.active().unwrap().doc.layers[0].children().unwrap().len(), 1);
    assert_eq!(s.active().unwrap().history.undo.len(), steps + 3);
    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(path(&s, id).anchor_count(), 2);
    assert_eq!(s.active().unwrap().doc.artboards.len(), 2);
    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(s.active().unwrap().doc.artboards.len(), 1);
}
