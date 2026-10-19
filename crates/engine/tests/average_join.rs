#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Original geometry fixtures for the one-step Average and Join command.
use serde_json::{Value, json};
use vectorcraft_doc::NodeId;
use vectorcraft_engine::Session;
use vectorcraft_geom::{AnchorKind, PathData, Point};

fn session() -> Session {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 600, "height": 400})).unwrap();
    s
}
fn path(s: &Session, id: NodeId) -> PathData {
    s.doc().unwrap().doc.node(id).unwrap().path_data().unwrap().clone()
}
fn make(s: &mut Session, anchors: Value, closed: bool) -> NodeId {
    NodeId(s.execute("path.create", &json!({"anchors": anchors, "closed": closed})).unwrap()["id"].as_u64().unwrap())
}
fn pick(s: &mut Session, items: Value) {
    s.execute("select.anchorsMany", &json!({"items": items})).unwrap();
}
fn pair(s: &mut Session) -> (NodeId, NodeId) {
    let a = make(s, json!([{"x": 20, "y": 100}, {"x": 100, "y": 100, "in": [80, 60]}]), false);
    let b = make(s, json!([{"x": 140, "y": 140, "out": [170, 150]}, {"x": 240, "y": 100}]), false);
    pick(s, json!([{"id": a.0, "anchors": [[0,1]]}, {"id": b.0, "anchors": [[0,0]]}]));
    (a, b)
}

#[test]
fn average_join_corner_is_one_step_and_retains_handles_selection_and_saved_state() {
    let mut s = session();
    let (a, b) = pair(&mut s);
    s.doc_mut().unwrap().mark_saved();
    let before = s.doc().unwrap().doc.clone();
    let selection = s.doc().unwrap().selection.clone();
    let undo = s.doc().unwrap().history.undo.len();
    s.execute("path.averageJoin", &json!({"kind": "corner"})).unwrap();
    let p = path(&s, a);
    let sp = &p.subpaths[0];
    assert!(!sp.closed);
    assert_eq!(sp.anchors.len(), 3);
    let join = sp.anchors[1];
    assert_eq!(join.p, Point::new(120.0, 120.0));
    assert_eq!(join.h_in, Point::new(100.0, 80.0));
    assert_eq!(join.h_out, Point::new(150.0, 130.0));
    assert_eq!(join.kind, AnchorKind::Corner);
    assert_eq!(sp.anchors[0], before.node(a).unwrap().path_data().unwrap().subpaths[0].anchors[0]);
    assert_eq!(sp.anchors[2], before.node(b).unwrap().path_data().unwrap().subpaths[0].anchors[1]);
    assert_eq!(s.doc().unwrap().doc.node(a).unwrap().appearance, before.node(a).unwrap().appearance);
    assert!(s.doc().unwrap().doc.node(b).is_none());
    assert_eq!(s.doc().unwrap().selection.anchors[&a], [(0, 1)].into());
    assert_eq!(s.doc().unwrap().history.undo.len(), undo + 1);
    assert!(s.doc().unwrap().is_dirty());
    let after = s.doc().unwrap().doc.clone();
    let after_sel = s.doc().unwrap().selection.clone();
    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(s.doc().unwrap().doc, before);
    assert_eq!(s.doc().unwrap().selection, selection);
    assert!(!s.doc().unwrap().is_dirty());
    s.execute("edit.redo", &json!({})).unwrap();
    assert_eq!(s.doc().unwrap().doc, after);
    assert_eq!(s.doc().unwrap().selection, after_sel);
}

#[test]
fn average_join_smooth_uses_neighbor_direction_and_third_distance_handles() {
    let mut s = session();
    let (a, _) = pair(&mut s);
    s.execute("path.averageJoin", &json!({"kind": "smooth"})).unwrap();
    let p = path(&s, a);
    let join = p.subpaths[0].anchors[1];
    assert_eq!(join.p, Point::new(120.0, 120.0));
    assert_eq!(join.kind, AnchorKind::Smooth);
    assert_eq!(join.h_in.y, 120.0);
    assert_eq!(join.h_out.y, 120.0);
    assert!((join.p.distance(join.h_in) - 10400_f64.sqrt() / 3.0).abs() < 1e-9);
    assert!((join.p.distance(join.h_out) - 14800_f64.sqrt() / 3.0).abs() < 1e-9);
    assert!(join.h_in.x < join.p.x && join.h_out.x > join.p.x);
}

