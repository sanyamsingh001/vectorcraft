#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Same-object ordinary Join: original finite geometry, constructed through commands.
//! This isolates the post-deletion shape without depending on the Anchor Delete repair.

use serde_json::json;
use std::sync::Arc;
use vectorcraft_doc::{Document, NodeId, NodeKind, Selection};
use vectorcraft_engine::{HistoryEntry, Session};
use vectorcraft_geom::{PathData, Point, SubPath};

type Entries = Vec<(String, Arc<Document>, Arc<Selection>)>;

fn entries(items: &[HistoryEntry]) -> Entries {
    items.iter().map(|e| (e.label.clone(), e.doc.clone(), e.selection.clone())).collect()
}

struct Snapshot {
    doc: Arc<Document>,
    selection: Selection,
    undo: Entries,
    redo: Entries,
    dirty: bool,
}

impl Snapshot {
    fn of(s: &Session) -> Self {
        let st = s.doc().unwrap();
        Self {
            doc: st.doc.clone(),
            selection: st.selection.clone(),
            undo: entries(&st.history.undo),
            redo: entries(&st.history.redo),
            dirty: st.is_dirty(),
        }
    }

    fn assert_same(&self, s: &Session) {
        let actual = Self::of(s);
        assert_eq!(actual.doc, self.doc);
        assert_eq!(actual.selection, self.selection);
        assert_eq!(actual.undo, self.undo);
        assert_eq!(actual.redo, self.redo);
        assert_eq!(actual.dirty, self.dirty);
    }
}

fn make(s: &mut Session, d: &str) -> NodeId {
    NodeId(s.execute("path.create", &json!({"d": d})).unwrap()["id"].as_u64().unwrap())
}

fn fixture(d: &str) -> (Session, NodeId) {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 200, "height": 120})).unwrap();
    let id = make(&mut s, d);
    s.execute("paint.setStroke", &json!({"color": "#2468d8"})).unwrap();
    make(&mut s, "M140 70L170 70L155 100Z");
    s.execute("paint.setFill", &json!({"color": "#da5634"})).unwrap();
    // A normal undone edit leaves a real Redo branch for Join to replace.
    s.execute("object.move", &json!({"dx": 3, "dy": 7})).unwrap();
    s.execute("edit.undo", &json!({})).unwrap();
    (s, id)
}

fn path(s: &Session, id: NodeId) -> PathData {
    s.doc().unwrap().doc.node(id).unwrap().path_data().unwrap().clone()
}

fn pick(s: &mut Session, id: NodeId, anchors: &[(usize, usize)]) {
    s.execute("select.anchorsMany", &json!({"items": [{"id": id.0, "anchors": anchors}]})).unwrap();
    let sel = &s.doc().unwrap().selection;
    assert_eq!(sel.objects, [id]);
    assert_eq!(sel.anchors.len(), 1);
    assert_eq!(sel.anchors[&id].iter().copied().collect::<Vec<_>>(), anchors.to_vec());
    assert!(sel.segments.is_empty() && sel.guides.is_empty());
    s.doc_mut().unwrap().mark_saved();
}

// Only the independent expected-document clone is modified directly.
fn expected_document(before: &Snapshot, id: NodeId, joined: PathData) -> Arc<Document> {
    let mut expected = before.doc.clone();
    let NodeKind::Path { path, .. } = &mut Arc::make_mut(&mut expected).node_mut(id).unwrap().kind else { panic!("fixture is a path") };
    *path = joined;
    expected
}

fn assert_art_and_history(s: &mut Session, id: NodeId, before: &Snapshot, expected: Arc<Document>) {
    let after = Snapshot::of(s);
    assert_eq!(after.doc, expected, "only the target path geometry may change; identity, appearance and unrelated artwork stay exact");
    assert_eq!(after.selection.objects, [id]);
    // Do not prescribe a new post-Join fine-grained anchor-selection policy.
    assert_eq!(before.redo.len(), 1);
    assert!(!before.dirty);
    assert_eq!(after.undo.len(), before.undo.len() + 1, "Join is one history edit");
    assert_eq!(&after.undo[..before.undo.len()], before.undo.as_slice());
    assert_eq!(after.undo.last().unwrap(), &("Join".into(), before.doc.clone(), Arc::new(before.selection.clone())));
    assert!(after.redo.is_empty(), "Join replaces the prior Redo branch");
    assert!(after.dirty);

    assert_eq!(s.execute("edit.undo", &json!({})).unwrap()["undone"], "Join");
    let undone = Snapshot::of(s);
    assert_eq!(undone.doc, before.doc);
    assert_eq!(undone.selection, before.selection);
    assert_eq!(undone.undo, before.undo);
    assert_eq!(undone.redo, vec![("Join".into(), after.doc.clone(), Arc::new(after.selection.clone()))]);
    assert_eq!(undone.dirty, before.dirty);

    assert_eq!(s.execute("edit.redo", &json!({})).unwrap()["redone"], "Join");
    after.assert_same(s);
}

