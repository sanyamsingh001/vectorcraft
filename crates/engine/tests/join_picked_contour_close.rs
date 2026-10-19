#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Ordinary Join must close the picked contour, not an earlier unselected open contour.

use serde_json::json;
use std::sync::Arc;
use vectorcraft_doc::{Document, NodeId, NodeKind, Selection};
use vectorcraft_engine::{HistoryEntry, Session};
use vectorcraft_geom::{PathData, Point, SubPath};

type Entries = Vec<(String, Arc<Document>, Arc<Selection>)>;

fn entries(items: &[HistoryEntry]) -> Entries {
    items.iter().map(|e| (e.label.clone(), e.doc.clone(), e.selection.clone())).collect()
}

#[derive(Debug, PartialEq)]
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
        assert!(st.interaction.is_none(), "ordinary clicks and Join must not leave an interaction");
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
fn ordinary_join_closes_picked_later_contour_without_closing_earlier_open_contour() {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 320, "height": 160})).unwrap();
    let id = make(&mut s, "M20 20L40 40L60 20 M100 20L120 40L140 20 M180 20L200 20L190 40Z");
    s.execute("paint.setFill", &json!({"none": true})).unwrap();
    s.execute("paint.setStroke", &json!({"color": "#2468d8"})).unwrap();
    make(&mut s, "M250 90L290 90L270 130Z");
    s.execute("paint.setFill", &json!({"color": "#da5634"})).unwrap();
    // A normal undone move supplies an independent Redo branch for Join to replace.
    s.execute("object.move", &json!({"dx": 3, "dy": 7})).unwrap();
    s.execute("edit.undo", &json!({})).unwrap();

    // The same public selection commands as an unselected-anchor click then Shift-click.
    s.execute("select.anchors", &json!({"id": id.0, "anchors": [[1, 0]], "mode": "set"})).unwrap();
    s.execute("select.anchors", &json!({"id": id.0, "anchors": [[1, 2]], "mode": "toggle"})).unwrap();
    s.doc_mut().unwrap().mark_saved();
    let before = Snapshot::of(&s);
    assert_eq!(before.selection.objects, [id]);
    assert_eq!(before.selection.anchors.len(), 1);
    assert_eq!(before.selection.anchors[&id], [(1, 0), (1, 2)].into());
    assert!(before.selection.segments.is_empty() && before.selection.guides.is_empty());
    assert!(!before.dirty && before.redo.len() == 1);
    let original = before.doc.node(id).unwrap().path_data().unwrap();
    assert_eq!(original.subpaths.iter().map(|sp| sp.closed).collect::<Vec<_>>(), [false, false, true]);
    assert!(original.subpaths.iter().all(|sp| sp.anchors.len() == 3));

    // Literal independent geometry: only contour1 closes. No Join/reverse helper builds this oracle.
    let expected_path = PathData::new(vec![
        SubPath::polyline(&[(20.0, 20.0), (40.0, 40.0), (60.0, 20.0)].map(Point::from), false),
        SubPath::polyline(&[(100.0, 20.0), (120.0, 40.0), (140.0, 20.0)].map(Point::from), true),
        SubPath::polyline(&[(180.0, 20.0), (200.0, 20.0), (190.0, 40.0)].map(Point::from), true),
    ]);
    // Only an expected-document clone is edited directly; live setup uses public commands.
    let mut expected_doc = before.doc.clone();
    let NodeKind::Path { path, .. } = &mut Arc::make_mut(&mut expected_doc).node_mut(id).unwrap().kind else { panic!("fixture is a path") };
    *path = expected_path;

    s.execute("path.join", &json!({})).unwrap();
    let after = Snapshot::of(&s);
    let actual = after.doc.node(id).unwrap().path_data().unwrap();
    assert_eq!(
        actual.subpaths.iter().map(|sp| sp.closed).collect::<Vec<_>>(),
        [false, true, true],
        "JOIN_PICKED_LATER_CONTOUR_CLOSE: close selected contour1; leave earlier open contour0 and unrelated closed contour2 unchanged"
    );
    assert_eq!(after.doc, expected_doc, "all geometry, handles, identity, appearance and unrelated artwork stay exact apart from contour1 closing");
    // Closing distinct endpoints does not renumber them: retain ordinary close's picked selection.
    assert_eq!(after.selection, before.selection);
    assert_eq!(after.undo.len(), before.undo.len() + 1, "Join is one history edit");
    assert_eq!(&after.undo[..before.undo.len()], before.undo.as_slice());
    assert_eq!(after.undo.last().unwrap(), &("Join".into(), before.doc.clone(), Arc::new(before.selection.clone())));
    assert!(after.redo.is_empty() && after.dirty);
    assert_eq!((after.limit, after.byte_budget), (before.limit, before.byte_budget));

    assert_eq!(s.execute("edit.undo", &json!({})).unwrap()["undone"], "Join");
    let undone = Snapshot::of(&s);
    assert_eq!(undone.doc, before.doc);
    assert_eq!(undone.selection, before.selection);
    assert_eq!(undone.undo, before.undo);
    assert_eq!(undone.redo, vec![("Join".into(), after.doc.clone(), Arc::new(after.selection.clone()))]);
    assert_eq!(undone.dirty, before.dirty);
    assert_eq!((undone.limit, undone.byte_budget), (before.limit, before.byte_budget));
    assert_eq!(s.execute("edit.redo", &json!({})).unwrap()["redone"], "Join");
    assert_eq!(Snapshot::of(&s), after);
}
