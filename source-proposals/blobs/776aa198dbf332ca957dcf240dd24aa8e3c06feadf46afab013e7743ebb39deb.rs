fn join_merges_the_selected_anchors_average_brought_together() {
    let mut s = session();
    let r = rect(&mut s, 100.0, 100.0, 100.0, 80.0);
    let pts = |s: &Session| path(s, r).subpaths[0].anchors.iter().map(|x| (x.p.x, x.p.y)).collect::<Vec<_>>();
    s.execute("select.anchors", &json!({"id": r.0, "anchors": [[0, 0], [0, 1]], "mode": "set"})).unwrap();
    assert!(s.execute("path.join", &json!({})).is_err(), "apart: nothing to merge, and the path is closed");
    s.execute("path.average", &json!({"axis": "both"})).unwrap();
    s.execute("path.join", &json!({})).unwrap();
    assert_eq!(pts(&s), [(150.0, 100.0), (200.0, 180.0), (100.0, 180.0)]);
    assert!(path(&s, r).subpaths[0].closed);
    let sel = &s.doc().unwrap().selection;
    assert!(sel.objects == [r] && sel.anchors.is_empty());
    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(pts(&s).len(), 4);
    // The closing pair: the first and last anchors of a closed path.
    s.execute("select.anchors", &json!({"id": r.0, "anchors": [[0, 0], [0, 3]], "mode": "set"})).unwrap();
    s.execute("path.average", &json!({"axis": "both"})).unwrap();
    let before = pts(&s);
    s.execute("path.join", &json!({})).unwrap();
    assert_eq!(pts(&s), before[..3]);
}

/// `path.join {ids, ends}` joins the ends asked for, not the nearest pair; bad requests are errors.