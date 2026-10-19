#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Closing picked coincident endpoints keeps only the surviving anchor selected.

use serde_json::json;
use std::sync::Arc;
use vectorcraft_doc::{Document, NodeId, NodeKind, Selection};
use vectorcraft_engine::{HistoryEntry, Session};
use vectorcraft_geom::{PathData, Point, SubPath};

type Entries = Vec<(String, Arc<Document>, Arc<Selection>)>;

fn entries(items: &[HistoryEntry]) -> Entries {
    items.iter().map(|e| (e.label.clone(), e.doc.clone(), e.selection.clone())).collect()
}

#[derive(Clone, Debug, PartialEq)]
struct Snapshot {
    doc: Arc<Document>,
    selection: Selection,
    undo: Entries,
    redo: Entries,
    dirty: bool,
    limit: usize,
    byte_budget: usize,
}

impl Snapshot {
    fn of(s: &Session) -> Self {
        let st = s.doc().unwrap();
        assert!(st.interaction.is_none(), "ordinary picks and Join must not leave an interaction");
        Self {
            doc: st.doc.clone(),
            selection: st.selection.clone(),
            undo: entries(&st.history.undo),
            redo: entries(&st.history.redo),
            dirty: st.is_dirty(),
            limit: st.history.limit,
            byte_budget: st.history.byte_budget,
        }
    }
}

fn make(s: &mut Session, d: &str) -> NodeId {
    NodeId(s.execute("path.create", &json!({"d": d})).unwrap()["id"].as_u64().unwrap())
}

#[test]
fn ordinary_join_coincident_later_contour_keeps_only_surviving_anchor_selected() {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 320, "height": 160})).unwrap();
    // No Z on contour1: its four anchors are open, with distinct first/last anchors at the same point.
    let id = make(&mut s, "M20 20L40 40L60 20 M100 20L140 20L120 50L100 20 M180 20L200 20L190 40Z");
    s.execute("paint.setFill", &json!({"none": true})).unwrap();
    s.execute("paint.setStroke", &json!({"color": "#2468d8"})).unwrap();
    make(&mut s, "M250 90L290 90L270 130Z");
    s.execute("paint.setFill", &json!({"color": "#da5634"})).unwrap();
    // A real undone move supplies an unrelated Redo entry; Join must replace that branch.
    s.execute("object.move", &json!({"dx": 3, "dy": 7})).unwrap();
    s.execute("edit.undo", &json!({})).unwrap();

    // Public anchor-pick commands, with no direct writes to live geometry or selection.
    s.execute("select.anchors", &json!({"id": id.0, "anchors": [[1, 0]], "mode": "set"})).unwrap();
    s.execute("select.anchors", &json!({"id": id.0, "anchors": [[1, 3]], "mode": "toggle"})).unwrap();
    s.doc_mut().unwrap().mark_saved();
    let before = Snapshot::of(&s);
    assert_eq!(before.selection.objects, [id]);
    assert_eq!(before.selection.anchors.len(), 1);
    assert_eq!(before.selection.anchors[&id], [(1, 0), (1, 3)].into());
    assert!(before.selection.segments.is_empty() && before.selection.guides.is_empty());
    assert!(!before.dirty && before.redo.len() == 1);
    let original = before.doc.node(id).unwrap().path_data().unwrap();
    assert_eq!(original.subpaths.iter().map(|sp| (sp.closed, sp.anchors.len())).collect::<Vec<_>>(), [(false, 3), (false, 4), (true, 3)]);
    assert_eq!(original.subpaths[1].anchors[0].p, original.subpaths[1].anchors[3].p);

    // Literal oracle: contour1 becomes a closed triangle; no Join helper creates expected geometry.
    let expected_path = PathData::new(vec![
        SubPath::polyline(&[(20.0, 20.0), (40.0, 40.0), (60.0, 20.0)].map(Point::from), false),
        SubPath::polyline(&[(100.0, 20.0), (140.0, 20.0), (120.0, 50.0)].map(Point::from), true),
        SubPath::polyline(&[(180.0, 20.0), (200.0, 20.0), (190.0, 40.0)].map(Point::from), true),
    ]);
    let mut expected_doc = before.doc.clone();
    let NodeKind::Path { path, .. } = &mut Arc::make_mut(&mut expected_doc).node_mut(id).unwrap().kind else { panic!("fixture is a path") };
    *path = expected_path;
    let mut expected_selection = before.selection.clone();
    expected_selection.anchors.insert(id, [(1, 0)].into());

    s.execute("path.join", &json!({})).unwrap();
    let after = Snapshot::of(&s);
    assert_eq!(after.doc, expected_doc, "only contour1 closes and loses its duplicate endpoint; other contours, artwork and appearance stay exact");
    assert_eq!(
        after.selection, expected_selection,
        "JOIN_COINCIDENT_PICKED_CONTOUR_SELECTION: only survivor (1,0) stays selected; discarded (1,3) must not survive"
    );
    let mut expected_after = before.clone();
    expected_after.doc = expected_doc;
    expected_after.selection = expected_selection;
    expected_after.undo.push(("Join".into(), before.doc.clone(), Arc::new(before.selection.clone())));
    expected_after.redo.clear();
    expected_after.dirty = true;
    assert_eq!(after, expected_after, "Join is one edit, invalidates prior Redo and retains history budgets");

    assert_eq!(s.execute("edit.undo", &json!({})).unwrap()["undone"], "Join");
    let mut expected_undone = before.clone();
    expected_undone.redo = vec![("Join".into(), after.doc.clone(), Arc::new(after.selection.clone()))];
    assert_eq!(Snapshot::of(&s), expected_undone, "Undo restores the complete saved document and original two-endpoint selection");
    assert_eq!(s.execute("edit.redo", &json!({})).unwrap()["redone"], "Join");
    assert_eq!(Snapshot::of(&s), after, "Redo restores the complete closed document and valid survivor-only selection");

    // Deselect the surviving anchor normally. An empty direct-selection entry is the existing API
    // representation here; it must contain no phantom endpoint and must not become whole-object art.
    s.execute("select.anchors", &json!({"id": id.0, "anchors": [[1, 0]], "mode": "toggle"})).unwrap();
    let mut expected_deselected = after.clone();
    expected_deselected.selection.anchors.insert(id, Default::default());
    assert_eq!(Snapshot::of(&s), expected_deselected, "toggling the survivor off leaves no selected anchor and changes no document/history state");
    s.execute("select.anchors", &json!({"id": id.0, "anchors": [[1, 0]], "mode": "toggle"})).unwrap();
    assert_eq!(Snapshot::of(&s), after, "toggling it back selects only the valid survivor without adding history");
}
