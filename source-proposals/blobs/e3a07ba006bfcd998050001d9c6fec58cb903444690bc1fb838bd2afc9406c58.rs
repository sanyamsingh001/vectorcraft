fn join_takes_the_paths_and_the_ends_to_join() {
    let mut s = session();
    let a = line(&mut s, 0.0, 0.0, 100.0, 0.0);
    let b = line(&mut s, 110.0, 0.0, 200.0, 0.0);
    // The far ends: a's first to b's last (the nearest pair would be a's last and b's first).
    s.execute("path.join", &json!({"ids": [a.0, b.0], "ends": ["first", "last"]})).unwrap();
    let pts: Vec<(f64, f64)> = path(&s, a).subpaths[0].anchors.iter().map(|x| (x.p.x, x.p.y)).collect();
    assert_eq!(pts, [(100.0, 0.0), (0.0, 0.0), (200.0, 0.0), (110.0, 0.0)]);
    let r = rect(&mut s, 0.0, 50.0, 10.0, 10.0);
    for bad in [
        json!({"ids": [a.0, a.0]}),
        json!({"ids": [a.0, 999]}),
        json!({"ids": [a.0, r.0], "ends": ["first"]}),
        json!({"ids": [a.0, b.0], "ends": ["first", "middle"]}),
    ] {
        assert!(s.execute("path.join", &bad).is_err(), "{bad}");
    }
}
