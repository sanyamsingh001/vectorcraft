//! Private proposed headless regressions. Prepared from source; not compiled or run.
//! The writer-refusal case never writes a file. All art is generated through commands.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
use serde_json::{Value, json};
use vectorcraft_engine::cmd::fileio::{self, SaveMode};
use vectorcraft_engine::{EngineError, Session, ViewInfo};
use vectorcraft_geom::Rect;
use vectorcraft_tools::{Mods, PointerEvent, PointerKind, ToolKey};

fn pointer(s: &mut Session, kind: PointerKind, x: f64, y: f64) {
    s.pointer(&PointerEvent::new(kind, x, y), ViewInfo { smart_guides: false, ..Default::default() }).unwrap();
}
fn document(s: &Session) -> Value {
    serde_json::to_value(&*s.doc().unwrap().doc).unwrap()
}
fn assert_replay(s: &Session) {
    let mut replay = Session::new();
    for (command, params) in &s.journal {
        replay.execute(command, params).unwrap();
    }
    assert_eq!(document(&replay), document(s), "the journal must reproduce the accepted geometry and metadata");
}
fn artboard_resize() -> Session {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width":100,"height":80,"created":1_000_000_000})).unwrap();
    s.execute("shape.rectangle", &json!({"x":10,"y":10,"width":20,"height":10})).unwrap();
    s.select_tool("artboard", ViewInfo::default()).unwrap();
    s.set_tool_option("scaleArt", &json!(true));
    pointer(&mut s, PointerKind::Down, 100.0, 80.0);
    pointer(&mut s, PointerKind::Drag, 150.0, 120.0);
    assert_eq!(s.doc().unwrap().doc.artboards[0].rect, Rect::new(0.0, 0.0, 150.0, 120.0));
    assert!(s.in_interaction() && s.tool_busy());
    s
}
#[test]
fn accepted_batch_during_artboard_resize_has_separate_undo_and_replay() {
    let mut s = artboard_resize();
    let steps = s.doc().unwrap().history.undo.len();
    let preview = document(&s);
    s.execute(
        "command.batch",
        &json!({"label":"Rename after resize","commands":[
            {"command":"artboard.setProps","params":{"index":0,"name":"Renamed"}}
        ]}),
    )
    .unwrap();
    assert_eq!(s.doc().unwrap().history.undo.len(), steps + 2, "resize and batch need separate boundaries");
    assert!(!s.in_interaction() && !s.tool_busy());
    let accepted = document(&s);
    assert_replay(&s);
    pointer(&mut s, PointerKind::Drag, 200.0, 160.0);
    pointer(&mut s, PointerKind::Up, 200.0, 160.0);
    assert_eq!(document(&s), accepted, "late events cannot change the accepted batch");
    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(document(&s), preview);
    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(s.doc().unwrap().doc.artboards[0].rect, Rect::new(0.0, 0.0, 100.0, 80.0));
    s.execute("edit.redo", &json!({})).unwrap();
    s.execute("edit.redo", &json!({})).unwrap();
    assert_eq!(document(&s), accepted);
    assert_replay(&s);
}
#[test]
fn rejected_batch_during_artboard_resize_keeps_preview_and_continuation() {
    let mut s = artboard_resize();
    let mut control = artboard_resize();
    let preview = document(&s);
    let selection = s.doc().unwrap().selection.clone();
    let options = s.tool_options();
    let steps = s.doc().unwrap().history.undo.len();
    let journal = s.journal.clone();
    let error = s
        .execute(
            "command.batch",
            &json!({"label":"Rejected page rename","commands":[
                {"command":"artboard.setProps","params":{"index":0,"name":"Discarded"}},
                {"command":"artboard.setProps","params":{"index":1,"name":"Missing"}}
            ]}),
        )
        .unwrap_err();
    assert!(error.to_string().contains("batch step 1"));
    assert_eq!(document(&s), preview, "rejection must retain the visible 150x120 resize");
    assert_eq!(s.doc().unwrap().selection, selection);
    assert_eq!(s.tool_options(), options);
    assert_eq!(s.doc().unwrap().history.undo.len(), steps);
    assert_eq!(s.journal, journal);
    assert!(s.in_interaction() && s.tool_busy());
    for state in [&mut s, &mut control] {
        pointer(state, PointerKind::Drag, 200.0, 160.0);
        pointer(state, PointerKind::Up, 200.0, 160.0);
    }
    assert_eq!(document(&s), document(&control), "resumed resize must match a no-batch control");
    assert_eq!(s.doc().unwrap().history.undo.len(), steps + 1);
    assert!(!s.in_interaction() && !s.tool_busy());
    assert_replay(&s);
    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(s.doc().unwrap().doc.artboards[0].rect, Rect::new(0.0, 0.0, 100.0, 80.0));
    s.execute("edit.redo", &json!({})).unwrap();
    assert_eq!(document(&s), document(&control));
}
#[test]
fn rejected_idle_batch_restores_active_board_before_delete() {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width":100,"height":80,"created":1_000_000_000})).unwrap();
    s.execute("artboard.new", &json!({"x":200,"y":0,"width":100,"height":80,"name":"Second"})).unwrap();
    s.select_tool("artboard", ViewInfo::default()).unwrap();
    pointer(&mut s, PointerKind::Down, 250.0, 40.0);
    pointer(&mut s, PointerKind::Up, 250.0, 40.0);
    assert_eq!(s.tool_options()["active"], 1);
    assert!(!s.in_interaction() && !s.tool_busy());
    let before = document(&s);
    let first = s.doc().unwrap().doc.artboards[0].clone();
    let options = s.tool_options();
    let steps = s.doc().unwrap().history.undo.len();
    let journal = s.journal.clone();
    s.execute(
        "command.batch",
        &json!({"label":"Rejected delete","commands":[
            {"command":"artboard.delete","params":{"index":0}},
            {"command":"artboard.setProps","params":{"index":99,"name":"Missing"}}
        ]}),
    )
    .unwrap_err();
    assert_eq!(document(&s), before);
    assert_eq!(s.doc().unwrap().history.undo.len(), steps);
    assert_eq!(s.journal, journal);
    assert_eq!(s.tool_options(), options, "a rejected batch must restore the active board, not just its document");
    s.tool_key(ToolKey::Delete, Mods::default(), ViewInfo::default()).unwrap();
    assert_eq!(s.doc().unwrap().doc.artboards, vec![first], "Delete must remove Second, which was active before rejection");
    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(document(&s), before);
}
fn pen_preview() -> Session {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width":800,"height":600,"created":1_000_000_000})).unwrap();
    s.execute("artboard.new", &json!({"x":900,"y":0,"width":100,"height":80,"name":"Second"})).unwrap();
    s.select_tool("pen", ViewInfo::default()).unwrap();
    pointer(&mut s, PointerKind::Down, 100.0, 300.0);
    pointer(&mut s, PointerKind::Up, 100.0, 300.0);
    pointer(&mut s, PointerKind::Down, 200.0, 300.0);
    pointer(&mut s, PointerKind::Drag, 230.0, 320.0);
    assert!(s.in_interaction() && s.tool_busy());
    s
}
fn external_artboard_edit(command: &str, params: Value) {
    let mut s = pen_preview();
    let steps = s.doc().unwrap().history.undo.len();
    let preview = document(&s);
    s.execute(command, &params).unwrap();
    let accepted = document(&s);
    pointer(&mut s, PointerKind::Drag, 250.0, 350.0);
    pointer(&mut s, PointerKind::Up, 250.0, 350.0);
    assert_eq!(document(&s), accepted, "the remaining Pen preview must not discard {command}");
    assert_eq!(s.doc().unwrap().history.undo.len(), steps + 2);
    assert!(!s.in_interaction() && !s.tool_busy());
    assert_replay(&s);
    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(document(&s), preview);
    s.execute("edit.redo", &json!({})).unwrap();
    assert_eq!(document(&s), accepted);
    assert_replay(&s);
}
#[test]
fn pen_external_artboard_duplicate_survives_late_events_and_replays() {
    external_artboard_edit("artboard.duplicate", json!({"index":0,"art":true}));
}
#[test]
fn pen_external_artboard_reorder_survives_late_events_and_replays() {
    external_artboard_edit("artboard.reorder", json!({"index":1,"to":0}));
}
#[test]
fn refused_save_preserves_pen_metadata_including_cancel_snapshot() {
    let mut s = pen_preview();
    let before = s.doc().unwrap().clone();
    let journal = s.journal.clone();
    let options = s.tool_options();
    let plan = fileio::save_plan(&s, SaveMode::SaveAs, &json!({"path":"refused-save.vectorcraft","modified":2_000_000_000,"preview":false})).unwrap();
    let mut writer_called = false;
    let error = fileio::save_with(&mut s, plan, |_, _| {
        writer_called = true;
        Err(EngineError::Other("fixture writer refused; no bytes written".into()))
    })
    .unwrap_err();
    assert!(writer_called && error.to_string().contains("fixture writer refused"));
    assert_eq!(s.doc().unwrap().doc.metadata, before.doc.metadata, "a refused write is not a completed save date");
    assert_eq!(document(&s), serde_json::to_value(&*before.doc).unwrap());
    assert_eq!(s.doc().unwrap().path, before.path);
    assert_eq!(s.doc().unwrap().history.undo.len(), before.history.undo.len());
    assert_eq!(s.journal, journal);
    assert_eq!(s.tool_options(), options);
    assert!(s.in_interaction() && s.tool_busy());
    assert_eq!(s.doc().unwrap().interaction.as_ref().unwrap().doc.metadata, before.interaction.as_ref().unwrap().doc.metadata);
    s.cancel_interaction().unwrap();
    pointer(&mut s, PointerKind::Up, 230.0, 320.0);
    assert_eq!(s.doc().unwrap().doc.metadata, before.doc.metadata, "Cancel must not retain dates from a failed save");
    assert_eq!(s.doc().unwrap().history.undo.len(), before.history.undo.len());
}
