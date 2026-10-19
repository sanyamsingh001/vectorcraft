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

#[test]
fn pen_external_batch_new_artboard_keeps_preview_in_replay() {
    let (mut s, id, first_anchor, steps) = pen_preview(Some(1_000_000_000));
    let preview = s.doc().unwrap().clone();
    let visible_path = path(&s, id);
    let first_document = preview.interaction.as_ref().unwrap().doc.clone();
    assert_eq!(first_anchor.anchor_count(), 1);
    assert_eq!(visible_path.anchor_count(), 2);
    let batch = json!({
        "label":"After Pen Batch",
        "commands":[{"command":"artboard.new","params":{"x":900,"y":0,"width":100,"height":80,"name":"Batch Board"}}]
    });

    // This is an external Session command during the second-anchor preview, not tool actions.
    let out = s.execute("command.batch", &batch).unwrap();
    assert_eq!(out["results"].as_array().unwrap().len(), 1);
    assert_eq!(out["results"][0]["index"], 1);
    assert_eq!(s.doc().unwrap().doc.artboards.len(), 2);
    assert_eq!(s.doc().unwrap().doc.artboards[0], preview.doc.artboards[0]);
    assert_eq!(s.doc().unwrap().doc.artboards[1].rect, vectorcraft_geom::Rect::new(900.0, 0.0, 1000.0, 80.0));
    assert_eq!(s.doc().unwrap().doc.artboards[1].name, "Batch Board");
    assert_eq!(path(&s, id), visible_path);
    assert_eq!(s.doc().unwrap().selection, preview.selection);

    // Replay immediately, before a late event can change or accidentally journal the preview.
    assert_replay_document(&s);
    assert!(!s.in_interaction() && !s.tool_busy(), "an accepted external batch must finish the preceding Pen gesture");
    assert_eq!(s.tool_id(), "pen");
    assert_eq!(s.doc().unwrap().history.undo.len(), steps + 2, "Pen and the batch have separate undo boundaries");
    assert_eq!(s.doc().unwrap().history.undo.last().unwrap().label, "After Pen Batch");
    assert_eq!(s.journal.last().unwrap().0, "command.batch");
    assert_eq!(s.journal.last().unwrap().1["commands"].as_array().unwrap().len(), 1);

    let edited = s.doc().unwrap().clone();
    let edited_json = serde_json::to_value(&*edited.doc).unwrap();
    let journal = s.journal.clone();
    let options = s.tool_options();
    pointer(&mut s, PointerKind::Drag, 250.0, 350.0);
    pointer(&mut s, PointerKind::Up, 250.0, 350.0);
    assert_eq!(serde_json::to_value(&*s.doc().unwrap().doc).unwrap(), edited_json);
    assert_eq!(s.doc().unwrap().selection, edited.selection);
    assert_pen_history_unchanged(&s.doc().unwrap().history, &edited.history);
    assert_eq!(s.doc().unwrap().revision, edited.revision);
    assert_eq!(s.doc().unwrap().is_dirty(), edited.is_dirty());
    assert_eq!(s.journal, journal);
    assert_eq!(s.tool_options(), options);
    assert!(!s.in_interaction() && !s.tool_busy());
    assert_replay_document(&s);

    // Undo removes the batch's board first, leaving the already finished Pen preview in place.
    let out = s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(out["undone"], "After Pen Batch");
    assert_eq!(serde_json::to_value(&*s.doc().unwrap().doc).unwrap(), serde_json::to_value(&*preview.doc).unwrap());
    assert_eq!(s.doc().unwrap().selection, preview.selection);
    assert_eq!(path(&s, id), visible_path);
    assert_eq!(s.doc().unwrap().history.undo.len(), steps + 1);
    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(path(&s, id), first_anchor);
    assert_eq!(serde_json::to_value(&*s.doc().unwrap().doc).unwrap(), serde_json::to_value(&*first_document).unwrap());
    assert_eq!(s.doc().unwrap().history.undo.len(), steps);
    s.execute("edit.redo", &json!({})).unwrap();
    assert_eq!(path(&s, id), visible_path);
    assert_eq!(s.doc().unwrap().doc.artboards.len(), 1);
    s.execute("edit.redo", &json!({})).unwrap();
    assert_eq!(serde_json::to_value(&*s.doc().unwrap().doc).unwrap(), edited_json);
    assert_eq!(s.doc().unwrap().history.undo.len(), steps + 2);
    assert!(s.doc().unwrap().history.redo.is_empty());
    assert_replay_document(&s);
}

