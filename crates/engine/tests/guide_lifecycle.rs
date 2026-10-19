//! Guide commands issued externally while a small Pen handle preview is live.
//! Baseline and post-integration execution are deferred to the parent.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::Arc;

use serde_json::{Value, json};
use vectorcraft_doc::{Document, NodeId};
use vectorcraft_engine::{EngineError, History, Session, ViewInfo};
use vectorcraft_geom::PathData;
use vectorcraft_tools::{PointerEvent, PointerKind};

fn document(s: &Session) -> Value {
    serde_json::to_value(&*s.doc().unwrap().doc).unwrap()
}

fn path(s: &Session, id: NodeId) -> PathData {
    s.doc().unwrap().doc.node(id).unwrap().path_data().unwrap().clone()
}

fn pointer(s: &mut Session, kind: PointerKind, x: f64, y: f64) {
    s.pointer(&PointerEvent::new(kind, x, y), ViewInfo::default()).unwrap();
}

fn session() -> Session {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width":800,"height":600,"created":1_000_000_000})).unwrap();
    s
}

fn add_guide(s: &mut Session, vertical: bool, pos: f64, owned: bool) -> usize {
    let mut p = json!({"vertical":vertical,"pos":pos});
    if owned {
        p["artboard"] = json!(0);
    }
    s.execute("guide.add", &p).unwrap()["index"].as_u64().unwrap() as usize
}

fn start_live_pen(mut s: Session, with_redo: bool) -> (Session, NodeId, PathData, usize) {
    // Match the established pen_commands / pen_artboard_setprops gesture convention.
    s.select_tool("pen", ViewInfo::default()).unwrap();
    pointer(&mut s, PointerKind::Down, 100.0, 300.0);
    pointer(&mut s, PointerKind::Up, 100.0, 300.0);
    let id = s.doc().unwrap().selection.objects[0];
    let first_anchor = path(&s, id);
    assert_eq!(first_anchor.anchor_count(), 1);
    if with_redo {
        // Create and undo one ordinary tiny rectangle between anchors. This leaves a real
        // redo entry, whose full snapshot must survive a rejected command during the preview.
        let before = document(&s);
        s.execute("shape.rectangle", &json!({"x":10,"y":10,"width":10,"height":10})).unwrap();
        s.execute("edit.undo", &json!({})).unwrap();
        assert_eq!(document(&s), before);
        assert_eq!(s.doc().unwrap().selection.objects, [id]);
        assert_eq!(s.doc().unwrap().history.redo.len(), 1);
    }
    let steps = s.doc().unwrap().history.undo.len();
    pointer(&mut s, PointerKind::Down, 200.0, 300.0);
    pointer(&mut s, PointerKind::Drag, 230.0, 320.0);
    assert!(s.in_interaction() && s.tool_busy());
    assert_eq!(path(&s, id).anchor_count(), 2);
    if with_redo {
        assert_eq!(s.doc().unwrap().history.redo.len(), 1, "a preview has not committed the new anchor");
    }
    (s, id, first_anchor, steps)
}

fn assert_replay(s: &Session) {
    let mut replay = Session::new();
    for (command, params) in &s.journal {
        replay.execute(command, params).unwrap();
    }
    assert_eq!(document(&replay), document(s), "replay must preserve the complete document, including guide state");
}

fn assert_history_unchanged(actual: &History, before: &History) {
    assert_eq!(actual.limit, before.limit);
    for (actual, before) in [(&actual.undo, &before.undo), (&actual.redo, &before.redo)] {
        assert_eq!(actual.len(), before.len());
        for (actual, before) in actual.iter().zip(before) {
            assert_eq!(actual.label, before.label);
            assert!(Arc::ptr_eq(&actual.doc, &before.doc));
            assert_eq!(actual.selection, before.selection);
        }
    }
}

fn assert_external_edit_history(s: &Session, steps: usize, journal_before: &[(String, Value)], command: &str, params: &Value, label: &str) {
    let st = s.doc().unwrap();
    assert!(!s.in_interaction() && !s.tool_busy(), "a successful external edit ends the live gesture");
    assert_eq!(st.history.undo.len(), steps + 2, "Pen and the external guide edit are separate undo steps");
    assert_eq!(st.history.undo[steps].label, "Pen");
    assert_eq!(st.history.undo[steps + 1].label, label);
    assert!(st.history.redo.is_empty());
    assert_eq!(s.journal.len(), journal_before.len() + 2);
    assert_eq!(&s.journal[..journal_before.len()], journal_before);
    assert_eq!(s.journal[journal_before.len()].0, "path.appendAnchor");
    assert_eq!(s.journal[journal_before.len() + 1], (command.into(), params.clone()));
}