#[test]
fn average_join_honors_all_four_endpoint_orientations() {
    for first_a in [false, true] {
        for first_b in [false, true] {
            let mut s = session();
            let (a, b) = pair(&mut s);
            if first_a {
                s.execute("path.reverse", &json!({"ids":[a.0]})).unwrap();
            }
            if !first_b {
                s.execute("path.reverse", &json!({"ids":[b.0]})).unwrap();
            }
            pick(&mut s, json!([{"id":a.0,"anchors":[[0,if first_a {0} else {1}]]},{"id":b.0,"anchors":[[0,if first_b {0} else {1}]]}]));
            s.execute("path.averageJoin", &json!({})).unwrap();
            let p = path(&s, a);
            assert_eq!(
                p.subpaths[0].anchors.iter().map(|a| a.p).collect::<Vec<_>>(),
                [Point::new(20.0, 100.0), Point::new(120.0, 120.0), Point::new(240.0, 100.0)]
            );
            assert_eq!(p.subpaths[0].anchors[1].h_in, Point::new(100.0, 80.0));
            assert_eq!(p.subpaths[0].anchors[1].h_out, Point::new(150.0, 130.0));
        }
    }
}

#[test]
fn average_join_closes_the_two_open_endpoints_at_their_midpoint() {
    let mut s = session();
    let a = make(&mut s, json!([{"x":20,"y":20},{"x":100,"y":20},{"x":100,"y":100},{"x":20,"y":100}]), false);
    pick(&mut s, json!([{"id":a.0,"anchors":[[0,0],[0,3]]}]));
    s.execute("path.averageJoin", &json!({})).unwrap();
    let p = path(&s, a);
    assert!(p.subpaths[0].closed);
    assert_eq!(p.subpaths[0].anchors.iter().map(|a| (a.p.x, a.p.y)).collect::<Vec<_>>(), [(20.0, 60.0), (100.0, 20.0), (100.0, 100.0)]);
}

#[test]
fn average_join_rejects_closed_and_interior_anchors_atomically() {
    for (closed, indices) in [(true, [0, 3]), (true, [0, 1]), (false, [1, 2]), (false, [0, 1])] {
        let mut s = session();
        let a = make(&mut s, json!([{"x":20,"y":20},{"x":100,"y":20},{"x":100,"y":100},{"x":20,"y":100}]), closed);
        pick(&mut s, json!([{"id":a.0,"anchors":[[0,indices[0]],[0,indices[1]]]}]));
        s.doc_mut().unwrap().mark_saved();
        let before = s.doc().unwrap().doc.clone();
        let selection = s.doc().unwrap().selection.clone();
        let undo = s.doc().unwrap().history.undo.len();
        for kind in ["corner", "smooth"] {
            assert!(s.execute("path.averageJoin", &json!({"kind":kind})).is_err());
            assert_eq!(s.doc().unwrap().doc, before);
            assert_eq!(s.doc().unwrap().selection, selection);
            assert_eq!(s.doc().unwrap().history.undo.len(), undo);
            assert!(!s.doc().unwrap().is_dirty());
        }
    }
}

#[test]
fn average_join_preserves_other_subpaths_on_both_objects() {
    let mut s = session();
    let (a, b) = pair(&mut s);
    for (id, main, extra) in [
        (a, json!([{"x":20,"y":100},{"x":100,"y":100}]), json!([{"x":300,"y":300},{"x":400,"y":300}])),
        (b, json!([{"x":140,"y":140},{"x":240,"y":100}]), json!([{"x":500,"y":200},{"x":550,"y":250}])),
    ] {
        s.execute(
            "path.setAnchors",
            &json!({"id":id.0,"subpaths":[
                {"anchors":main,"closed":false},
                {"anchors":extra,"closed":false}
            ]}),
        )
        .unwrap();
    }
    pick(&mut s, json!([{"id":a.0,"anchors":[[0,1]]},{"id":b.0,"anchors":[[0,0]]}]));
    let extra_a = path(&s, a).subpaths[1].clone();
    let extra_b = path(&s, b).subpaths[1].clone();
    assert_ne!(extra_a, extra_b, "distinct unselected contours expose accidental substitution");
    s.execute("path.averageJoin", &json!({})).unwrap();
    assert_eq!(path(&s, a).subpaths[1], extra_a);
    assert_eq!(path(&s, b).subpaths, vec![extra_b]);
    // Two different subpaths of one node: merge only those two, leaving the other object intact.
    let b_before = s.doc().unwrap().doc.node(b).unwrap().clone();
    pick(&mut s, json!([{"id":a.0,"anchors":[[0,2],[1,0]]}]));
    s.execute("path.averageJoin", &json!({})).unwrap();
    assert_eq!(path(&s, a).subpaths.len(), 1);
    assert_eq!(path(&s, a).subpaths[0].anchors.len(), 4);
    assert_eq!(s.doc().unwrap().doc.node(b).unwrap(), &b_before);
}