#[test]
fn pen_rejected_external_batch_new_artboard_preserves_live_preview_history_and_tool() {
    let (mut s, id, _, _) = pen_preview(Some(1_000_000_000));
    // A standard 100x80 board crosses the coordinate limit by 50 points; no large allocation.
    rejected_pen_command(
        &mut s,
        id,
        "command.batch",
        json!({
            "label":"Rejected Artboard Batch",
            "commands":[{"command":"artboard.new","params":{
                "x":vectorcraft_engine::MAX_COORD - 50.0,"y":0,"width":100,"height":80,"name":"Rejected"
            }}]
        }),
        "batch step 0 (`artboard.new`) failed: result would exceed the canvas (coordinates out of range)",
    );
    assert!(!s.in_interaction() && !s.tool_busy());
    assert_replay_document(&s);
}

fn pen_batch_two_documents(saved_path: Option<&std::path::Path>) -> (Session, NodeId, PathData, usize) {
    let mut s = small_pen_document();
    s.execute("shape.rectangle", &json!({"x":10,"y":10,"width":10,"height":10})).unwrap();
    s.execute("file.new", &json!({"name":"Live Pen","width":800,"height":600,"created":1_000_000_000})).unwrap();
    if let Some(path) = saved_path {
        s.execute("document.save", &json!({"path":path,"modified":null,"preview":false})).unwrap();
    }
    pen_preview_in(s)
}

fn pen_batch_documents(s: &Session) -> Vec<serde_json::Value> {
    s.documents().iter().map(|st| serde_json::to_value(&*st.doc).unwrap()).collect()
}

fn assert_pen_batch_replays_both_documents(s: &Session) {
    let mut replay = Session::new();
    for (command, params) in &s.journal {
        replay.execute(command, params).unwrap();
    }
    assert_eq!(pen_batch_documents(&replay), pen_batch_documents(s));
    assert_eq!(replay.active_index(), s.active_index());
    assert!(replay.documents().iter().all(|st| st.interaction.is_none()));
}

