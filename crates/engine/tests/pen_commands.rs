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
    pen_preview_in(s)
}

fn pen_preview_in(mut s: Session) -> (Session, NodeId, PathData, usize) {
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

fn small_pen_document() -> Session {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width":800,"height":600,"created":1_000_000_000})).unwrap();
    s
}

fn assert_replay_document(s: &Session) {
    let mut replay = Session::new();
    for (command, params) in &s.journal {
        replay.execute(command, params).unwrap();
    }
    assert_eq!(serde_json::to_value(&*replay.doc().unwrap().doc).unwrap(), serde_json::to_value(&*s.doc().unwrap().doc).unwrap());
}

fn external_clear_during_pen(explicit_other_object: bool) {
    let mut s = small_pen_document();
    let other = NodeId(s.execute("shape.rectangle", &json!({"x":10,"y":10,"width":10,"height":10})).unwrap()["id"].as_u64().unwrap());
    let (mut s, id, first_anchor, steps) = pen_preview_in(s);
    let preview = s.doc().unwrap().clone();
    assert!(preview.selection.anchors.get(&id).is_some_and(|anchors| anchors.contains(&(0, 1))));
    // Default Clear delegates to anchor deletion. With two anchors, removing the selected
    // terminal anchor removes the undersized path; explicit ids take the object-removal branch.
    let params = if explicit_other_object { json!({"ids":[other.0]}) } else { json!({}) };
    let deleted = if explicit_other_object { other } else { id };
    s.execute("edit.clear", &params).unwrap();
    assert!(s.doc().unwrap().doc.node(deleted).is_none(), "Clear removes the requested object immediately");
    let edited = s.doc().unwrap().doc.clone();
    // A delayed event from the gesture must not resurrect either the Pen path or other art.
    pointer(&mut s, PointerKind::Drag, 250.0, 350.0);
    pointer(&mut s, PointerKind::Up, 250.0, 350.0);
    assert!(s.doc().unwrap().doc.node(deleted).is_none(), "the next Pen preview must not undo external Clear");
    assert_eq!(serde_json::to_value(&*s.doc().unwrap().doc).unwrap(), serde_json::to_value(&*edited).unwrap());
    assert!(!s.in_interaction() && !s.tool_busy());
    assert!(s.doc().unwrap().selection.is_empty());
    assert_eq!(s.doc().unwrap().history.undo.len(), steps + 2, "Pen and Clear are separate undo steps");
    assert_eq!(s.doc().unwrap().history.undo.last().unwrap().label, "Clear");
    let n = s.journal.len();
    assert_eq!(s.journal[n - 2].0, "path.appendAnchor");
    assert_eq!(s.journal[n - 1], ("edit.clear".into(), params));
    assert_replay_document(&s);
    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(serde_json::to_value(&*s.doc().unwrap().doc).unwrap(), serde_json::to_value(&*preview.doc).unwrap());
    assert_eq!(s.doc().unwrap().selection, preview.selection);
    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(path(&s, id), first_anchor);
    s.execute("edit.redo", &json!({})).unwrap();
    assert_eq!(path(&s, id), preview.doc.node(id).unwrap().path_data().unwrap().clone());
    s.execute("edit.redo", &json!({})).unwrap();
    assert_eq!(serde_json::to_value(&*s.doc().unwrap().doc).unwrap(), serde_json::to_value(&*edited).unwrap());
}

#[test]
fn pen_external_clear_selected_path_survives_late_events_and_replays() {
    external_clear_during_pen(false);
}

#[test]
fn pen_external_clear_explicit_object_survives_late_events_and_replays() {
    external_clear_during_pen(true);
}

#[test]
fn pen_external_artboard_delete_survives_late_events_and_replays() {
    let mut s = small_pen_document();
    s.execute("artboard.new", &json!({"x":900,"y":0,"width":100,"height":80,"name":"Second"})).unwrap();
    s.execute("guide.add", &json!({"vertical":true,"pos":925,"artboard":1})).unwrap();
    let (mut s, id, first_anchor, steps) = pen_preview_in(s);
    let preview = s.doc().unwrap().clone();
    let visible_path = path(&s, id);
    assert_eq!(preview.doc.artboards.len(), 2);
    assert_eq!(preview.doc.guides.len(), 1);
    s.execute("artboard.delete", &json!({"index":1})).unwrap();
    assert_eq!(s.doc().unwrap().doc.artboards, vec![preview.doc.artboards[0].clone()]);
    assert!(s.doc().unwrap().doc.guides.is_empty());
    let edited = s.doc().unwrap().doc.clone();
    pointer(&mut s, PointerKind::Drag, 250.0, 350.0);
    pointer(&mut s, PointerKind::Up, 250.0, 350.0);
    assert_eq!(s.doc().unwrap().doc.artboards, edited.artboards, "a Pen preview must not restore a deleted artboard");
    assert_eq!(s.doc().unwrap().doc.guides, edited.guides);
    assert_eq!(path(&s, id), visible_path);
    assert_eq!(s.doc().unwrap().selection, preview.selection);
    assert!(!s.in_interaction() && !s.tool_busy());
    assert_eq!(s.doc().unwrap().history.undo.len(), steps + 2);
    assert_eq!(s.doc().unwrap().history.undo.last().unwrap().label, "Delete Artboard");
    let n = s.journal.len();
    assert_eq!(s.journal[n - 2].0, "path.appendAnchor");
    assert_eq!(s.journal[n - 1], ("artboard.delete".into(), json!({"index":1})));
    assert_replay_document(&s);
    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(serde_json::to_value(&*s.doc().unwrap().doc).unwrap(), serde_json::to_value(&*preview.doc).unwrap());
    assert_eq!(s.doc().unwrap().selection, preview.selection);
    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(path(&s, id), first_anchor);
    s.execute("edit.redo", &json!({})).unwrap();
    s.execute("edit.redo", &json!({})).unwrap();
    assert_eq!(serde_json::to_value(&*s.doc().unwrap().doc).unwrap(), serde_json::to_value(&*edited).unwrap());
}

fn assert_pen_history_unchanged(actual: &vectorcraft_engine::History, expected: &vectorcraft_engine::History) {
    assert_eq!(actual.limit, expected.limit);
    for (actual, expected) in [(&actual.undo, &expected.undo), (&actual.redo, &expected.redo)] {
        assert_eq!(actual.len(), expected.len());
        for (actual, expected) in actual.iter().zip(expected) {
            assert_eq!(actual.label, expected.label);
            assert!(std::sync::Arc::ptr_eq(&actual.doc, &expected.doc));
            assert_eq!(actual.selection, expected.selection);
        }
    }
}

fn rejected_pen_command(s: &mut Session, id: NodeId, command: &str, params: serde_json::Value, message: &str) {
    let before = s.doc().unwrap().clone();
    let interaction = before.interaction.as_ref().unwrap();
    let first_anchor = interaction.doc.node(id).unwrap().path_data().unwrap().clone();
    let preview = path(s, id);
    let journal = s.journal.clone();
    let prefs = s.prefs.clone();
    let paint = s.paint.clone();
    let options = s.tool_options();
    let tool = s.tool_id();
    let active = s.active_index();
    let out = s.execute(command, &params);
    assert!(matches!(out, Err(vectorcraft_engine::EngineError::Other(ref value)) if value == message));
    let st = s.doc().unwrap();
    assert_eq!(serde_json::to_value(&*st.doc).unwrap(), serde_json::to_value(&*before.doc).unwrap());
    assert!(std::sync::Arc::ptr_eq(&st.doc, &before.doc));
    assert_eq!(st.selection, before.selection);
    assert_pen_history_unchanged(&st.history, &before.history);
    assert_eq!(st.revision, before.revision);
    assert_eq!(st.is_dirty(), before.is_dirty());
    assert_eq!(st.active_layer, before.active_layer);
    assert_eq!(st.layer_rows, before.layer_rows);
    assert_eq!(st.isolation, before.isolation);
    assert_eq!(st.last_transform, before.last_transform);
    assert_eq!(st.last_perspective, before.last_perspective);
    let current = st.interaction.as_ref().expect("rejection keeps the live gesture");
    assert!(std::sync::Arc::ptr_eq(&current.doc, &interaction.doc));
    assert_eq!(current.label, interaction.label);
    assert_eq!(current.preview, interaction.preview);
    assert_eq!(current.selection, interaction.selection);
    assert_eq!(current.active_layer, interaction.active_layer);
    assert_eq!(current.layer_rows, interaction.layer_rows);
    assert_eq!(current.isolation, interaction.isolation);
    assert_eq!(current.perspective_again, interaction.perspective_again);
    assert_eq!(s.journal, journal);
    assert_eq!(s.prefs, prefs);
    assert_eq!(s.paint, paint);
    assert_eq!(s.tool_options(), options);
    assert_eq!(s.tool_id(), tool);
    assert_eq!(s.active_index(), active);
    assert!(s.in_interaction() && s.tool_busy());
    pointer(s, PointerKind::Drag, 250.0, 350.0);
    pointer(s, PointerKind::Up, 250.0, 350.0);
    assert_ne!(path(s, id), preview, "the rejected command keeps the handle editable");
    assert_eq!(path(s, id).anchor_count(), 2);
    assert_eq!(s.doc().unwrap().doc.artboards, before.doc.artboards);
    assert_eq!(s.doc().unwrap().doc.guides, before.doc.guides);
    assert_eq!(s.doc().unwrap().history.undo.len(), before.history.undo.len() + 1);
    assert_eq!(&s.journal[..journal.len()], &journal);
    assert_eq!(s.journal.last().unwrap().0, "path.appendAnchor");
    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(path(s, id), first_anchor);
}

#[test]
fn pen_rejected_new_artboard_preserves_live_preview_history_and_tool() {
    let (mut s, id, _, _) = pen_preview(Some(1_000_000_000));
    // A standard 100x80 artboard crosses the existing coordinate policy by 50 points.
    // The coordinate never determines an allocation or a workload.
    rejected_pen_command(
        &mut s,
        id,
        "artboard.new",
        json!({"x":vectorcraft_engine::MAX_COORD - 50.0,"y":0,"width":100,"height":80,"name":"Rejected"}),
        "result would exceed the canvas (coordinates out of range)",
    );
}

#[test]
fn pen_rejected_artboard_delete_preserves_live_preview_history_and_tool() {
    for sole_board in [true, false] {
        let mut s = small_pen_document();
        if !sole_board {
            s.execute("artboard.new", &json!({"x":900,"y":0,"width":100,"height":80})).unwrap();
        }
        let (mut s, id, _, _) = pen_preview_in(s);
        let (index, message) = if sole_board { (0, "a document needs at least one artboard") } else { (2, "no such artboard") };
        rejected_pen_command(&mut s, id, "artboard.delete", json!({"index":index}), message);
    }
}