#[test]
fn average_join_rejects_unsupported_selection_without_losing_redo_or_saved_state() {
    let mut s = session();
    let a = make(&mut s, json!([{"x":20,"y":20},{"x":100,"y":20},{"x":100,"y":100},{"x":20,"y":100}]), true);
    s.execute("object.move", &json!({"dx":10,"dy":0})).unwrap();
    s.execute("edit.undo", &json!({})).unwrap();
    pick(&mut s, json!([{"id":a.0,"anchors":[[0,0],[0,2]]}]));
    s.doc_mut().unwrap().mark_saved();
    let before = s.doc().unwrap().doc.clone();
    let sel = s.doc().unwrap().selection.clone();
    let history = format!("{:?}", s.doc().unwrap().history);
    assert!(s.execute("path.averageJoin", &json!({"kind":"corner"})).is_err());
    assert_eq!(s.doc().unwrap().doc, before);
    assert_eq!(s.doc().unwrap().selection, sel);
    assert_eq!(format!("{:?}", s.doc().unwrap().history), history);
    assert!(!s.doc().unwrap().is_dirty());
}

#[test]
fn average_join_degenerate_smooth_rejects_before_averaging() {
    let mut s = session();
    let a = make(&mut s, json!([{"x":20,"y":20},{"x":100,"y":20},{"x":100,"y":100}]), false);
    pick(&mut s, json!([{"id":a.0,"anchors":[[0,0],[0,2]]}]));
    let before = s.doc().unwrap().doc.clone();
    let undo = s.doc().unwrap().history.undo.len();
    assert!(s.execute("path.averageJoin", &json!({"kind":"smooth"})).is_err());
    assert_eq!(s.doc().unwrap().doc, before);
    assert_eq!(s.doc().unwrap().history.undo.len(), undo);
}

#[test]
fn average_join_coincident_smooth_inputs_keep_independent_handles_for_corner() {
    let mut s = session();
    let first = json!([{"x":20,"y":100},{"x":100,"y":100,"in":[80,60],"out":[120,140],"smooth":true}]);
    let a = make(&mut s, first.clone(), false);
    let b = make(&mut s, json!([{"x":100,"y":100,"in":[70,90],"out":[130,110],"smooth":true},{"x":240,"y":100}]), false);
    // Identical geometry in a different, unselected object must not become the join target.
    let duplicate = make(&mut s, first, false);
    let duplicate_before = s.doc().unwrap().doc.node(duplicate).unwrap().clone();
    assert_ne!(a, duplicate);
    assert_eq!(path(&s, a).subpaths[0].anchors[1].kind, AnchorKind::Smooth);
    assert_eq!(path(&s, b).subpaths[0].anchors[0].kind, AnchorKind::Smooth);
    pick(&mut s, json!([{"id":a.0,"anchors":[[0,1]]},{"id":b.0,"anchors":[[0,0]]}]));
    s.execute("path.averageJoin", &json!({"kind":"corner"})).unwrap();
    let p = path(&s, a);
    let join = p.subpaths[0].anchors[1];
    assert_eq!(join.p, Point::new(100.0, 100.0));
    assert_eq!(join.h_in, Point::new(80.0, 60.0));
    assert_eq!(join.h_out, Point::new(130.0, 110.0));
    assert_eq!(join.kind, AnchorKind::Corner);
    assert_eq!(s.doc().unwrap().doc.node(duplicate).unwrap(), &duplicate_before);
    assert!(s.doc().unwrap().doc.node(b).is_none());
    assert_eq!(s.doc().unwrap().selection.objects, vec![a]);
}

