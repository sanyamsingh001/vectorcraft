//! #541: key-object clicks reach the engine without changing the selection or drag semantics.

use super::*;
use serde_json::json;
use vectorcraft_geom::{Point, Rect, Vec2};
use vectorcraft_tools::{Mods, PointerEvent, PointerKind};

fn setup() -> (Session, [NodeId; 3]) {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 600, "height": 600})).unwrap();
    let ids = [(100, 100, 60, 30), (240, 240, 80, 50), (400, 400, 40, 70)].map(|(x, y, width, height)| {
        NodeId(s.execute("shape.rectangle", &json!({ "x": x, "y": y, "width": width, "height": height })).unwrap()["id"].as_u64().unwrap())
    });
    s.execute("select.set", &json!({"ids": ids.iter().map(|id| id.0).collect::<Vec<_>>()})).unwrap();
    (s, ids)
}

fn bounds(s: &Session, id: NodeId) -> Rect {
    s.active().unwrap().doc.node(id).unwrap().geometric_bounds().unwrap()
}

fn pointer(s: &mut Session, kind: PointerKind, p: Point, mods: Mods) {
    let view = ViewInfo { smart_guides: false, snap_to_point: false, ..Default::default() };
    s.pointer(&PointerEvent::new(kind, p.x, p.y).with_mods(mods), view).unwrap();
}

fn click(s: &mut Session, p: Point) {
    pointer(s, PointerKind::Down, p, Mods::default());
    pointer(s, PointerKind::Up, p, Mods::default());
}

#[test]
fn key_object_click_release_aligns_the_others_and_reclick_or_cancel_clears() {
    let (mut s, ids) = setup();
    let key = ids[1];
    let original = ids.map(|id| bounds(&s, id));
    let undo = s.active().unwrap().history.undo.len();
    pointer(&mut s, PointerKind::Down, original[1].center(), Mods::default());
    assert_eq!(s.active().unwrap().selection.key, None, "not until release");
    pointer(&mut s, PointerKind::Up, original[1].center(), Mods::default());
    assert_eq!(s.active().unwrap().selection.key, Some(key));
    assert_eq!(s.active().unwrap().selection.objects, ids);
    assert_eq!(s.active().unwrap().history.undo.len(), undo, "designation is not an art edit");
    s.execute("object.align", &json!({"horizontal": "left", "to": "key"})).unwrap();
    assert_eq!(bounds(&s, key), original[1]);
    for id in [ids[0], ids[2]] {
        assert_eq!(bounds(&s, id).x0, original[1].x0);
    }
    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(ids.map(|id| bounds(&s, id)), original);
    assert_eq!(s.active().unwrap().selection.key, Some(key));
    click(&mut s, original[1].center());
    assert_eq!(s.active().unwrap().selection.key, None);
    click(&mut s, original[2].center());
    assert_eq!(s.active().unwrap().selection.key, Some(ids[2]));
    s.execute("select.key", &json!({})).unwrap();
    assert_eq!(s.active().unwrap().selection.key, None);
    assert_eq!(s.active().unwrap().selection.objects, ids);
    click(&mut s, original[0].center());
    s.execute("select.toggle", &json!({"id": ids[0].0})).unwrap();
    assert_eq!(s.active().unwrap().selection.key, None, "removing the key clears it");
}

#[test]
fn key_object_spacing_anchors_first_middle_and_last_on_both_axes() {
    for axis in ["horizontal", "vertical"] {
        for key_index in 0..3 {
            let (mut s, ids) = setup();
            let original = ids.map(|id| bounds(&s, id));
            // Designate through the actual tool, rather than setting state in the test.
            click(&mut s, original[key_index].center());
            let key = ids[key_index];
            assert_eq!(s.active().unwrap().selection.key, Some(key));
            let undo = s.active().unwrap().history.undo.len();
            s.execute("object.distributeSpacing", &json!({"axis": axis, "spacing": 17})).unwrap();
            assert_eq!(bounds(&s, key), original[key_index], "{axis}: key stays fixed");
            let after = ids.map(|id| bounds(&s, id));
            for pair in after.windows(2) {
                let gap = if axis == "horizontal" { pair[1].x0 - pair[0].x1 } else { pair[1].y0 - pair[0].y1 };
                assert!((gap - 17.0).abs() < 1e-9, "{axis}: {gap}");
            }
            assert_eq!(s.active().unwrap().selection.objects, ids);
            assert_eq!(s.active().unwrap().history.undo.len(), undo + 1);
            s.execute("edit.undo", &json!({})).unwrap();
            assert_eq!(ids.map(|id| bounds(&s, id)), original);
            s.execute("edit.redo", &json!({})).unwrap();
            assert_eq!(ids.map(|id| bounds(&s, id)), after);
        }
    }
}

