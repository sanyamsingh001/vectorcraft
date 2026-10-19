#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! One original normal-workflow compatibility check; validation-only, not a standalone PR test.

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

// Only an expected-document clone is edited here. No live state or implementation helper is used.
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
fn delete_center_then_average_join_corner_preserves_art_and_two_step_history() {
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

    // Establish a real saved checkpoint with the public command, not mark_saved or direct edits.
    // Parent supplies TMPDIR under the compatibility artifact directory for this validation-only test.
    let save_dir = vectorcraft_testkit::temp_dir("r76-delete-average-join");
    let save_path = save_dir.join("before-delete.vectorcraft");
    s.execute(
        "document.save",
        &json!({"path": save_path.to_str().unwrap(), "format": "vectorcraft", "modified": null, "preview": false, "compress": false}),
    )
    .unwrap();
    assert!(save_path.is_file(), "the normal save must have completed");

    let before = state(&s);
    assert_eq!(before.doc, expected_geometry(&before.doc, id, &[&[0.0, 20.0, 40.0, 60.0, 80.0]]), "fixture geometry");
    assert_eq!(before.selection, selected(id, &[(0, 2)]), "only the center anchor is selected");
    assert_ne!(before.doc.node(id).unwrap().appearance, before.doc.node(other).unwrap().appearance, "untouched art has distinct paint");
    assert!(!before.dirty && before.redo.is_empty());
    assert!(!before.undo.is_empty(), "setup history must be preserved, not replaced by two labels");
    assert_eq!(before.saved_path.as_deref(), save_path.to_str());

    let split_doc = expected_geometry(&before.doc, id, &[&[0.0, 20.0], &[60.0, 80.0]]);
    let joined_doc = expected_geometry(&before.doc, id, &[&[0.0, 40.0, 80.0]]);
    let cut_whole = selected(id, &[]);
    let picked_ends = selected(id, &[(0, 1), (1, 0)]);
    let joined_anchor = selected(id, &[(0, 1)]);

    s.execute("edit.clear", &json!({})).unwrap();
    let mut after_clear = before.clone();
    after_clear.doc = split_doc.clone();
    after_clear.selection = cut_whole;
    after_clear.undo.push(step("Clear", &before.doc, &before.selection));
    after_clear.dirty = true;
    assert_eq!(state(&s), after_clear, "COMPAT_DELETE_RUNS: two open runs, same object/paint/other art, one Clear edit");

    s.execute("select.anchorsMany", &json!({"items": [{"id": id.0, "anchors": [[0, 1], [1, 0]]}]})).unwrap();
    let mut before_join = after_clear.clone();
    before_join.selection = picked_ends.clone();
    assert_eq!(state(&s), before_join, "endpoint picking changes selection only, not geometry, history or saved state");

    let result = s.execute("path.averageJoin", &json!({"kind": "corner"})).unwrap();
    assert_eq!(result, json!({"id": id.0, "subpath": 0, "anchor": 1}));
    let mut after_join = before_join.clone();
    after_join.doc = joined_doc.clone();
    after_join.selection = joined_anchor.clone();
    after_join.undo.push(step("Average and Join", &split_doc, &picked_ends));
    assert_eq!(state(&s), after_join, "COMPAT_AVERAGE_JOIN: exact [0,40,80], same identity/appearance and one additional edit");

    assert_eq!(s.execute("edit.undo", &json!({})).unwrap()["undone"], "Average and Join");
    let mut undo_join = before_join.clone();
    undo_join.redo.push(step("Average and Join", &joined_doc, &joined_anchor));
    assert_eq!(state(&s), undo_join, "first Undo restores both picked endpoints and the complete split document/history");

    assert_eq!(s.execute("edit.undo", &json!({})).unwrap()["undone"], "Clear");
    let mut undo_clear = before.clone();
    undo_clear.redo = vec![step("Average and Join", &joined_doc, &joined_anchor), step("Clear", &split_doc, &picked_ends)];
    assert_eq!(state(&s), undo_clear, "second Undo restores the saved five-anchor document and original center selection");

    assert_eq!(s.execute("edit.redo", &json!({})).unwrap()["redone"], "Clear");
    // Undo stored the then-current endpoint picks: Redo restores these, not the earlier whole selection.
    assert_eq!(state(&s), undo_join, "first Redo restores the split document, endpoint picks and remaining Average and Join redo");

    assert_eq!(s.execute("edit.redo", &json!({})).unwrap()["redone"], "Average and Join");
    assert_eq!(state(&s), after_join, "second Redo restores exact joined art, selection, both edits and dirty state");
}