fn rejected_pen_batch_document_reset(reset: &str, saved_path: Option<&std::path::Path>) {
    let (mut s, id, first_anchor, steps) = pen_batch_two_documents(saved_path);
    // The control receives the same pointer events and no batch. Matching its live handle,
    // including before Up, exercises the tool's private drag state through the public API.
    let (mut control, control_id, _, _) = pen_batch_two_documents(saved_path);
    assert_eq!(pen_batch_documents(&s), pen_batch_documents(&control));
    assert_eq!(s.active_index(), Some(1));
    let before = s.documents().to_vec();
    let preview = path(&s, id);
    let journal = s.journal.clone();
    let paint = s.paint.clone();
    let prefs = s.prefs.clone();
    let options = s.tool_options();
    assert_eq!(first_anchor.anchor_count(), 1);
    assert_eq!(preview.anchor_count(), 2);
    // Exercise the no-batch control before the rejection, even on the negative run.
    pointer(&mut control, PointerKind::Drag, 250.0, 350.0);
    assert_ne!(path(&control, control_id), preview);
    assert_eq!(path(&control, control_id).anchor_count(), 2);
    assert!(control.in_interaction() && control.tool_busy());
    let batch = json!({"label":"Rejected document reset","commands":[
        {"command":reset,"params":{}},
        {"command":"shape.rectangle","params":{"x":40,"y":10,"width":10,"height":10}},
        {"command":"artboard.setProps","params":{"index":1,"name":"Missing page"}}
    ]});
    let error = s.execute("command.batch", &batch).unwrap_err();
    assert_eq!(error.to_string(), "batch step 2 (`artboard.setProps`) failed: no such artboard");

    // Close temporarily edits A; Revert replaces B at the same uid. Both must restore the
    // original tab order, saved state, history and still-open second-anchor interaction.
    assert_eq!(s.documents().len(), 2);
    assert_eq!(s.active_index(), Some(1));
    assert_eq!(s.doc().unwrap().uid, before[1].uid);
    for (st, old) in s.documents().iter().zip(&before) {
        assert_eq!(st.uid, old.uid);
        assert!(std::sync::Arc::ptr_eq(&st.doc, &old.doc));
        assert_eq!(serde_json::to_value(&*st.doc).unwrap(), serde_json::to_value(&*old.doc).unwrap());
        assert_eq!(st.selection, old.selection);
        assert_pen_history_unchanged(&st.history, &old.history);
        assert_eq!(st.path, old.path);
        assert_eq!(st.format, old.format);
        assert_eq!(st.save_options, old.save_options);
        assert_eq!(st.is_dirty(), old.is_dirty());
        assert!(st.revision >= old.revision, "rollback must not move the redraw revision backwards");
        assert_eq!(st.active_layer, old.active_layer);
        assert_eq!(st.layer_rows, old.layer_rows);
        assert_eq!(st.isolation, old.isolation);
        match (&st.interaction, &old.interaction) {
            (Some(current), Some(original)) => {
                assert!(std::sync::Arc::ptr_eq(&current.doc, &original.doc));
                assert_eq!(current.label, original.label);
                assert_eq!(current.preview, original.preview);
                assert_eq!(current.selection, original.selection);
                assert_eq!(current.active_layer, original.active_layer);
                assert_eq!(current.layer_rows, original.layer_rows);
                assert_eq!(current.isolation, original.isolation);
                assert_eq!(current.perspective_again, original.perspective_again);
            }
            (None, None) => {}
            _ => panic!("rejection must restore each document's original interaction"),
        }
    }
    if reset == "file.revert" {
        assert!(s.doc().unwrap().revision > before[1].revision, "reverted content must redraw when restored");
    }
    assert_eq!(s.journal, journal);
    assert_eq!(s.paint, paint);
    assert_eq!(s.prefs, prefs);
    assert_eq!(s.tool_id(), "pen");
    assert_eq!(s.tool_options(), options);
    assert!(s.in_interaction() && s.tool_busy(), "rejection must return the original live Pen tool");

    pointer(&mut s, PointerKind::Drag, 250.0, 350.0);
    assert_ne!(path(&s, id), preview, "the restored private handle must respond before Up");
    assert_eq!(path(&s, id), path(&control, control_id));
    assert_eq!(path(&s, id).anchor_count(), 2, "the drag reshapes the same second anchor");
    assert_eq!(pen_batch_documents(&s), pen_batch_documents(&control));
    assert_eq!(s.doc().unwrap().selection, control.doc().unwrap().selection);
    assert_eq!(s.doc().unwrap().history.undo.len(), steps, "Drag has not committed the resumed gesture");
    assert_eq!(s.journal, journal, "the resumed preview is journaled only on Up");
    assert!(s.in_interaction() && s.tool_busy());

    pointer(&mut s, PointerKind::Up, 250.0, 350.0);
    pointer(&mut control, PointerKind::Up, 250.0, 350.0);
    let finished = path(&s, id);
    assert_ne!(finished, preview);
    assert_eq!(finished, path(&control, control_id));
    assert_eq!(pen_batch_documents(&s), pen_batch_documents(&control));
    assert_eq!(s.doc().unwrap().selection, control.doc().unwrap().selection);
    assert!(!s.in_interaction() && !s.tool_busy());
    assert_eq!(s.doc().unwrap().history.undo.len(), steps + 1, "only the resumed Pen gesture is added");
    assert_eq!(s.journal.len(), journal.len() + 1);
    assert_eq!(&s.journal[..journal.len()], &journal);
    assert_eq!(s.journal.last().unwrap().0, "path.appendAnchor");
    assert_pen_history_unchanged(&s.documents()[0].history, &before[0].history);
    assert_pen_batch_replays_both_documents(&s);

    s.execute("edit.undo", &json!({})).unwrap();
    control.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(path(&s, id), first_anchor);
    assert_eq!(pen_batch_documents(&s), pen_batch_documents(&control));
    assert_eq!(s.doc().unwrap().history.undo.len(), steps);
    assert_pen_batch_replays_both_documents(&s);
    s.execute("edit.redo", &json!({})).unwrap();
    control.execute("edit.redo", &json!({})).unwrap();
    assert_eq!(path(&s, id), finished);
    assert_eq!(pen_batch_documents(&s), pen_batch_documents(&control));
    assert_eq!(s.doc().unwrap().history.undo.len(), steps + 1);
    assert!(s.doc().unwrap().history.redo.is_empty());
    assert_eq!(s.active_index(), Some(1));
    assert_pen_history_unchanged(&s.documents()[0].history, &before[0].history);
    assert_pen_batch_replays_both_documents(&s);
}