#[test]
fn key_object_drag_and_shift_click_keep_existing_semantics() {
    for mods in [Mods::default(), Mods { shift: true, ..Default::default() }, Mods { alt: true, ..Default::default() }] {
        let (mut s, ids) = setup();
        let original = ids.map(|id| bounds(&s, id));
        click(&mut s, original[1].center());
        let undo = s.active().unwrap().history.undo.len();
        let from = original[0].center();
        pointer(&mut s, PointerKind::Down, from, mods);
        pointer(&mut s, PointerKind::Drag, from + Vec2::new(20.0, 5.0), mods);
        pointer(&mut s, PointerKind::Up, from + Vec2::new(20.0, 5.0), mods);
        let st = s.active().unwrap();
        assert_eq!(st.history.undo.len(), undo + 1);
        assert_eq!(st.history.undo.last().unwrap().label, if mods.alt { "Copy" } else { "Move" });
        if mods.alt {
            assert_eq!(ids.map(|id| bounds(&s, id)), original, "Alt leaves the originals");
            assert!(st.selection.objects.iter().all(|id| !ids.contains(id)), "copies selected");
        } else {
            assert_eq!(st.selection.key, Some(ids[1]), "drag never switches the key");
            for (id, before) in ids.into_iter().zip(original) {
                let after = bounds(&s, id);
                assert_eq!(after.x0 - before.x0, 20.0);
                assert_eq!(after.y0 - before.y0, if mods.shift { 0.0 } else { 5.0 });
            }
        }
        s.execute("edit.undo", &json!({})).unwrap();
        assert_eq!(ids.map(|id| bounds(&s, id)), original);
    }
    let (mut s, ids) = setup();
    let p = bounds(&s, ids[1]).center();
    click(&mut s, p);
    let shift = Mods { shift: true, ..Default::default() };
    pointer(&mut s, PointerKind::Down, p, shift);
    assert_eq!(s.active().unwrap().selection.key, Some(ids[1]));
    pointer(&mut s, PointerKind::Up, p, shift);
    assert_eq!(s.active().unwrap().selection.objects, [ids[0], ids[2]]);
    assert_eq!(s.active().unwrap().selection.key, None);
}

#[test]
fn key_object_hit_testing_uses_the_group_or_the_isolated_member() {
    let (mut s, ids) = setup();
    s.execute("select.set", &json!({"ids": [ids[0].0, ids[1].0]})).unwrap();
    let group = NodeId(s.execute("object.group", &json!({})).unwrap()["id"].as_u64().unwrap());
    s.execute("select.set", &json!({"ids": [group.0, ids[2].0]})).unwrap();
    let p = bounds(&s, ids[0]).center();
    click(&mut s, p);
    assert_eq!(s.active().unwrap().selection.key, Some(group));
    s.execute("object.isolate", &json!({"id": group.0})).unwrap();
    s.execute("select.set", &json!({"ids": [ids[0].0, ids[1].0]})).unwrap();
    click(&mut s, p);
    assert_eq!(s.active().unwrap().selection.key, Some(ids[0]));
}

#[test]
fn key_object_spacing_without_an_explicit_value_preserves_the_first_object() {
    for spacing in [json!(null), json!(17)] {
        let (mut s, ids) = setup();
        let original = ids.map(|id| bounds(&s, id));
        if spacing.is_null() {
            click(&mut s, original[1].center());
        }
        s.execute("object.distributeSpacing", &json!({"axis": "horizontal", "spacing": spacing})).unwrap();
        assert_eq!(bounds(&s, ids[0]), original[0]);
        let after = ids.map(|id| bounds(&s, id));
        assert_eq!(after[1].x0 - after[0].x1, after[2].x0 - after[1].x1);
    }
}

#[test]
fn key_object_reclick_clears_after_shift_click_leaves_only_the_key() {
    let (mut s, ids) = setup();
    s.execute("select.set", &json!({"ids": [ids[0].0, ids[1].0]})).unwrap();
    let key_point = bounds(&s, ids[0]).center();
    click(&mut s, key_point);
    let shift = Mods { shift: true, ..Default::default() };
    let other_point = bounds(&s, ids[1]).center();
    pointer(&mut s, PointerKind::Down, other_point, shift);
    pointer(&mut s, PointerKind::Up, other_point, shift);
    assert_eq!(s.active().unwrap().selection.objects, [ids[0]]);
    assert_eq!(s.active().unwrap().selection.key, Some(ids[0]));
    let doc = s.active().unwrap().doc.clone();
    let undo = s.active().unwrap().history.undo.len();
    click(&mut s, key_point);
    let st = s.active().unwrap();
    assert_eq!(st.selection.key, None, "re-click still cancels the remaining key");
    assert_eq!(st.selection.objects, [ids[0]]);
    assert_eq!(st.history.undo.len(), undo);
    assert!(std::sync::Arc::ptr_eq(&st.doc, &doc), "designation never changes the artwork");
}

#[test]
fn key_object_spacing_between_two_objects_measures_different_strokes() {
    for axis in ["horizontal", "vertical"] {
        for key_index in 0..2 {
            let (mut s, ids) = setup();
            for (id, weight) in [(ids[0], 10), (ids[1], 20)] {
                s.execute("select.set", &json!({"ids": [id.0]})).unwrap();
                s.execute("stroke.set", &json!({"weight": weight})).unwrap();
            }
            s.execute("prefs.set", &json!({"key": "usePreviewBounds", "value": true})).unwrap();
            s.execute("select.set", &json!({"ids": [ids[0].0, ids[1].0]})).unwrap();
            let original = ids.map(|id| bounds(&s, id));
            click(&mut s, original[key_index].center());
            assert_eq!(s.active().unwrap().selection.key, Some(ids[key_index]));
            s.execute("object.distributeSpacing", &json!({"axis": axis, "spacing": 17})).unwrap();
            assert_eq!(bounds(&s, ids[key_index]), original[key_index]);
            let doc = &s.active().unwrap().doc;
            let a = doc.bounds_of(&[ids[0]], true).unwrap();
            let b = doc.bounds_of(&[ids[1]], true).unwrap();
            let gap = if axis == "horizontal" { b.x0 - a.x1 } else { b.y0 - a.y1 };
            assert!((gap - 17.0).abs() < 1e-9, "{axis}: visible gap {gap}");
            let after = ids.map(|id| bounds(&s, id));
            assert_eq!(after[2], original[2], "unselected art stays put");
            s.execute("edit.undo", &json!({})).unwrap();
            assert_eq!(ids.map(|id| bounds(&s, id)), original);
            s.execute("edit.redo", &json!({})).unwrap();
            assert_eq!(ids.map(|id| bounds(&s, id)), after);
        }
    }
}
