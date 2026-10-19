#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Ordinary Join selection/preservation regressions, separate from Average and Join.
//! Original finite geometry; artwork and selections are built through public commands.

use serde_json::json;
use std::sync::Arc;
use vectorcraft_doc::{Document, NodeId, NodeKind, Selection};
use vectorcraft_engine::{EngineError, HistoryEntry, Session};
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

    fn assert_unchanged(&self, s: &Session) {
        let st = s.doc().unwrap();
        assert_eq!(st.doc, self.doc);
        assert_eq!(st.selection, self.selection);
        assert_eq!(entries(&st.history.undo), self.undo);
        assert_eq!(entries(&st.history.redo), self.redo);
        assert_eq!(st.is_dirty(), self.dirty);
    }
}

fn session() -> Session {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 600, "height": 400})).unwrap();
    s
}

fn make(s: &mut Session, d: &str) -> NodeId {
    NodeId(s.execute("path.create", &json!({"d": d})).unwrap()["id"].as_u64().unwrap())
}

fn path(s: &Session, id: NodeId) -> PathData {
    s.doc().unwrap().doc.node(id).unwrap().path_data().unwrap().clone()
}

fn pair(s: &mut Session) -> (NodeId, NodeId) {
    let a = make(s, "M0 0L100 0");
    let b = make(s, "M110 0L200 0");
    (a, b)
}

fn pick(s: &mut Session, a: NodeId, ai: usize, b: NodeId, bi: usize) {
    s.execute("select.anchorsMany", &json!({"items":[{"id":a.0,"anchors":[[0,ai]]},{"id":b.0,"anchors":[[0,bi]]}]})).unwrap();
    let sel = &s.doc().unwrap().selection;
    assert_eq!(sel.objects, [a, b]);
    assert_eq!(sel.anchors[&a], [(0, ai)].into());
    assert_eq!(sel.anchors[&b], [(0, bi)].into());
    assert!(sel.segments.is_empty());
}

// Only the expected-document clone is edited directly; fixture state uses commands.
fn expected_path(doc: &mut Document, id: NodeId, value: PathData) {
    let NodeKind::Path { path, .. } = &mut doc.node_mut(id).unwrap().kind else { panic!("fixture is a path") };
    *path = value;
}

fn line_points(xs: &[f64]) -> PathData {
    PathData::new(vec![SubPath::polyline(&xs.iter().map(|x| Point::new(*x, 0.0)).collect::<Vec<_>>(), false)])
}

fn assert_join_history(s: &mut Session, before: &Snapshot, kept: NodeId) {
    let after = Snapshot::of(s);
    let mut expected_selection = before.selection.clone();
    expected_selection.set([kept]);
    assert_eq!(after.selection, expected_selection, "joined path remains selected whole");
    assert_eq!(after.undo.len(), before.undo.len() + 1, "Join is one edit");
    assert_eq!(&after.undo[..before.undo.len()], before.undo.as_slice());
    assert_eq!(after.undo.last().unwrap(), &("Join".into(), before.doc.clone(), Arc::new(before.selection.clone())));
    assert!(after.redo.is_empty(), "successful Join replaces the old Redo branch");
    assert!(after.dirty);

    assert_eq!(s.execute("edit.undo", &json!({})).unwrap()["undone"], "Join");
    let undone = Snapshot::of(s);
    assert_eq!(undone.doc, before.doc);
    assert_eq!(undone.selection, before.selection);
    assert_eq!(undone.undo, before.undo);
    assert_eq!(undone.redo, vec![("Join".into(), after.doc.clone(), Arc::new(after.selection.clone()))]);
    assert_eq!(undone.dirty, before.dirty);

    assert_eq!(s.execute("edit.redo", &json!({})).unwrap()["redone"], "Join");
    after.assert_unchanged(s);
}