#[test]
fn pen_rejected_batch_close_restores_live_handle_and_both_documents() {
    rejected_pen_batch_document_reset("file.close", None);
}

#[test]
fn pen_rejected_batch_revert_restores_live_handle_and_both_documents() {
    let path = vectorcraft_testkit::temp_dir("pen-batch-revert").join("saved.vectorcraft");
    rejected_pen_batch_document_reset("file.revert", Some(&path));
}

fn assert_pen_batch_tabs_replay(s: &Session) {
    let mut replay = Session::new();
    for (command, params) in &s.journal {
        replay.execute(command, params).unwrap();
    }
    let documents = |s: &Session| s.documents().iter().map(|st| serde_json::to_value(&*st.doc).unwrap()).collect::<Vec<_>>();
    assert_eq!(documents(&replay), documents(s));
    assert_eq!(replay.active_index(), s.active_index());
}

#[test]
fn pen_external_batch_can_close_the_final_document() {
    let (mut s, _, _, _) = pen_preview(Some(1_000_000_000));
    let pen_entry = s.doc().unwrap().interaction.as_ref().unwrap().preview.clone().unwrap();
    let journal_before = s.journal.clone();
    let options = s.tool_options();
    let batch = json!({
        "label": "Pen then close",
        "commands": [
            {"command": "shape.rectangle", "params": {"x": 20, "y": 20, "width": 30, "height": 20}},
            {"command": "file.close", "params": {}}
        ]
    });

    let out = s.execute("command.batch", &batch).unwrap();
    assert_eq!(out["results"].as_array().unwrap().len(), 2);
    assert!(out["results"][0]["id"].as_u64().is_some());
    assert!(s.documents().is_empty());
    assert_eq!(s.active_index(), None);
    assert!(!s.in_interaction() && !s.tool_busy());
    assert_eq!(s.tool_id(), "pen");
    assert_eq!(s.tool_options(), options);
    assert_eq!(s.journal.len(), journal_before.len() + 2);
    assert_eq!(&s.journal[..journal_before.len()], &journal_before);
    assert_eq!(s.journal[journal_before.len()], pen_entry, "the Pen preview precedes the batch entry");
    assert_eq!(s.journal.last().unwrap().0, "command.batch");
    assert_eq!(s.journal.last().unwrap().1["commands"].as_array().unwrap().len(), 2);
    assert_pen_batch_tabs_replay(&s);

    // A release after the final tab closed must not require an active ToolContext.
    let closed_journal = s.journal.clone();
    pointer(&mut s, PointerKind::Drag, 250.0, 350.0);
    pointer(&mut s, PointerKind::Up, 250.0, 350.0);
    assert!(s.documents().is_empty());
    assert_eq!(s.active_index(), None);
    assert_eq!(s.journal, closed_journal);
    assert!(!s.in_interaction() && !s.tool_busy());
    assert_eq!(s.tool_options(), options);

    // A normal gesture in a subsequently opened tab still starts a fresh path.
    s.execute("file.new", &json!({"width": 200, "height": 150, "created": 2_000_000_000})).unwrap();
    pointer(&mut s, PointerKind::Down, 40.0, 50.0);
    pointer(&mut s, PointerKind::Up, 40.0, 50.0);
    let id = s.doc().unwrap().selection.objects[0];
    assert_eq!(path(&s, id).anchor_count(), 1);
    assert_eq!(s.doc().unwrap().doc.node_count(), 2);
    assert!(!s.in_interaction() && !s.tool_busy());
    assert_pen_batch_tabs_replay(&s);
}

