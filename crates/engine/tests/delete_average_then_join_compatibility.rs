#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! One original three-command workflow for validation only, not standalone PR content.

use serde_json::json;
use std::sync::Arc;
use vectorcraft_doc::{Document, NodeId, NodeKind, Selection};
use vectorcraft_engine::{HistoryEntry, Session};
use vectorcraft_geom::{Point, SubPath};

type Entries = Vec<(String, Arc<Document>, Arc<Selection>)>;

fn entries(items: &[HistoryEntry]) -> Entries {
    items.iter().map(|e| (e.label.clone(), e.doc.clone(), e.selection.clone())).collect()
}

#[derive(Clone, Debug, PartialEq)]
struct State {
    doc: Arc<Document>,
    selection: Selection,
    undo: Entries,
    redo: Entries,
    history_limit: usize,
    history_byte_budget: usize,
    dirty: bool,
    saved_path: Option<String>,
}

fn state(s: &Session) -> State {
    let st = s.doc().unwrap();
    State {
        doc: st.doc.clone(),
        selection: st.selection.clone(),
        undo: entries(&st.history.undo),
        redo: entries(&st.history.redo),
        history_limit: st.history.limit,
        history_byte_budget: st.history.byte_budget,
        dirty: st.is_dirty(),
        saved_path: st.path.clone(),
    }
}

fn selected(id: NodeId, anchors: &[(usize, usize)]) -> Selection {
    let mut selection = Selection { objects: vec![id], ..Selection::default() };
    if !anchors.is_empty() {
        selection.anchors.insert(id, anchors.iter().copied().collect());
    }
    selection
}

// Only expected-document clones are edited directly. Literal coordinates define each state.
fn expected_geometry(before: &Arc<Document>, id: NodeId, contours: &[&[f64]]) -> Arc<Document> {
    let mut expected = before.clone();
    let NodeKind::Path { path, .. } = &mut Arc::make_mut(&mut expected).node_mut(id).unwrap().kind else { panic!("fixture is a path") };
    path.subpaths = contours.iter().map(|xs| SubPath::polyline(&xs.iter().map(|&x| Point::new(x, 0.0)).collect::<Vec<_>>(), false)).collect();
    expected
}

fn step(label: &str, doc: &Arc<Document>, selection: &Selection) -> (String, Arc<Document>, Arc<Selection>) {
    (label.to_string(), doc.clone(), Arc::new(selection.clone()))
}