fn assert_undo_redo(
    s: &mut Session,
    id: NodeId,
    first_anchor: &PathData,
    preview: &Document,
    selection: &vectorcraft_doc::Selection,
    edited: &Value,
) {
    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(document(s), serde_json::to_value(preview).unwrap(), "one undo reverses only the guide edit");
    assert_eq!(&s.doc().unwrap().selection, selection);
    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(&path(s, id), first_anchor, "the preceding undo reverses the Pen anchor");
    assert_eq!(s.doc().unwrap().doc.guides, preview.guides);
    s.execute("edit.redo", &json!({})).unwrap();
    assert_eq!(document(s), serde_json::to_value(preview).unwrap());
    assert_eq!(&s.doc().unwrap().selection, selection);
    s.execute("edit.redo", &json!({})).unwrap();
    assert_eq!(&document(s), edited);
    assert_eq!(&s.doc().unwrap().selection, selection);
}

#[test]
fn guide_setprops_during_live_pen_survives_late_events_undo_and_replay() {
    for (initial, params, expected) in [
        ((true, false), json!({"index":0,"visible":false}), (false, false)),
        ((true, false), json!({"index":0,"locked":true}), (true, true)),
        ((true, false), json!({"index":0,"visible":false,"locked":true}), (false, true)),
        ((false, true), json!({"index":0,"visible":true,"locked":false}), (true, false)),
    ] {
        for globally_locked in [false, true] {
            let mut s = session();
            assert_eq!(add_guide(&mut s, true, 40.0, true), 0);
            add_guide(&mut s, false, 80.0, false);
            if initial != (true, false) {
                s.execute("guide.setProps", &json!({"index":0,"visible":initial.0,"locked":initial.1})).unwrap();
            }
            s.execute("view.guides.lock", &json!({"locked":globally_locked})).unwrap();
            let (mut s, id, first_anchor, steps) = start_live_pen(s, false);
            let preview = s.doc().unwrap().clone();
            let visible_path = path(&s, id);
            let journal = s.journal.clone();
            let out = s.execute("guide.setProps", &params).unwrap();
            assert_eq!(out, json!({"index":0,"visible":expected.0,"locked":expected.1}));
            assert_eq!(path(&s, id), visible_path);
            let edited = document(&s);
            pointer(&mut s, PointerKind::Drag, 250.0, 350.0);
            pointer(&mut s, PointerKind::Up, 250.0, 350.0);
            let guides = &s.doc().unwrap().doc.guides;
            assert_eq!((guides[0].visible, guides[0].locked), expected, "late Pen events must not revert guide properties");
            assert_eq!(
                (guides[0].vertical, guides[0].pos, guides[0].artboard),
                (preview.doc.guides[0].vertical, 40.0, preview.doc.guides[0].artboard)
            );
            assert_eq!(guides[1], preview.doc.guides[1], "the other guide stays untouched");
            assert_eq!(document(&s), edited);
            assert_eq!(s.doc().unwrap().selection, preview.selection);
            assert_eq!(s.guides_locked(), globally_locked, "individual properties do not alter the global lock");
            assert_external_edit_history(&s, steps, &journal, "guide.setProps", &params, "Guide Properties");
            assert_replay(&s);
            assert_undo_redo(&mut s, id, &first_anchor, &preview.doc, &preview.selection, &edited);
        }
    }
}

#[test]
fn clear_guides_during_live_pen_preserves_protected_guides_undo_and_replay() {
    let mut s = session();
    for (vertical, pos, owned, visible, locked) in
        [(true, 40.0, true, true, false), (false, 80.0, true, false, false), (true, 60.0, false, true, true), (false, 90.0, false, false, true)]
    {
        let index = add_guide(&mut s, vertical, pos, owned);
        if !visible || locked {
            s.execute("guide.setProps", &json!({"index":index,"visible":visible,"locked":locked})).unwrap();
        }
    }
    // One original 10x10 guide path exercises Clear Guides' existing path-removal branch.
    let guide_path = NodeId(s.execute("shape.rectangle", &json!({"x":10,"y":10,"width":10,"height":10})).unwrap()["id"].as_u64().unwrap());
    assert_eq!(s.execute("view.guides.make", &json!({})).unwrap()["count"], 1);
    let (mut s, id, first_anchor, steps) = start_live_pen(s, false);
    let preview = s.doc().unwrap().clone();
    let journal = s.journal.clone();
    let params = json!({});
    let out = s.execute("view.guides.clear", &params).unwrap();
    assert_eq!(out["count"], 2, "one editable ruler and one guide path are removed");
    assert_eq!(s.doc().unwrap().doc.guides, preview.doc.guides[1..]);
    assert!(s.doc().unwrap().doc.node(guide_path).is_none());
    let edited = document(&s);
    pointer(&mut s, PointerKind::Drag, 250.0, 350.0);
    pointer(&mut s, PointerKind::Up, 250.0, 350.0);
    assert_eq!(s.doc().unwrap().doc.guides, preview.doc.guides[1..], "late Pen events must not restore cleared rulers");
    assert!(s.doc().unwrap().doc.node(guide_path).is_none(), "late Pen events must not restore the guide path");
    assert_eq!(document(&s), edited);
    assert_eq!(s.doc().unwrap().selection, preview.selection);
    assert_external_edit_history(&s, steps, &journal, "view.guides.clear", &params, "Clear Guides");
    assert_replay(&s);
    assert_undo_redo(&mut s, id, &first_anchor, &preview.doc, &preview.selection, &edited);
}