#[test]
fn pen_external_batch_edits_original_then_opens_a_new_tab() {
    let (mut s, id, first_anchor, steps) = pen_preview(Some(1_000_000_000));
    let preview = s.doc().unwrap().clone();
    let visible_path = path(&s, id);
    let pen_entry = preview.interaction.as_ref().unwrap().preview.clone().unwrap();
    let journal_before = s.journal.clone();
    let label = "Pen edits two tabs";
    let batch = json!({
        "label": label,
        "commands": [
            {"command": "shape.rectangle", "params": {"x": 20, "y": 20, "width": 30, "height": 20}},
            {"command": "file.new", "params": {"width": 200, "height": 150, "created": 2_000_000_000}},
            {"command": "shape.ellipse", "params": {"x": 40, "y": 40, "width": 30, "height": 20}}
        ]
    });

    let out = s.execute("command.batch", &batch).unwrap();
    assert_eq!(out["results"].as_array().unwrap().len(), 3);
    let rectangle = NodeId(out["results"][0]["id"].as_u64().unwrap());
    let ellipse = NodeId(out["results"][2]["id"].as_u64().unwrap());
    assert_eq!(s.documents().len(), 2);
    assert_eq!(s.active_index(), Some(1));
    assert_eq!(s.tool_id(), "pen");
    assert!(!s.tool_busy());
    assert!(s.documents().iter().all(|st| st.interaction.is_none()));

    // Inspect the first tab now, before activation or another edit can hide a lost undo boundary.
    let original = &s.documents()[0];
    assert_eq!(original.uid, preview.uid);
    assert_ne!(original.uid, s.documents()[1].uid);
    assert_eq!(original.doc.node(id).unwrap().path_data(), Some(&visible_path));
    assert!(original.doc.node(rectangle).is_some());
    assert_eq!(original.doc.node_count(), preview.doc.node_count() + 1);
    assert_eq!(original.history.undo.len(), steps + 2, "Pen and this tab's batch edit are separate");
    assert_eq!(original.history.undo.last().unwrap().label, label);
    let new_tab = &s.documents()[1];
    assert!(new_tab.doc.node(ellipse).is_some());
    assert_eq!(new_tab.doc.node_count(), 2);
    assert_eq!(new_tab.history.undo.len(), 1);
    assert_eq!(new_tab.history.undo[0].label, label);
    assert_eq!(s.journal.len(), journal_before.len() + 2);
    assert_eq!(&s.journal[..journal_before.len()], &journal_before);
    assert_eq!(s.journal[journal_before.len()], pen_entry);
    assert_eq!(s.journal.last().unwrap().0, "command.batch");
    assert_eq!(s.journal.last().unwrap().1["commands"].as_array().unwrap().len(), 3);
    assert_pen_batch_tabs_replay(&s);

    // The original Pen's late events must change neither tab, including their editing state.
    let accepted = s.documents().to_vec();
    let journal = s.journal.clone();
    let options = s.tool_options();
    pointer(&mut s, PointerKind::Drag, 250.0, 350.0);
    pointer(&mut s, PointerKind::Up, 250.0, 350.0);
    assert_eq!(s.documents().len(), accepted.len());
    assert_eq!(s.active_index(), Some(1));
    for (actual, expected) in s.documents().iter().zip(&accepted) {
        assert!(std::sync::Arc::ptr_eq(&actual.doc, &expected.doc));
        assert_eq!(actual.uid, expected.uid);
        assert_eq!(actual.selection, expected.selection);
        assert_pen_history_unchanged(&actual.history, &expected.history);
        assert_eq!(actual.revision, expected.revision);
        assert_eq!(actual.is_dirty(), expected.is_dirty());
        assert!(actual.interaction.is_none());
    }
    assert_eq!(s.journal, journal);
    assert_eq!(s.tool_options(), options);
    assert!(!s.in_interaction() && !s.tool_busy());
    assert_pen_batch_tabs_replay(&s);

    s.execute("document.activate", &json!({"index": 0})).unwrap();
    let out = s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(out["undone"], label);
    assert_eq!(serde_json::to_value(&*s.doc().unwrap().doc).unwrap(), serde_json::to_value(&*preview.doc).unwrap());
    assert_eq!(s.doc().unwrap().selection, preview.selection);
    assert_eq!(path(&s, id), visible_path);
    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(path(&s, id), first_anchor);
    s.execute("edit.redo", &json!({})).unwrap();
    s.execute("edit.redo", &json!({})).unwrap();
    assert_eq!(serde_json::to_value(&*s.doc().unwrap().doc).unwrap(), serde_json::to_value(&*accepted[0].doc).unwrap());
    assert!(std::sync::Arc::ptr_eq(&s.documents()[1].doc, &accepted[1].doc));
    assert_pen_batch_tabs_replay(&s);
}