/// Joining an open contour must not silently delete other artwork on its owner.
/// Keeping B's closed contour under B preserves its identity/appearance. An unsupported
/// multi-contour request may instead be rejected, but only before changing any state.
#[test]
fn ordinary_join_preserves_second_paths_closed_contour_or_rejects_atomically() {
    let mut s = session();
    let a = make(&mut s, "M0 0L10 0");
    s.execute("paint.setStroke", &json!({"color":"#2468d8"})).unwrap();
    let b = make(&mut s, "M12 0L20 0 M100 100L120 100L110 120Z");
    s.execute("paint.setFill", &json!({"color":"#da5634"})).unwrap();
    s.execute("paint.setStroke", &json!({"color":"#23854a"})).unwrap();
    let unrelated = make(&mut s, "M300 200L330 210L315 240Z");
    // A normal undone move leaves a Redo branch that rejection must retain exactly.
    s.execute("object.move", &json!({"dx":3,"dy":7})).unwrap();
    s.execute("edit.undo", &json!({})).unwrap();
    s.execute("select.set", &json!({"ids":[a.0,b.0]})).unwrap();
    let pb = path(&s, b);
    assert_eq!(pb.subpaths.len(), 2);
    assert!(!pb.subpaths[0].closed && pb.subpaths[1].closed);
    assert_eq!(pb.subpaths[1].anchors.iter().map(|v| v.p).collect::<Vec<_>>(), [(100.0, 100.0), (120.0, 100.0), (110.0, 120.0)].map(Point::from));
    s.doc_mut().unwrap().mark_saved();
    let before = Snapshot::of(&s);
    assert_eq!(before.redo.len(), 1);
    assert_ne!(before.doc.node(a).unwrap().appearance, before.doc.node(b).unwrap().appearance);

    // Explicit ends isolate contour preservation from selected-endpoint inference.
    match s.execute("path.join", &json!({"ends":["last","first"]})) {
        Err(error) => {
            assert!(matches!(error, EngineError::BadParams { .. } | EngineError::Other(_)), "JOIN_CONTOUR_SAFE_REJECTION: {error}");
            before.assert_unchanged(&s);
        }
        Ok(_) => {
            assert!(s.doc().unwrap().doc.node(b).is_some(), "JOIN_PRESERVE_UNRELATED_CLOSED_CONTOUR: B's closed contour was deleted");
            let mut expected = before.doc.clone();
            let d = Arc::make_mut(&mut expected);
            expected_path(d, a, line_points(&[0.0, 10.0, 12.0, 20.0]));
            expected_path(d, b, PathData::new(vec![pb.subpaths[1].clone()]));
            assert_eq!(
                s.doc().unwrap().doc,
                expected,
                "only the joined open contours may change; B's appearance/identity stays with its closed contour"
            );
            assert_eq!(s.doc().unwrap().doc.node(unrelated), before.doc.node(unrelated));
            assert_join_history(&mut s, &before, a);
        }
    }
}

#[test]
fn ordinary_join_uses_picked_far_endpoints_without_explicit_ends() {
    let mut s = session();
    let (a, b) = pair(&mut s);
    pick(&mut s, a, 0, b, 1);
    s.doc_mut().unwrap().mark_saved();
    let before = Snapshot::of(&s);
    // This is the existing Connect Selected End Points command payload: no ids or ends.
    s.execute("path.join", &json!({})).unwrap();
    let expected_path_data = line_points(&[100.0, 0.0, 200.0, 110.0]);
    assert_eq!(
        path(&s, a),
        expected_path_data,
        "JOIN_PICKED_FAR_ENDPOINTS: the bridge must connect the selected 0 and 200, not the nearest 100 and 110"
    );
    let mut expected = before.doc.clone();
    let d = Arc::make_mut(&mut expected);
    expected_path(d, a, expected_path_data);
    d.remove(b).unwrap();
    assert_eq!(s.doc().unwrap().doc, expected);
    assert_join_history(&mut s, &before, a);
}

#[test]
fn ordinary_join_whole_object_selection_still_uses_nearest_pair() {
    let mut s = session();
    let (a, b) = pair(&mut s);
    s.execute("select.set", &json!({"ids":[a.0,b.0]})).unwrap();
    let selection = &s.doc().unwrap().selection;
    assert_eq!(selection.objects, [a, b]);
    assert!(selection.anchors.is_empty() && selection.segments.is_empty());
    s.doc_mut().unwrap().mark_saved();
    let before = Snapshot::of(&s);
    s.execute("path.join", &json!({})).unwrap();
    let mut expected = before.doc.clone();
    let d = Arc::make_mut(&mut expected);
    expected_path(d, a, line_points(&[0.0, 100.0, 110.0, 200.0]));
    d.remove(b).unwrap();
    assert_eq!(s.doc().unwrap().doc, expected, "whole-object Join retains nearest-pair behavior");
    assert_join_history(&mut s, &before, a);
}

