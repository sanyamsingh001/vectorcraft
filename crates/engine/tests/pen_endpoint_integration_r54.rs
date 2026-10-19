//! Original tiny-geometry controls for Pen endpoint/Alt integration with continue-any-path.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use serde_json::json;
use vectorcraft_doc::{Node, NodeId, NodeKind};
use vectorcraft_engine::{Session, ViewInfo};
use vectorcraft_geom::{AnchorKind, Point};
use vectorcraft_tools::{Cursor, Mods, PointerEvent, PointerKind};

fn session() -> Session {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 800, "height": 600})).unwrap();
    s.prefs.selection_tolerance = 8.0;
    s.prefs.use_precise_cursors = false;
    s
}

fn view() -> ViewInfo {
    ViewInfo { zoom: 1.0, smart_guides: false, snap_to_grid: false, snap_to_pixel: false, snap_to_point: false, ..Default::default() }
}

fn node(s: &Session, id: NodeId) -> &Node {
    s.doc().unwrap().doc.node(id).unwrap()
}

fn path_ids(s: &Session) -> Vec<NodeId> {
    let mut ids = vec![];
    s.doc().unwrap().doc.walk(|n| {
        if matches!(n.kind, NodeKind::Path { .. }) {
            ids.push(n.id);
        }
    });
    ids.sort_unstable();
    ids
}

fn points(s: &Session, id: NodeId) -> Vec<(f64, f64)> {
    let path = node(s, id).path_data().unwrap();
    assert_eq!(path.subpaths.len(), 1);
    assert!(!path.subpaths[0].closed);
    path.subpaths[0].anchors.iter().map(|a| (a.p.x, a.p.y)).collect()
}

fn line(s: &mut Session, from: (f64, f64), to: (f64, f64)) -> NodeId {
    let result = s.execute("shape.line", &json!({"x1": from.0, "y1": from.1, "x2": to.0, "y2": to.1})).unwrap();
    NodeId(result["id"].as_u64().unwrap())
}

fn gesture(s: &mut Session, view: ViewInfo, samples: &[(f64, f64)], mods: Mods) {
    let first = samples.first().unwrap();
    s.pointer(&PointerEvent::new(PointerKind::Down, first.0, first.1).with_mods(mods), view).unwrap();
    for &(x, y) in samples.iter().skip(1) {
        s.pointer(&PointerEvent::new(PointerKind::Drag, x, y).with_mods(mods), view).unwrap();
    }
    let last = samples.last().unwrap();
    s.pointer(&PointerEvent::new(PointerKind::Up, last.0, last.1).with_mods(mods), view).unwrap();
}

// B is above A. Its first endpoint coincides with A's interior corner, not either end of A.
fn overlapping_endpoint() -> (Session, NodeId, NodeId) {
    let mut s = session();
    let result = s.execute("path.create", &json!({"anchors": [{"x": 100, "y": 100}, {"x": 200, "y": 100}, {"x": 300, "y": 100}]})).unwrap();
    let a = NodeId(result["id"].as_u64().unwrap());
    let b = line(&mut s, (200.0, 100.0), (200.0, 200.0));
    s.execute("select.set", &json!({"ids": [a.0]})).unwrap();
    s.select_tool("pen", view()).unwrap();
    assert_eq!(s.cursor(Point::new(300.0, 100.0), Mods::default(), view()), Cursor::PenContinue);
    gesture(&mut s, view(), &[(300.0, 100.0)], Mods::default());
    assert_eq!(path_ids(&s), vec![a, b]);
    assert_eq!(s.doc().unwrap().selection.objects, vec![a]);
    (s, a, b)
}

#[test]
fn alt_at_another_endpoint_reports_conversion_and_preserves_objects_through_undo_redo() {
    let (mut s, a, b) = overlapping_endpoint();
    let before_a = node(&s, a).clone();
    let before_b = node(&s, b).clone();
    let before_undo = s.doc().unwrap().history.undo.len();
    let alt = Mods { alt: true, ..Default::default() };
    let advertised = s.cursor(Point::new(200.0, 100.0), alt, view());
    gesture(&mut s, view(), &[(200.0, 100.0), (200.0, 120.0)], alt);

    let mut converted_a = before_a.clone();
    let NodeKind::Path { path, .. } = &mut converted_a.kind else { panic!("fixture must be a path") };
    let middle = &mut path.subpaths[0].anchors[1];
    middle.kind = AnchorKind::Smooth;
    middle.h_in = Point::new(200.0, 80.0);
    middle.h_out = Point::new(200.0, 120.0);
    assert_eq!(path_ids(&s), vec![a, b], "Alt conversion must not join away either object");
    assert_eq!(node(&s, a), &converted_a, "only A's interior corner becomes smooth");
    assert_eq!(node(&s, b), &before_b, "B's endpoint is under the pointer but B must stay unchanged");
    assert_eq!(s.doc().unwrap().history.undo.len(), before_undo + 1);
    assert_eq!(s.execute("edit.undo", &json!({})).unwrap()["undone"], "Convert Anchor Point");
    assert_eq!(path_ids(&s), vec![a, b]);
    assert_eq!(node(&s, a), &before_a);
    assert_eq!(node(&s, b), &before_b);
    assert_eq!(s.doc().unwrap().history.undo.len(), before_undo);
    s.execute("edit.redo", &json!({})).unwrap();
    assert_eq!(path_ids(&s), vec![a, b]);
    assert_eq!(node(&s, a), &converted_a);
    assert_eq!(node(&s, b), &before_b);
    assert_eq!(s.doc().unwrap().history.undo.len(), before_undo + 1);
    // Check the captured cursor last: a mismatch must not hide the actual conversion/history controls.
    assert_eq!(advertised, Cursor::PenConvert, "Alt cursor must describe the conversion actually performed, not Join");
}