#[test]
fn average_join_uses_picked_far_ends_after_document_space_transforms() {
    for (command, params) in [
        ("object.move", json!({"dx":30,"dy":50})),
        ("object.rotate", json!({"angle":37,"origin":[0,0]})),
        ("object.scale", json!({"sx":170,"sy":60,"origin":[0,0]})),
        ("object.reflect", json!({"axis":"vertical","origin":[0,0]})),
    ] {
        let mut s = session();
        let (a, b) = pair(&mut s);
        s.execute("select.set", &json!({"ids":[a.0,b.0]})).unwrap();
        s.execute(command, &params).unwrap();
        let (pa, pb) = (path(&s, a), path(&s, b));
        let (a0, b1) = (pa.subpaths[0].anchors[0], pb.subpaths[0].anchors[1]);
        let center = Point::new((a0.p.x + b1.p.x) / 2.0, (a0.p.y + b1.p.y) / 2.0);
        pick(&mut s, json!([{"id":a.0,"anchors":[[0,0]]},{"id":b.0,"anchors":[[0,1]]}]));
        s.execute("path.averageJoin", &json!({"kind":"corner"})).unwrap();
        let p = path(&s, a);
        let join = p.subpaths[0].anchors[1];
        assert!(join.p.distance(center) < 1e-9, "{command}");
        assert!(join.h_in.distance(a0.h_out + (center - a0.p)) < 1e-9, "{command}");
        assert!(join.h_out.distance(b1.h_in + (center - b1.p)) < 1e-9, "{command}");
        assert_eq!(p.subpaths[0].anchors[0].p, pa.subpaths[0].anchors[1].p);
        assert_eq!(p.subpaths[0].anchors[2].p, pb.subpaths[0].anchors[0].p);
    }
}

#[test]
fn average_join_rejects_empty_anchor_member_after_last_anchor_toggle() {
    let mut s = session();
    s.select_tool("directSelection", Default::default()).unwrap();
    let (a, b) = pair(&mut s);
    let c = make(&mut s, json!([{"x":320,"y":20},{"x":380,"y":60}]), false);
    // Keep a real Redo branch while normal selection commands produce the extra empty member.
    s.execute("object.move", &json!({"dx":4,"dy":3})).unwrap();
    s.execute("edit.undo", &json!({})).unwrap();
    pick(
        &mut s,
        json!([
            {"id":a.0,"anchors":[[0,1]]},
            {"id":b.0,"anchors":[[0,0]]},
            {"id":c.0,"anchors":[[0,0]]}
        ]),
    );
    s.execute("select.anchors", &json!({"id":c.0,"anchors":[[0,0]],"mode":"toggle"})).unwrap();
    let selection = &s.doc().unwrap().selection;
    assert_eq!(selection.objects, vec![a, b, c]);
    assert_eq!(selection.anchors[&a].len(), 1);
    assert_eq!(selection.anchors[&b].len(), 1);
    assert!(selection.anchors[&c].is_empty(), "last-anchor toggle left the ordinary empty entry");
    assert!(selection.segments.is_empty());
    // The predicate is shared by command execution and menu enablement.
    let command = vectorcraft_engine::find_command("path.averageJoin").unwrap();
    assert!((command.enabled)(&s).is_err(), "an empty anchor member must not be silently ignored");
    s.doc_mut().unwrap().mark_saved();
    let before = s.doc().unwrap().doc.clone();
    let before_selection = s.doc().unwrap().selection.clone();
    let history = |s: &Session| {
        let entries = |items: &[vectorcraft_engine::HistoryEntry]| {
            items.iter().map(|e| (e.label.clone(), e.doc.clone(), e.selection.clone())).collect::<Vec<_>>()
        };
        let h = &s.doc().unwrap().history;
        (entries(&h.undo), entries(&h.redo))
    };
    let before_history = history(&s);
    assert!(!before_history.1.is_empty(), "selection commands retained the Redo branch");
    for kind in ["corner", "smooth"] {
        assert!(s.execute("path.averageJoin", &json!({"kind":kind})).is_err());
        assert_eq!(s.doc().unwrap().doc, before);
        assert_eq!(s.doc().unwrap().selection, before_selection);
        assert_eq!(history(&s), before_history);
        assert!(!s.doc().unwrap().is_dirty());
    }
    // Empty segment selection clears normally, rather than leaving a stale segment-map entry.
    s.execute("select.segments", &json!({"id":c.0,"segments":[]})).unwrap();
    let selection = &s.doc().unwrap().selection;
    assert!(selection.objects.is_empty() && selection.anchors.is_empty() && selection.segments.is_empty());
    pick(&mut s, json!([{"id":a.0,"anchors":[[0,1]]},{"id":b.0,"anchors":[[0,0]]}]));
    assert!((command.enabled)(&s).is_ok(), "a clean pair is still eligible");
    assert_eq!(s.doc().unwrap().doc, before);
    assert_eq!(history(&s), before_history);
    assert!(!s.doc().unwrap().is_dirty());
}