#[test]
fn ordinary_join_selected_gap_endpoints_in_one_object_reconnects_open_subpaths() {
    let (mut s, id) = fixture("M20 40L40 20 M80 20L100 40");
    pick(&mut s, id, &[(0, 1), (1, 0)]);
    let original = path(&s, id);
    assert_eq!(original.subpaths.len(), 2);
    assert!(original.subpaths.iter().all(|sp| !sp.closed && sp.anchors.len() == 2));
    let before = Snapshot::of(&s);
    let points = [(20.0, 40.0), (40.0, 20.0), (80.0, 20.0), (100.0, 40.0)].map(Point::from);
    let joined = PathData::new(vec![SubPath::polyline(&points, false)]);
    let expected = expected_document(&before, id, joined.clone());

    s.execute("path.join", &json!({})).unwrap();
    let actual = path(&s, id);
    assert_eq!(actual.subpaths.len(), 1, "JOIN_SINGLE_OBJECT_GAP_ENDPOINTS: connect the selected gap, not close the first contour");
    assert!(!actual.subpaths[0].closed, "JOIN_SINGLE_OBJECT_GAP_OPEN: the bridge must remain open");
    assert_eq!(actual, joined, "JOIN_SINGLE_OBJECT_GAP_GEOMETRY: preserve A-B-D-E and bridge B to D without restoring the deleted center");
    assert_art_and_history(&mut s, id, &before, expected);
}

#[test]
fn ordinary_join_selected_ends_of_single_contour_still_closes() {
    let (mut s, id) = fixture("M20 40L40 20L80 40");
    pick(&mut s, id, &[(0, 0), (0, 2)]);
    let mut closed = path(&s, id);
    assert_eq!(closed.subpaths.len(), 1);
    assert!(!closed.subpaths[0].closed);
    assert_eq!(closed.subpaths[0].anchors.len(), 3);
    let before = Snapshot::of(&s);
    closed.subpaths[0].closed = true;
    let expected = expected_document(&before, id, closed.clone());

    s.execute("path.join", &json!({})).unwrap();
    assert_eq!(path(&s, id), closed, "JOIN_SINGLE_CONTOUR_CLOSE_CONTROL: retain ordinary closure and all anchors");
    assert_art_and_history(&mut s, id, &before, expected);
}

#[test]
fn ordinary_join_same_object_reversed_curves_preserve_other_contours_and_history() {
    // Every row states its own vertex order and incoming/outgoing handles. Neither
    // the production reversal helper nor another row computes the expected geometry.
    let cases = [
        (
            "last-first",
            1,
            0,
            [
                ((20.0, 40.0), (20.0, 40.0), (30.0, 10.0)),
                ((60.0, 40.0), (50.0, 60.0), (60.0, 40.0)),
                ((100.0, 80.0), (100.0, 80.0), (110.0, 50.0)),
                ((140.0, 80.0), (130.0, 100.0), (140.0, 80.0)),
            ],
        ),
        (
            "first-first",
            0,
            0,
            [
                ((60.0, 40.0), (60.0, 40.0), (50.0, 60.0)),
                ((20.0, 40.0), (30.0, 10.0), (20.0, 40.0)),
                ((100.0, 80.0), (100.0, 80.0), (110.0, 50.0)),
                ((140.0, 80.0), (130.0, 100.0), (140.0, 80.0)),
            ],
        ),
        (
            "last-last",
            1,
            1,
            [
                ((20.0, 40.0), (20.0, 40.0), (30.0, 10.0)),
                ((60.0, 40.0), (50.0, 60.0), (60.0, 40.0)),
                ((140.0, 80.0), (140.0, 80.0), (130.0, 100.0)),
                ((100.0, 80.0), (110.0, 50.0), (100.0, 80.0)),
            ],
        ),
        (
            "first-last",
            0,
            1,
            [
                ((60.0, 40.0), (60.0, 40.0), (50.0, 60.0)),
                ((20.0, 40.0), (30.0, 10.0), (20.0, 40.0)),
                ((140.0, 80.0), (140.0, 80.0), (130.0, 100.0)),
                ((100.0, 80.0), (110.0, 50.0), (100.0, 80.0)),
            ],
        ),
    ];
    for (orientation, a_end, b_end, geometry) in cases {
        let (mut s, id) = fixture("M5 5L15 5L10 15Z M20 40C30 10 50 60 60 40 M160 10L180 10 M100 80C110 50 130 100 140 80");
        pick(&mut s, id, &[(1, a_end), (3, b_end)]);
        let original = path(&s, id);
        assert_eq!(original.subpaths.len(), 4);
        assert!(original.subpaths[0].closed && !original.subpaths[2].closed);
        assert!(original.subpaths.iter().skip(1).all(|sp| !sp.closed && sp.anchors.len() == 2));
        assert_eq!(original.subpaths[1].anchors[0].h_out, Point::new(30.0, 10.0));
        assert_eq!(original.subpaths[1].anchors[1].h_in, Point::new(50.0, 60.0));
        assert_eq!(original.subpaths[3].anchors[0].h_out, Point::new(110.0, 50.0));
        assert_eq!(original.subpaths[3].anchors[1].h_in, Point::new(130.0, 100.0));
        let before = Snapshot::of(&s);

        let points = geometry.map(|(position, _, _)| Point::from(position));
        let mut joined = SubPath::polyline(&points, false);
        for (anchor, (_, incoming, outgoing)) in joined.anchors.iter_mut().zip(geometry) {
            anchor.h_in = Point::from(incoming);
            anchor.h_out = Point::from(outgoing);
        }
        let expected_path = PathData::new(vec![original.subpaths[0].clone(), joined, original.subpaths[2].clone()]);
        let expected = expected_document(&before, id, expected_path.clone());

        s.execute("path.join", &json!({})).unwrap();
        assert_eq!(
            path(&s, id),
            expected_path,
            "JOIN_SINGLE_OBJECT_REVERSED_CURVES: {orientation}: preserve the explicit curve/bridge geometry and every unjoined contour"
        );
        assert_art_and_history(&mut s, id, &before, expected);
    }
}