#[test]
fn ordinary_join_explicit_ends_override_picked_endpoints() {
    let mut s = session();
    let (a, b) = pair(&mut s);
    pick(&mut s, a, 1, b, 0);
    s.doc_mut().unwrap().mark_saved();
    let before = Snapshot::of(&s);
    // Explicit ends retain precedence for command/Pen callers, even over a different picked pair.
    s.execute("path.join", &json!({"ids":[a.0,b.0],"ends":["first","last"]})).unwrap();
    let mut expected = before.doc.clone();
    let d = Arc::make_mut(&mut expected);
    expected_path(d, a, line_points(&[100.0, 0.0, 200.0, 110.0]));
    d.remove(b).unwrap();
    assert_eq!(s.doc().unwrap().doc, expected, "explicit endpoints retain precedence");
    assert_join_history(&mut s, &before, a);
}

#[test]
fn ordinary_join_selected_curve_contours_preserve_other_contours_and_handles() {
    let mut s = session();
    let a = make(&mut s, "M300 0L310 0 M0 20C20 30 80 40 100 50");
    s.execute("paint.setStroke", &json!({"color":"#2468d8"})).unwrap();
    let b = make(&mut s, "M400 100L420 100L410 120Z M500 0L510 0 M110 60C130 70 170 80 200 90");
    s.execute("paint.setFill", &json!({"color":"#da5634"})).unwrap();
    s.execute("paint.setStroke", &json!({"color":"#23854a"})).unwrap();
    s.execute("select.anchorsMany", &json!({"items":[{"id":a.0,"anchors":[[1,0]]},{"id":b.0,"anchors":[[2,1]]}]})).unwrap();
    let selection = &s.doc().unwrap().selection;
    assert_eq!(selection.objects, [a, b]);
    assert_eq!(selection.anchors[&a], [(1, 0)].into());
    assert_eq!(selection.anchors[&b], [(2, 1)].into());
    assert!(selection.segments.is_empty());
    s.doc_mut().unwrap().mark_saved();
    let before = Snapshot::of(&s);
    let (mut pa, mut pb) = (path(&s, a), path(&s, b));
    assert_eq!((pa.subpaths.len(), pb.subpaths.len()), (2, 3));
    assert_ne!(before.doc.node(a).unwrap().appearance, before.doc.node(b).unwrap().appearance);

    // Independent ordered endpoints and handle coordinates, not the Join/reverse implementation.
    let mut anchors = vec![pa.subpaths[1].anchors[1], pa.subpaths[1].anchors[0], pb.subpaths[2].anchors[1], pb.subpaths[2].anchors[0]];
    assert_eq!(anchors.iter().map(|v| v.p).collect::<Vec<_>>(), [(100.0, 50.0), (0.0, 20.0), (200.0, 90.0), (110.0, 60.0)].map(Point::from));
    for (anchor, (incoming, outgoing)) in anchors.iter_mut().zip([
        ((100.0, 50.0), (80.0, 40.0)),
        ((20.0, 30.0), (0.0, 20.0)),
        ((200.0, 90.0), (170.0, 80.0)),
        ((130.0, 70.0), (110.0, 60.0)),
    ]) {
        anchor.h_in = Point::from(incoming);
        anchor.h_out = Point::from(outgoing);
    }
    pa.subpaths[1] = SubPath::new(anchors, false);
    assert!(pb.subpaths.pop().is_some());
    let mut expected = before.doc.clone();
    let d = Arc::make_mut(&mut expected);
    expected_path(d, a, pa);
    expected_path(d, b, pb);

    s.execute("path.join", &json!({})).unwrap();
    assert_eq!(s.doc().unwrap().doc, expected, "selected contours join; other open/closed contours and their owner appearance remain exact");
    assert_join_history(&mut s, &before, a);
}

#[test]
fn ordinary_join_explicit_ids_ignore_unrelated_endpoint_selection() {
    let mut s = session();
    let (a, b) = pair(&mut s);
    let c = make(&mut s, "M300 0L400 0");
    let d = make(&mut s, "M410 0L500 0");
    pick(&mut s, c, 0, d, 1);
    s.doc_mut().unwrap().mark_saved();
    let before = Snapshot::of(&s);
    let mut expected = before.doc.clone();
    let doc = Arc::make_mut(&mut expected);
    expected_path(doc, a, line_points(&[0.0, 100.0, 110.0, 200.0]));
    doc.remove(b).unwrap();

    s.execute("path.join", &json!({"ids":[a.0,b.0]})).unwrap();
    assert_eq!(s.doc().unwrap().doc, expected, "explicit targets remain independent of another selected endpoint pair");
    assert_join_history(&mut s, &before, a);
}