#[test]
fn plain_click_at_same_endpoint_joins_paths_with_undo_redo() {
    let (mut s, a, b) = overlapping_endpoint();
    let before_a = node(&s, a).clone();
    let before_b = node(&s, b).clone();
    let before_undo = s.doc().unwrap().history.undo.len();
    assert_eq!(s.cursor(Point::new(200.0, 100.0), Mods::default(), view()), Cursor::PenJoin);
    gesture(&mut s, view(), &[(200.0, 100.0)], Mods::default());
    assert_eq!(path_ids(&s), vec![a]);
    assert!(s.doc().unwrap().doc.node(b).is_none());
    assert_eq!(points(&s, a), [(100.0, 100.0), (200.0, 100.0), (300.0, 100.0), (200.0, 100.0), (200.0, 200.0)]);
    assert_eq!(s.doc().unwrap().selection.objects, vec![a]);
    assert_eq!(s.doc().unwrap().history.undo.len(), before_undo + 1);
    let joined = node(&s, a).clone();
    assert_eq!(s.execute("edit.undo", &json!({})).unwrap()["undone"], "Join");
    assert_eq!(path_ids(&s), vec![a, b]);
    assert_eq!(node(&s, a), &before_a);
    assert_eq!(node(&s, b), &before_b);
    s.execute("edit.redo", &json!({})).unwrap();
    assert_eq!(path_ids(&s), vec![a]);
    assert_eq!(node(&s, a), &joined);
    assert!(s.doc().unwrap().doc.node(b).is_none());
}

fn grid_continue(preselected: bool, snap_to_grid: bool) {
    let mut s = session();
    let mut prefs = s.prefs.clone();
    prefs.gridline_every = 100.0;
    prefs.grid_subdivisions = 1;
    s.apply_prefs(prefs);
    assert_eq!(s.doc().unwrap().doc.grid.spacing, 100.0);
    assert_eq!(s.doc().unwrap().doc.grid.subdivisions, 1);
    let id = line(&mut s, (13.0, 303.0), (63.0, 303.0));
    let original = node(&s, id).clone();
    s.execute("select.set", &json!({"ids": if preselected { vec![id.0] } else { vec![] }})).unwrap();
    let view = ViewInfo { snap_to_grid, ..view() };
    s.select_tool("pen", view).unwrap();
    assert_eq!(s.cursor(Point::new(63.0, 303.0), Mods::default(), view), Cursor::PenContinue);
    let journal_start = s.journal.len();
    gesture(&mut s, view, &[(63.0, 303.0)], Mods::default());
    assert_eq!(path_ids(&s), vec![id], "clicking the advertised endpoint must continue it, not create a snapped path");
    assert_eq!(node(&s, id), &original);
    assert_eq!(s.doc().unwrap().selection.objects, vec![id]);
    assert!(!s.journal[journal_start..].iter().any(|(command, _)| command == "path.create"));
    let before_append = s.doc().unwrap().history.undo.len();
    gesture(&mut s, view, &[(200.0, 300.0)], Mods::default());
    assert_eq!(path_ids(&s), vec![id]);
    assert_eq!(points(&s, id), [(13.0, 303.0), (63.0, 303.0), (200.0, 300.0)]);
    assert_eq!(s.doc().unwrap().history.undo.len(), before_append + 1);
    let extended = node(&s, id).clone();
    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(node(&s, id), &original);
    s.execute("edit.redo", &json!({})).unwrap();
    assert_eq!(node(&s, id), &extended);
}

#[test]
fn unselected_grid_snapped_endpoint_continues_at_raw_pointer() {
    grid_continue(false, true);
}

#[test]
fn preselected_grid_snapped_endpoint_continues_at_raw_pointer() {
    grid_continue(true, true);
}

#[test]
fn unselected_endpoint_continues_with_grid_snap_disabled() {
    grid_continue(false, false);
}

fn nearest_first_continue(preselected: bool) {
    let mut s = session();
    let id = line(&mut s, (100.0, 300.0), (106.0, 300.0));
    s.execute("select.set", &json!({"ids": if preselected { vec![id.0] } else { vec![] }})).unwrap();
    s.select_tool("pen", view()).unwrap();
    // Both ends are within the explicit 8 px tolerance, but the first is exactly under the pointer.
    assert_eq!(s.cursor(Point::new(100.0, 300.0), Mods::default(), view()), Cursor::PenContinue);
    gesture(&mut s, view(), &[(100.0, 300.0)], Mods::default());
    assert_eq!(path_ids(&s), vec![id]);
    assert_eq!(s.doc().unwrap().selection.objects, vec![id]);
    gesture(&mut s, view(), &[(80.0, 300.0)], Mods::default());
    assert_eq!(path_ids(&s), vec![id]);
    assert_eq!(points(&s, id), [(106.0, 300.0), (100.0, 300.0), (80.0, 300.0)], "continue from the nearer first end");
}

#[test]
fn unselected_short_path_continues_nearest_first_endpoint() {
    nearest_first_continue(false);
}

#[test]
fn preselected_short_path_continues_nearest_first_endpoint() {
    nearest_first_continue(true);
}