#[test]
fn delete_center_then_average_then_join_preserves_art_and_three_step_history() {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 400, "height": 300})).unwrap();
    let other = NodeId(s.execute("path.create", &json!({"d": "M200 150L240 150L220 190Z"})).unwrap()["id"].as_u64().unwrap());
    s.execute("paint.setFill", &json!({"color": "#d65a38"})).unwrap();
    s.execute("paint.setStroke", &json!({"color": "#23854a"})).unwrap();
    let anchors: Vec<_> = [0, 20, 40, 60, 80].into_iter().map(|x| json!({"x": x, "y": 0})).collect();
    let id = NodeId(s.execute("path.create", &json!({"anchors": anchors, "closed": false})).unwrap()["id"].as_u64().unwrap());
    s.execute("paint.setFill", &json!({"none": true})).unwrap();
    s.execute("paint.setStroke", &json!({"color": "#2468d8"})).unwrap();
    s.execute("select.anchorsMany", &json!({"items": [{"id": id.0, "anchors": [[0, 2]]}]})).unwrap();

    // A real public save establishes the checkpoint; parent supplies ROOT-local TMPDIR.
    let save_dir = vectorcraft_testkit::temp_dir("r78-delete-average-then-join");
    let save_path = save_dir.join("before-delete.vectorcraft");
    s.execute(
        "document.save",
        &json!({"path": save_path.to_str().unwrap(), "format": "vectorcraft", "modified": null, "preview": false, "compress": false}),
    )
    .unwrap();
    let saved_bytes = std::fs::read(&save_path).unwrap();
    assert!(!saved_bytes.is_empty(), "the public save completed");

    let before = state(&s);
    assert_eq!(before.doc, expected_geometry(&before.doc, id, &[&[0.0, 20.0, 40.0, 60.0, 80.0]]), "fixture geometry");
    assert_eq!(before.selection, selected(id, &[(0, 2)]));
    assert_ne!(before.doc.node(id).unwrap().appearance, before.doc.node(other).unwrap().appearance);
    assert!(!before.dirty && before.redo.is_empty());
    assert!(!before.undo.is_empty(), "preserve the complete setup history");
    assert_eq!(before.saved_path.as_deref(), save_path.to_str());

    let split_doc = expected_geometry(&before.doc, id, &[&[0.0, 20.0], &[60.0, 80.0]]);
    let averaged_doc = expected_geometry(&before.doc, id, &[&[0.0, 40.0], &[40.0, 80.0]]);
    let joined_doc = expected_geometry(&before.doc, id, &[&[0.0, 40.0, 80.0]]);
    let whole = selected(id, &[]);
    let picked_ends = selected(id, &[(0, 1), (1, 0)]);

    s.execute("edit.clear", &json!({})).unwrap();
    let mut after_clear = before.clone();
    after_clear.doc = split_doc.clone();
    after_clear.selection = whole.clone();
    after_clear.undo.push(step("Clear", &before.doc, &before.selection));
    after_clear.dirty = true;
    assert_eq!(state(&s), after_clear, "COMPAT_THREE_DELETE: two open runs, same identity, appearance and unrelated art");

    s.execute("select.anchorsMany", &json!({"items": [{"id": id.0, "anchors": [[0, 1], [1, 0]]}]})).unwrap();
    let mut before_average = after_clear.clone();
    before_average.selection = picked_ends.clone();
    assert_eq!(state(&s), before_average, "picking changes only selection");

    s.execute("path.average", &json!({"axis": "both"})).unwrap();
    let mut after_average = before_average.clone();
    after_average.doc = averaged_doc.clone();
    after_average.undo.push(step("Average", &split_doc, &picked_ends));
    assert_eq!(state(&s), after_average, "COMPAT_THREE_AVERAGE: both picked endpoints move to40 but remain in separate open contours");

    s.execute("path.join", &json!({})).unwrap();
    let mut after_join = after_average.clone();
    after_join.doc = joined_doc.clone();
    after_join.selection = whole.clone();
    after_join.undo.push(step("Join", &averaged_doc, &picked_ends));
    assert_eq!(state(&s), after_join, "COMPAT_THREE_JOIN: exact open [0,40,80], one midpoint anchor and a separate Join history step");

    assert_eq!(s.execute("edit.undo", &json!({})).unwrap()["undone"], "Join");
    let mut undo_join = after_average.clone();
    undo_join.redo = vec![step("Join", &joined_doc, &whole)];
    assert_eq!(state(&s), undo_join, "Undo1 restores two coincident picked endpoints and the complete averaged state");

    assert_eq!(s.execute("edit.undo", &json!({})).unwrap()["undone"], "Average");
    let mut undo_average = before_average.clone();
    undo_average.redo = vec![step("Join", &joined_doc, &whole), step("Average", &averaged_doc, &picked_ends)];
    assert_eq!(state(&s), undo_average, "Undo2 restores endpoints20/60, their selection and the full split state");

    assert_eq!(s.execute("edit.undo", &json!({})).unwrap()["undone"], "Clear");
    let mut undo_clear = before.clone();
    undo_clear.redo = vec![step("Join", &joined_doc, &whole), step("Average", &averaged_doc, &picked_ends), step("Clear", &split_doc, &picked_ends)];
    assert_eq!(state(&s), undo_clear, "Undo3 restores the saved five-anchor document and center selection");
    assert!(Arc::ptr_eq(&s.doc().unwrap().doc, &before.doc), "Undo reaches the actual saved document snapshot");

    // Undo captured the endpoint reselection: Redo Clear restores that selection, not the earlier whole-object selection.
    assert_eq!(s.execute("edit.redo", &json!({})).unwrap()["redone"], "Clear");
    assert_eq!(state(&s), undo_average, "Redo1 restores the exact split state and remaining Average/Join branch");
    assert_eq!(s.execute("edit.redo", &json!({})).unwrap()["redone"], "Average");
    assert_eq!(state(&s), undo_join, "Redo2 restores the exact averaged state and remaining Join branch");
    assert_eq!(s.execute("edit.redo", &json!({})).unwrap()["redone"], "Join");
    assert_eq!(state(&s), after_join, "Redo3 restores the exact joined document, selection, three history edits and dirty state");
    assert_eq!(std::fs::read(&save_path).unwrap(), saved_bytes, "editing and history navigation do not overwrite the saved file");
}