#[test]
fn ordinary_join_selected_endpoint_orientations_use_the_picked_pair() {
    for (ai, bi, xs) in [
        (0, 0, [100.0, 0.0, 110.0, 200.0]),
        (0, 1, [100.0, 0.0, 200.0, 110.0]),
        (1, 0, [0.0, 100.0, 110.0, 200.0]),
        (1, 1, [0.0, 100.0, 200.0, 110.0]),
    ] {
        let mut s = session();
        let (a, b) = pair(&mut s);
        pick(&mut s, a, ai, b, bi);
        s.doc_mut().unwrap().mark_saved();
        let before = Snapshot::of(&s);
        let mut expected = before.doc.clone();
        let d = Arc::make_mut(&mut expected);
        expected_path(d, a, line_points(&xs));
        d.remove(b).unwrap();
        s.execute("path.join", &json!({})).unwrap();
        assert_eq!(s.doc().unwrap().doc, expected, "picked endpoint indices {ai}, {bi}");
        assert_join_history(&mut s, &before, a);
    }
}

#[test]
fn ordinary_join_explicit_ids_ignore_same_target_picks_and_keep_caller_order() {
    for reverse_ids in [false, true] {
        let mut s = session();
        let (a, b) = pair(&mut s);
        // These same-target far picks must not change an explicit ids-only call.
        pick(&mut s, a, 0, b, 1);
        s.doc_mut().unwrap().mark_saved();
        let before = Snapshot::of(&s);
        let (kept, consumed, xs) = if reverse_ids { (b, a, [200.0, 110.0, 100.0, 0.0]) } else { (a, b, [0.0, 100.0, 110.0, 200.0]) };
        let mut expected = before.doc.clone();
        let d = Arc::make_mut(&mut expected);
        expected_path(d, kept, line_points(&xs));
        d.remove(consumed).unwrap();

        s.execute("path.join", &json!({"ids":[kept.0,consumed.0]})).unwrap();
        assert_eq!(
            s.doc().unwrap().doc,
            expected,
            "JOIN_EXPLICIT_IDS_NEAREST: same-target picks must not override caller order or nearest endpoints ({reverse_ids})"
        );
        assert_join_history(&mut s, &before, kept);
    }
}

