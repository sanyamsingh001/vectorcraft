//! Ordinary Pen/Artboard command and batch regressions.
//! All art is generated through commands; resize uses the default Artboard tool options.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
use serde_json::{Value, json};
use vectorcraft_engine::{Session, ViewInfo};
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

/// Reordering must keep the same active board through history, including the next Delete.
#[test]
fn external_artboard_reorder_follows_active_board_through_undo_redo_and_delete() {
    for batch in [false, true] {
        let mut s = Session::new();
        s.execute("file.new", &json!({"width":100,"height":80,"created":1_000_000_000})).unwrap();
        s.execute("artboard.new", &json!({"x":200,"y":0,"width":100,"height":80,"name":"Second"})).unwrap();
        let original = s.doc().unwrap().doc.artboards.clone();
        s.select_tool("artboard", ViewInfo::default()).unwrap();
        pointer(&mut s, PointerKind::Down, 250.0, 40.0);
        pointer(&mut s, PointerKind::Up, 250.0, 40.0);
        let active_id = |s: &Session| {
            let index = s.tool_options()["active"].as_u64().unwrap() as usize;
            s.doc().unwrap().doc.artboards[index].id
        };
        assert_eq!(active_id(&s), original[1].id, "positive control: Second is active before reorder");
        assert!(!s.in_interaction() && !s.tool_busy());
        let steps = s.doc().unwrap().history.undo.len();
        if batch {
            s.execute("command.batch", &json!({"label":"Reorder Second","commands":[{"command":"artboard.reorder","params":{"index":1,"to":0}}]}))
                .unwrap();
        } else {
            s.execute("artboard.reorder", &json!({"index":1,"to":0})).unwrap();
        }
        assert_eq!(s.doc().unwrap().doc.artboards, vec![original[1].clone(), original[0].clone()]);
        assert_eq!(active_id(&s), original[1].id, "the accepted command follows Second's stable ID");
        assert_eq!(s.doc().unwrap().history.undo.len(), steps + 1);
        assert_replay(&s);
        s.execute("edit.undo", &json!({})).unwrap();
        assert_eq!(s.doc().unwrap().doc.artboards, original);
        assert_eq!(active_id(&s), original[1].id, "Undo must not silently activate First");
        s.execute("edit.redo", &json!({})).unwrap();
        assert_eq!(s.doc().unwrap().doc.artboards, vec![original[1].clone(), original[0].clone()]);
        assert_eq!(active_id(&s), original[1].id, "Redo follows the same surviving board");
        s.execute("edit.undo", &json!({})).unwrap();
        s.tool_key(ToolKey::Delete, Mods::default(), ViewInfo::default()).unwrap();
        assert_eq!(s.doc().unwrap().doc.artboards, vec![original[0].clone()], "Delete must remove Second after Undo");
        s.execute("edit.undo", &json!({})).unwrap();
        assert_eq!(s.doc().unwrap().doc.artboards, original);
        assert!(!s.in_interaction() && !s.tool_busy());
        assert!(s.prefs.tool_settings.get("artboard").is_none_or(|options| !options.contains_key("active")));
        assert_replay(&s);
    }
}