#[test]
fn invalid_guide_setprops_preserves_live_pen_tool_preview_and_complete_history() {
    for globally_locked in [false, true] {
        let mut s = session();
        add_guide(&mut s, true, 40.0, true);
        s.execute("view.guides.lock", &json!({"locked":globally_locked})).unwrap();
        let (mut s, id, first_anchor, steps) = start_live_pen(s, true);
        let before = s.doc().unwrap().clone();
        let interaction = before.interaction.as_ref().unwrap();
        let preview = path(&s, id);
        let original = document(&s);
        let journal = s.journal.clone();
        let prefs = s.prefs.clone();
        let paint = s.paint.clone();
        let clipboard = s.clipboard.clone();
        let tool = s.tool_id();
        let options = s.tool_options();
        let active = s.active_index();
        for params in [
            json!({"index":0}),
            json!({"visible":false}),
            json!({"index":-1,"locked":true}),
            json!({"index":0.5,"locked":true}),
            json!({"index":"0","locked":true}),
            json!({"index":1,"visible":false}),
            json!({"index":0,"visible":"false"}),
            json!({"index":0,"locked":0}),
            json!({"index":0,"visible":false,"locked":null}),
        ] {
            let out = s.execute("guide.setProps", &params);
            assert!(matches!(out, Err(EngineError::BadParams { ref cmd, .. }) if cmd == "guide.setProps"), "{params} must be a parameter rejection");
            let st = s.doc().unwrap();
            assert_eq!(document(&s), original);
            assert!(Arc::ptr_eq(&st.doc, &before.doc));
            assert_eq!(st.selection, before.selection);
            assert_history_unchanged(&st.history, &before.history);
            assert_eq!(st.revision, before.revision);
            assert_eq!(st.is_dirty(), before.is_dirty());
            assert_eq!(st.active_layer, before.active_layer);
            assert_eq!(st.layer_rows, before.layer_rows);
            assert_eq!(st.isolation, before.isolation);
            assert_eq!(st.last_transform, before.last_transform);
            assert_eq!(st.last_perspective, before.last_perspective);
            let current = st.interaction.as_ref().expect("rejection retains the live preview");
            assert!(Arc::ptr_eq(&current.doc, &interaction.doc));
            assert_eq!(current.label, interaction.label);
            assert_eq!(current.preview, interaction.preview);
            assert_eq!(current.selection, interaction.selection);
            assert_eq!(current.active_layer, interaction.active_layer);
            assert_eq!(current.layer_rows, interaction.layer_rows);
            assert_eq!(current.isolation, interaction.isolation);
            assert_eq!(current.perspective_again, interaction.perspective_again);
            assert_eq!(path(&s, id), preview);
            assert_eq!(s.journal, journal);
            assert_eq!(s.prefs, prefs);
            assert_eq!(s.paint, paint);
            assert_eq!(s.clipboard, clipboard);
            assert_eq!(s.tool_id(), tool);
            assert_eq!(s.tool_options(), options);
            assert_eq!(s.active_index(), active);
            assert_eq!(s.guides_locked(), globally_locked);
            assert!(s.in_interaction() && s.tool_busy());
            assert!(st.undo_group.is_none());
        }
        pointer(&mut s, PointerKind::Drag, 250.0, 350.0);
        pointer(&mut s, PointerKind::Up, 250.0, 350.0);
        assert_ne!(path(&s, id), preview, "the rejected commands leave the handle editable");
        assert_eq!(path(&s, id).anchor_count(), 2);
        assert_eq!(s.doc().unwrap().doc.guides, before.doc.guides);
        assert_eq!(s.doc().unwrap().history.undo.len(), steps + 1);
        assert!(s.doc().unwrap().history.redo.is_empty(), "only the successful Pen commit clears redo");
        assert!(!s.in_interaction() && !s.tool_busy());
        assert_eq!(&s.journal[..journal.len()], &journal);
        assert_eq!(s.journal.len(), journal.len() + 1);
        assert_eq!(s.journal.last().unwrap().0, "path.appendAnchor");
        let committed = document(&s);
        assert_replay(&s);
        s.execute("edit.undo", &json!({})).unwrap();
        assert_eq!(path(&s, id), first_anchor);
        assert_eq!(document(&s), serde_json::to_value(&*interaction.doc).unwrap());
        s.execute("edit.redo", &json!({})).unwrap();
        assert_eq!(document(&s), committed);
    }
}