/// Consuming B's open contour must not recolor the closed artwork retained under B.
#[test]
fn ordinary_join_preserves_retained_contour_gradient_pixels() {
    use vectorcraft_color::Paint;
    use vectorcraft_geom::Affine;
    use vectorcraft_render::{RenderOptions, Renderer};

    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 600, "height": 400, "colorMode": "rgb"})).unwrap();
    let a = make(&mut s, "M0 0L10 0");
    s.execute("paint.setStroke", &json!({"color": "#2468d8"})).unwrap();
    let b = make(&mut s, "M12 0L20 0 M100 100L120 100L110 120Z");
    // Original four-stop artwork: a red plateau, a transition, then a blue plateau.
    // Flat colors around the sample avoid interpolation-rounding and edge-AA oracles.
    s.execute(
        "paint.setFill",
        &json!({"gradient": {"kind": "linear", "angle": 0, "stops": [
            {"offset": 0.0, "color": "#ff0000"},
            {"offset": 0.6, "color": "#ff0000"},
            {"offset": 0.8, "color": "#0000ff"},
            {"offset": 1.0, "color": "#0000ff"}
        ]}}),
    )
    .unwrap();
    s.execute("paint.setStroke", &json!({"none": true})).unwrap();
    let unrelated = make(&mut s, "M300 200L330 210L315 240Z");
    s.execute("paint.setFill", &json!({"color": "#23854a"})).unwrap();
    s.execute("object.move", &json!({"dx": 3, "dy": 7})).unwrap();
    s.execute("edit.undo", &json!({})).unwrap();
    s.execute("select.set", &json!({"ids": [a.0, b.0]})).unwrap();
    s.doc_mut().unwrap().mark_saved();
    let before = Snapshot::of(&s);
    assert_eq!(before.redo.len(), 1, "JOIN_GRADIENT_SETUP_HISTORY");
    assert_eq!(before.selection.objects, [a, b], "JOIN_GRADIENT_SETUP_SELECTION");
    let Paint::Gradient(initial_gradient) = before.doc.node(b).unwrap().appearance.fill_paint() else {
        panic!("JOIN_GRADIENT_SETUP_PAINT: B must have a gradient fill")
    };
    assert!(initial_gradient.geom.is_none(), "JOIN_GRADIENT_SETUP_PAINT: fresh bounds-fitted fill");
    let retained = PathData::new(vec![SubPath::polyline(&[(100.0, 100.0), (120.0, 100.0), (110.0, 120.0)].map(Point::from), true)]);
    let pb = path(&s, b);
    assert_eq!(pb.subpaths.len(), 2, "JOIN_GRADIENT_SETUP_CONTOURS");
    assert_eq!(pb.subpaths[1], retained.subpaths[0], "JOIN_GRADIENT_SETUP_TRIANGLE");

    // Public CPU renderer, identity world-to-pixel mapping, fresh caches each time.
    // Pixel (110,110) is wholly inside the unchanged triangle, away from its edges.
    let pixel = |s: &Session| Renderer::new().render(&s.doc().unwrap().doc, 160, 160, Affine::IDENTITY, &RenderOptions::default()).pixel(110, 110);
    let original_pixel = pixel(&s);
    assert_eq!(original_pixel, [0, 0, 255, 255], "JOIN_GRADIENT_SETUP_PIXEL: original blue interior");
    eprintln!("JOIN_GRADIENT_SETUP_READY");

    // Ordinary whole-object Join: no explicit ids/ends or direct document mutation.
    s.execute("path.join", &json!({})).expect("JOIN_GRADIENT_COMMAND: ordinary Join must succeed");
    eprintln!("JOIN_GRADIENT_COMMAND_COMPLETED");
    assert!(s.doc().unwrap().doc.node(b).is_some(), "JOIN_GRADIENT_B_MISSING: retained contour owner was deleted");
    assert_eq!(path(&s, b), retained, "JOIN_GRADIENT_B_GEOMETRY: retained triangle changed");
    assert_eq!(path(&s, a), line_points(&[0.0, 10.0, 12.0, 20.0]), "JOIN_GRADIENT_A_GEOMETRY");
    eprintln!("JOIN_GRADIENT_B_RETAINED");

    let mut expected = before.doc.clone();
    let d = Arc::make_mut(&mut expected);
    expected_path(d, a, line_points(&[0.0, 10.0, 12.0, 20.0]));
    expected_path(d, b, retained);
    // Allow only gradient placement storage to change: rendered color is the oracle for
    // that representation. All other paint, identity, ownership and document fields stay exact.
    let Paint::Gradient(actual_gradient) = s.doc().unwrap().doc.node(b).unwrap().appearance.fill_paint() else {
        panic!("JOIN_GRADIENT_PAINT_KIND: B lost its gradient")
    };
    let Paint::Gradient(expected_gradient) = &mut d.node_mut(b).unwrap().appearance.fill_mut().unwrap().paint else {
        panic!("JOIN_GRADIENT_EXPECTED_PAINT: fixture lost its gradient")
    };
    expected_gradient.geom = actual_gradient.geom;
    assert_eq!(s.doc().unwrap().doc, expected, "JOIN_GRADIENT_DOCUMENT: unrelated fields changed");
    assert_eq!(s.doc().unwrap().doc.node(unrelated), before.doc.node(unrelated), "JOIN_GRADIENT_UNRELATED_ART");
    eprintln!("JOIN_GRADIENT_STRUCTURE_PRESERVED");

    assert_eq!(pixel(&s), [0, 0, 255, 255], "JOIN_GRADIENT_RECOLOR: retained B interior must remain the original blue");
    eprintln!("JOIN_GRADIENT_PIXEL_PRESERVED");
    assert_join_history(&mut s, &before, a);
    assert_eq!(pixel(&s), original_pixel, "JOIN_GRADIENT_REDO_PIXEL: Redo must restore the same blue interior");
    eprintln!("JOIN_GRADIENT_HISTORY_PRESERVED");
}