#[test]
fn pen_external_empty_batch_keeps_the_live_gesture_usable() {
    let (mut s, id, first_anchor, steps) = pen_preview(Some(1_000_000_000));
    let before = s.doc().unwrap().clone();
    let interaction = before.interaction.as_ref().unwrap();
    let visible_path = path(&s, id);
    let journal_before = s.journal.clone();
    let options = s.tool_options();

    let out = s.execute("command.batch", &json!({"label": "Empty Pen batch", "commands": []})).unwrap();
    assert_eq!(out["results"], json!([]));
    assert_eq!(s.documents().len(), 1);
    assert_eq!(s.active_index(), Some(0));
    assert_eq!(s.tool_id(), "pen");
    assert_eq!(s.tool_options(), options);
    assert!(s.in_interaction() && s.tool_busy(), "an empty batch leaves the current Pen gesture usable");
    let current = s.doc().unwrap();
    assert!(std::sync::Arc::ptr_eq(&current.doc, &before.doc));
    assert_eq!(current.uid, before.uid);
    assert_eq!(current.selection, before.selection);
    assert_pen_history_unchanged(&current.history, &before.history);
    assert_eq!(current.revision, before.revision);
    assert_eq!(current.is_dirty(), before.is_dirty());
    assert_eq!(current.active_layer, before.active_layer);
    assert_eq!(current.layer_rows, before.layer_rows);
    assert_eq!(current.isolation, before.isolation);
    assert_eq!(current.last_transform, before.last_transform);
    assert_eq!(current.last_perspective, before.last_perspective);
    let current = current.interaction.as_ref().unwrap();
    assert!(std::sync::Arc::ptr_eq(&current.doc, &interaction.doc));
    assert_eq!(current.label, interaction.label);
    assert_eq!(current.preview, interaction.preview);
    assert_eq!(current.selection, interaction.selection);
    assert_eq!(current.active_layer, interaction.active_layer);
    assert_eq!(current.layer_rows, interaction.layer_rows);
    assert_eq!(current.isolation, interaction.isolation);
    assert_eq!(current.perspective_again, interaction.perspective_again);

    // Both omitting the no-op entry and recording one empty batch entry are permitted.
    assert!(s.journal.len() == journal_before.len() || s.journal.len() == journal_before.len() + 1);
    assert_eq!(&s.journal[..journal_before.len()], &journal_before);
    if s.journal.len() == journal_before.len() + 1 {
        let entry = s.journal.last().unwrap();
        assert_eq!(entry.0, "command.batch");
        assert_eq!(entry.1["commands"], json!([]));
    }
    let empty_journal = s.journal.clone();
    pointer(&mut s, PointerKind::Drag, 250.0, 350.0);
    pointer(&mut s, PointerKind::Up, 250.0, 350.0);
    assert_ne!(path(&s, id), visible_path, "the same second anchor is still editable");
    assert_eq!(path(&s, id).anchor_count(), 2);
    assert_eq!(s.doc().unwrap().history.undo.len(), steps + 1);
    assert!(!s.in_interaction() && !s.tool_busy());
    assert_eq!(s.journal.len(), empty_journal.len() + 1);
    assert_eq!(&s.journal[..empty_journal.len()], &empty_journal);
    assert_eq!(s.journal.last().unwrap().0, "path.appendAnchor");
    // Replay only after the resumed preview commits; the live preview was not yet journaled.
    assert_replay_document(&s);
    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(path(&s, id), first_anchor);
    s.execute("edit.redo", &json!({})).unwrap();
    assert_replay_document(&s);
}
