//! Individual ruler-guide controls: original small documents, public commands and tool gestures.
//! Runtime and exact-source fullCI must run before publication.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use serde_json::{Value, json};
use vectorcraft_doc::{Document, Guide, Selection};
use vectorcraft_engine::{Session, ViewInfo};
use vectorcraft_geom::{Point, Rect, Vec2};
use vectorcraft_testkit::fixtures::{self, exec};
use vectorcraft_tools::{PointerEvent, PointerKind};

const FLAGS: [(bool, bool); 4] = [(true, false), (true, true), (false, false), (false, true)];

fn doc(s: &Session) -> &Document {
    &s.doc().unwrap().doc
}

fn session() -> Session {
    fixtures::session_with(400.0, 300.0)
}

fn add(s: &mut Session, vertical: bool, pos: f64) -> usize {
    exec(s, "guide.add", json!({"vertical": vertical, "pos": pos}))["index"].as_u64().unwrap() as usize
}

fn props(s: &mut Session, index: usize, visible: bool, locked: bool) -> Value {
    exec(s, "guide.setProps", json!({"index": index, "visible": visible, "locked": locked}))
}

fn snapshot(s: &Session) -> (Value, Selection, usize, usize) {
    let st = s.doc().unwrap();
    (serde_json::to_value(doc(s)).unwrap(), st.selection.clone(), st.history.undo.len(), st.history.redo.len())
}

fn rejects_without_changes(s: &mut Session, command: &str, params: Value) {
    let before = snapshot(s);
    assert!(s.execute(command, &params).is_err(), "{command} {params} must be refused");
    assert_eq!(snapshot(s), before, "{command} must preserve document, selection, and undo/redo on error");
}

fn pointer(s: &mut Session, kind: PointerKind, x: f64, y: f64, view: ViewInfo) {
    s.pointer(&PointerEvent::new(kind, x, y), view).unwrap();
}

#[test]
fn legacy_guides_default_to_visible_unlocked_and_flags_roundtrip_natively() {
    let legacy: Guide = serde_json::from_value(json!({"vertical": true, "pos": 40.0})).unwrap();
    assert_eq!(legacy, Guide::new(true, 40.0));
    assert!(legacy.visible && !legacy.locked && legacy.is_editable());
    assert!(Guide::default().visible && !Guide::default().locked);
    assert_eq!(serde_json::to_value(&legacy).unwrap(), json!({"vertical": true, "pos": 40.0}));
    let mut d = Document::new(100.0, 100.0);
    for (i, (visible, locked)) in FLAGS.into_iter().enumerate() {
        let g = Guide { visible, locked, artboard: Some(d.artboards[0].id), ..Guide::new(i % 2 == 0, 20.0 + i as f64 * 10.0) };
        let saved = serde_json::to_value(&g).unwrap();
        assert_eq!(saved.get("visible"), if visible { None } else { Some(&Value::Bool(false)) });
        assert_eq!(saved.get("locked"), if locked { Some(&Value::Bool(true)) } else { None });
        assert_eq!(serde_json::from_value::<Guide>(saved).unwrap(), g);
        d.guides.push(g);
    }
    let back = vectorcraft_format::load(&vectorcraft_format::save_file(&d)).unwrap();
    assert_eq!(back.guides, d.guides, "native persistence retains flags and board ownership");
    assert!(serde_json::from_value::<Guide>(json!({"vertical": true, "pos": 40, "visible": "false"})).is_err());
}

#[test]
fn hiding_or_locking_prunes_selection_and_undo_redo_restores_it() {
    for flags in [json!({"visible": false}), json!({"locked": true}), json!({"visible": false, "locked": true})] {
        let mut s = session();
        add(&mut s, true, 40.0);
        add(&mut s, false, 60.0);
        exec(&mut s, "guide.select", json!({"indexes": [0, 1]}));
        let before = doc(&s).guides.clone();
        let undo = s.doc().unwrap().history.undo.len();
        let mut params = flags.clone();
        params["index"] = json!(0);
        let out = exec(&mut s, "guide.setProps", params);
        assert_eq!(
            out,
            json!({"index": 0, "visible": flags["visible"].as_bool().unwrap_or(true), "locked": flags["locked"].as_bool().unwrap_or(false)})
        );
        assert_eq!(s.doc().unwrap().selection.guides, [1]);
        assert_eq!(doc(&s).guides[1], before[1]);
        assert!(!doc(&s).guides[0].is_editable());
        assert_eq!(s.doc().unwrap().history.undo.len(), undo + 1);
        assert_eq!(s.doc().unwrap().history.undo.last().unwrap().label, "Guide Properties");
        exec(&mut s, "edit.undo", json!({}));
        assert_eq!(doc(&s).guides, before);
        assert_eq!(s.doc().unwrap().selection.guides, [0, 1]);
        exec(&mut s, "edit.redo", json!({}));
        assert!(!doc(&s).guides[0].is_editable());
        assert_eq!(s.doc().unwrap().selection.guides, [1]);
        props(&mut s, 0, true, false);
        assert_eq!(s.doc().unwrap().selection.guides, [1], "revealing/unlocking does not select the guide");
    }
}

#[test]
fn properties_keep_selected_artwork_and_equal_values_add_no_undo_step() {
    let mut s = session();
    add(&mut s, true, 40.0);
    let art = fixtures::rect(&mut s, 200.0, 200.0, 20.0, 10.0);
    assert_eq!(s.doc().unwrap().selection.objects, [art]);
    let out = props(&mut s, 0, false, true);
    assert_eq!(out, json!({"index": 0, "visible": false, "locked": true}));
    assert_eq!(s.doc().unwrap().selection.objects, [art]);
    let before = snapshot(&s);
    assert_eq!(props(&mut s, 0, false, true), out);
    assert_eq!(snapshot(&s), before);
    let list = exec(&mut s, "guide.list", json!({}));
    assert_eq!(list[0]["visible"], false);
    assert_eq!(list[0]["locked"], true);
    props(&mut s, 0, true, false);
    let list = exec(&mut s, "guide.list", json!({}));
    assert_eq!(list[0], json!({"index": 0, "vertical": true, "pos": 40.0, "selected": false}), "default rows keep their old query shape");
}

#[test]
fn invalid_properties_are_rejected_before_any_partial_change() {
    let mut s = session();
    add(&mut s, true, 40.0);
    exec(&mut s, "guide.select", json!({"indexes": [0]}));
    for params in [
        json!({"index": 0}),
        json!({"visible": false}),
        json!({"index": -1, "locked": true}),
        json!({"index": 1, "visible": false}),
        json!({"index": 0.5, "locked": true}),
        json!({"index": "0", "locked": true}),
        json!({"index": 0, "visible": 0}),
        json!({"index": 0, "locked": "yes"}),
        json!({"index": 0, "visible": false, "locked": null}),
    ] {
        rejects_without_changes(&mut s, "guide.setProps", params);
    }
}

#[test]
fn protected_guides_reject_explicit_edits_and_stale_selected_batches_atomically() {
    for (visible, locked) in [(false, false), (true, true)] {
        let mut s = session();
        add(&mut s, true, 40.0);
        add(&mut s, false, 90.0);
        props(&mut s, 1, visible, locked);
        let art = fixtures::rect(&mut s, 200.0, 200.0, 20.0, 10.0);
        assert_eq!(s.doc().unwrap().selection.objects, [art], "positive control: failed picks must not deselect art");
        for params in [json!({"indexes": [0, 1]}), json!({"indexes": [1], "toggle": true})] {
            rejects_without_changes(&mut s, "guide.select", params);
        }
        rejects_without_changes(&mut s, "guide.move", json!({"index": 1, "pos": 120}));
        rejects_without_changes(&mut s, "guide.remove", json!({"index": 1}));
        // Simulate a stale selection arriving from another document mutation or history state.
        s.doc_mut().unwrap().selection.set_guides([0, 1]);
        for (command, params) in [
            ("guide.move", json!({"dx": 5, "dy": 7})),
            ("guide.move", json!({"dx": 5, "dy": 7, "copy": true})),
            ("guide.remove", json!({})),
            ("object.move", json!({"dx": 5, "dy": 7})),
            ("object.nudge", json!({"dx": 1, "dy": 0})),
            ("edit.clear", json!({})),
        ] {
            rejects_without_changes(&mut s, command, params);
        }
        assert_eq!(doc(&s).guides[0].pos, 40.0, "the editable member of a rejected batch must not move");
        assert_eq!(doc(&s).guides.len(), 2);
    }
}

#[test]
fn global_lock_stays_authoritative_after_individual_unlock_and_on_delegated_edits() {
    let mut s = session();
    add(&mut s, true, 40.0);
    props(&mut s, 0, true, true);
    exec(&mut s, "view.guides.lock", json!({"locked": true}));
    assert_eq!(props(&mut s, 0, true, false), json!({"index": 0, "visible": true, "locked": false}));
    assert!(s.guides_locked(), "individual properties never unlock the global setting");
    for (command, params) in [
        ("guide.select", json!({"indexes": [0]})),
        ("guide.move", json!({"index": 0, "pos": 50})),
        ("guide.remove", json!({"index": 0})),
        ("view.guides.clear", json!({})),
    ] {
        rejects_without_changes(&mut s, command, params);
    }
    s.doc_mut().unwrap().selection.set_guides([0]);
    for (command, params) in [("object.move", json!({"dx": 5})), ("object.nudge", json!({"dx": 1})), ("edit.clear", json!({}))] {
        rejects_without_changes(&mut s, command, params);
    }
    exec(&mut s, "guide.setProps", json!({"index": 0, "locked": false}));
    assert!(s.doc().unwrap().selection.guides.is_empty(), "the setter also prunes stale selection under global lock");
    exec(&mut s, "view.guides.lock", json!({"locked": false}));
    exec(&mut s, "guide.select", json!({"indexes": [0]}));
    assert_eq!(s.doc().unwrap().selection.guides, [0], "positive control: global unlock restores normal picking");
}

#[test]
fn other_document_mutations_prune_protected_and_missing_guide_indexes_only() {
    let mut s = session();
    for pos in [40.0, 60.0, 80.0, 100.0] {
        add(&mut s, true, pos);
    }
    let art = fixtures::rect(&mut s, 200.0, 200.0, 20.0, 10.0);
    s.doc_mut().unwrap().selection.guides = vec![0, 1, 2, 3, 99];
    s.edit("Other Document Mutation", |d, _| {
        d.guides[1].locked = true;
        d.guides[2].visible = false;
        d.guides[3].visible = false;
        d.guides[3].locked = true;
        Ok(())
    })
    .unwrap();
    assert_eq!(s.doc().unwrap().selection.guides, [0]);
    assert_eq!(s.doc().unwrap().selection.objects, [art]);
    let mut sel = Selection::default();
    sel.set_guides([0, 1, 2, 3, 99]);
    sel.prune(doc(&s));
    assert_eq!(sel.guides, [0]);
}

#[test]
fn hidden_guides_have_no_reachable_span_but_locked_guides_keep_their_geometry() {
    let d = Document::new(100.0, 100.0);
    let mut g = Guide { artboard: Some(d.artboards[0].id), ..Guide::new(true, 40.0) };
    assert_eq!(d.guide_span(&g), Some((0.0, 100.0)));
    assert!(d.guide_passes(&g, Point::new(40.0, 50.0), 0.0));
    assert!(!d.guide_passes(&g, Point::new(40.0, 150.0), 0.0));
    g.locked = true;
    assert!(d.guide_passes(&g, Point::new(40.0, 50.0), 0.0));
    assert!(!g.is_editable());
    g.visible = false;
    assert_eq!(d.guide_span(&g), Some((0.0, 100.0)), "span remains geometry, not a visibility sentinel");
    assert!(!d.guide_passes(&g, Point::new(40.0, 50.0), 0.0));
    g.artboard = None;
    assert_eq!(d.guide_span(&g), None);
    assert!(!d.guide_passes(&g, Point::new(40.0, 150.0), 0.0), "a hidden canvas guide does not pass any point");
}

#[test]
fn guide_flags_follow_artboard_move_duplicate_and_guide_copy() {
    let mut s = session();
    for (i, (visible, locked)) in FLAGS.into_iter().enumerate() {
        let result = exec(&mut s, "guide.add", json!({"vertical": i % 2 == 0, "pos": 40.0 + i as f64 * 20.0, "artboard": 0}));
        props(&mut s, result["index"].as_u64().unwrap() as usize, visible, locked);
    }
    let canvas = add(&mut s, true, 170.0);
    props(&mut s, canvas, false, true);
    let before = doc(&s).guides.clone();
    let delta = Vec2::new(20.0, 10.0);
    exec(&mut s, "artboard.move", json!({"index": 0, "dx": delta.x, "dy": delta.y}));
    for (i, g) in before.iter().take(4).enumerate() {
        assert_eq!(doc(&s).guides[i], g.moved(delta), "board movement retains both flags");
    }
    assert_eq!(doc(&s).guides[canvas], before[canvas], "canvas guides stay put");
    let source = doc(&s).artboards[0].clone();
    let originals = doc(&s).guides.clone();
    let index = exec(&mut s, "artboard.duplicate", json!({"index": 0}))["index"].as_u64().unwrap() as usize;
    let copied = doc(&s).artboards[index].clone();
    let delta = copied.rect.origin() - source.rect.origin();
    assert_eq!(doc(&s).guides.len(), originals.len() + 4);
    for (i, g) in originals.iter().take(4).enumerate() {
        assert_eq!(doc(&s).guides[originals.len() + i], Guide { artboard: Some(copied.id), ..g.moved(delta) });
    }
    let first_board_copy = doc(&s).guides.len();
    let board_delta = Vec2::new(-30.0, 330.0);
    let copied_index = exec(&mut s, "artboard.move", json!({"index": 0, "dx": board_delta.x, "dy": board_delta.y, "copy": true}))["index"]
        .as_u64()
        .unwrap() as usize;
    let copied_id = doc(&s).artboards[copied_index].id;
    assert_eq!(doc(&s).guides.len(), first_board_copy + 4);
    for (i, g) in originals.iter().take(4).enumerate() {
        assert_eq!(doc(&s).guides[first_board_copy + i], Guide { artboard: Some(copied_id), ..g.moved(board_delta) });
    }
    exec(&mut s, "guide.select", json!({"indexes": [0]}));
    let first = doc(&s).guides.len();
    let expected = doc(&s).guides[0].moved(Vec2::new(7.0, 11.0));
    exec(&mut s, "guide.move", json!({"dx": 7, "dy": 11, "copy": true}));
    assert_eq!(doc(&s).guides[first], expected);
    assert_eq!(s.doc().unwrap().selection.guides, [first]);
    assert_eq!(vectorcraft_format::load(&vectorcraft_format::save_file(doc(&s))).unwrap().guides, doc(&s).guides);
}

#[test]
fn selection_tool_picks_only_visible_unlocked_guides_under_global_policy() {
    for (visible, locked) in FLAGS {
        for (shown, global_locked) in [(true, false), (false, false), (true, true)] {
            let mut s = session();
            add(&mut s, true, 40.0);
            props(&mut s, 0, visible, locked);
            exec(&mut s, "view.guides.lock", json!({"locked": global_locked}));
            let view = ViewInfo { smart_guides: false, guides: shown, ..ViewInfo::default() };
            s.select_tool("selection", view).unwrap();
            let before = doc(&s).guides.clone();
            pointer(&mut s, PointerKind::Down, 40.0, 20.0, view);
            pointer(&mut s, PointerKind::Up, 40.0, 20.0, view);
            let expected = if visible && !locked && shown && !global_locked { vec![0] } else { vec![] };
            assert_eq!(
                s.doc().unwrap().selection.guides,
                expected,
                "flags: visible={visible}, locked={locked}; global: shown={shown}, locked={global_locked}"
            );
            assert_eq!(doc(&s).guides, before, "a click must not move the guide");
        }
    }
}

#[test]
fn drawing_snaps_to_visible_locked_guides_but_never_to_hidden_guides() {
    for (visible, locked) in FLAGS {
        for (shown, global_locked) in [(true, false), (false, false), (true, true)] {
            let mut s = session();
            add(&mut s, true, 40.0);
            props(&mut s, 0, visible, locked);
            exec(&mut s, "view.guides.lock", json!({"locked": global_locked}));
            let view = ViewInfo { smart_guides: false, guides: shown, ..ViewInfo::default() };
            s.select_tool("rectangle", view).unwrap();
            pointer(&mut s, PointerKind::Down, 39.0, 20.0, view);
            pointer(&mut s, PointerKind::Drag, 90.0, 50.0, view);
            pointer(&mut s, PointerKind::Up, 90.0, 50.0, view);
            let selected = &s.doc().unwrap().selection.objects;
            assert_eq!(selected.len(), 1, "positive control: the rectangle was created");
            let bounds = doc(&s).node(selected[0]).unwrap().geometric_bounds().unwrap();
            let x = if visible && shown && !global_locked { 40.0 } else { 39.0 };
            assert_eq!(bounds, Rect::new(x, 20.0, 90.0, 50.0), "individual locks preserve snapping; global policy still gates it");
        }
    }
}

#[test]
fn clear_guides_preserves_protected_rulers_and_is_undoable() {
    let mut s = session();
    for (i, (visible, locked)) in FLAGS.into_iter().enumerate() {
        add(&mut s, true, 40.0 + i as f64 * 20.0);
        props(&mut s, i, visible, locked);
    }
    exec(&mut s, "guide.select", json!({"indexes": [0]}));
    let before = doc(&s).guides.clone();
    let undo = s.doc().unwrap().history.undo.len();
    assert_eq!(exec(&mut s, "view.guides.clear", json!({}))["count"], 1);
    assert_eq!(doc(&s).guides, before[1..]);
    assert!(s.doc().unwrap().selection.guides.is_empty());
    assert_eq!(s.doc().unwrap().history.undo.len(), undo + 1);
    exec(&mut s, "edit.undo", json!({}));
    assert_eq!(doc(&s).guides, before);
    assert_eq!(s.doc().unwrap().selection.guides, [0]);
    exec(&mut s, "edit.redo", json!({}));
    assert_eq!(doc(&s).guides, before[1..]);
    let protected = snapshot(&s);
    assert_eq!(exec(&mut s, "view.guides.clear", json!({}))["count"], 0);
    assert_eq!(snapshot(&s), protected, "clearing protected guides alone adds no undo step");
}

#[test]
fn deletion_remaps_editable_selection_around_protected_guides() {
    let mut s = session();
    for pos in [40.0, 60.0, 80.0, 100.0] {
        add(&mut s, true, pos);
    }
    props(&mut s, 1, true, true);
    props(&mut s, 2, false, false);
    exec(&mut s, "guide.select", json!({"indexes": [3]}));
    let before = doc(&s).guides.clone();
    assert_eq!(exec(&mut s, "guide.remove", json!({"index": 0}))["count"], 1);
    assert_eq!(doc(&s).guides, before[1..]);
    assert_eq!(s.doc().unwrap().selection.guides, [2]);
    exec(&mut s, "edit.undo", json!({}));
    assert_eq!(doc(&s).guides, before);
    assert_eq!(s.doc().unwrap().selection.guides, [3]);
}

// External guide edits preserve live previews, protection, Undo/Redo and journal replay.

fn prepublish_assert_document_replay(s: &Session) {
    let mut replay = Session::new();
    for (command, params) in &s.journal {
        exec(&mut replay, command, params.clone());
    }
    assert_eq!(
        serde_json::to_value(doc(&replay)).unwrap(),
        serde_json::to_value(doc(s)).unwrap(),
        "the command journal must reproduce the visible document"
    );
}

#[test]
fn prepublish_external_guide_properties_survive_hosted_ruler_drag_and_replay() {
    for (visible, locked) in [(false, false), (true, true)] {
        let mut s = fixtures::session_with(100.0, 100.0);
        add(&mut s, true, 25.0);
        let view = ViewInfo { smart_guides: false, snap_to_point: false, ..ViewInfo::default() };
        s.select_tool("selection", view).unwrap();
        s.ruler_guide(false, &PointerEvent::new(PointerKind::Drag, 50.0, 50.0), true, view).unwrap();
        assert!(!s.tool_busy(), "the hosted ruler gesture is separate from the selected tool");
        assert!(s.doc().unwrap().interaction.is_some());
        assert_eq!(doc(&s).guides.len(), 2);
        assert_eq!(props(&mut s, 0, visible, locked), json!({"index": 0, "visible": visible, "locked": locked}));

        s.ruler_guide(false, &PointerEvent::new(PointerKind::Drag, 50.0, 60.0), true, view).unwrap();
        assert_eq!(
            (doc(&s).guides[0].visible, doc(&s).guides[0].locked),
            (visible, locked),
            "a later ruler preview must not discard an accepted external property edit"
        );
        s.ruler_guide(false, &PointerEvent::new(PointerKind::Up, 50.0, 60.0), true, view).unwrap();
        assert_eq!((doc(&s).guides[0].visible, doc(&s).guides[0].locked), (visible, locked));
        assert_eq!(doc(&s).guides.len(), 2, "finishing or continuing the hosted gesture must not duplicate its guide");
        assert!(s.doc().unwrap().interaction.is_none());
        prepublish_assert_document_replay(&s);
    }
}

#[test]
fn prepublish_clear_guides_respects_converted_path_and_ancestor_protection() {
    // This oracle assumes the command's "visible, unlocked ... guide paths" description
    // is intended to protect path guides too; resolve that policy before adopting this test.
    for (ancestor, visible, locked) in [(false, false, false), (false, true, true), (true, false, false), (true, true, true)] {
        let mut s = fixtures::session_with(100.0, 100.0);
        let path = fixtures::rect(&mut s, 10.0, 10.0, 10.0, 10.0);
        assert_eq!(exec(&mut s, "view.guides.make", json!({}))["count"], 1);
        let target = if ancestor { doc(&s).layers[0].id } else { path };
        exec(&mut s, "layer.setProps", json!({"ids": [target.0], "visible": visible, "locked": locked}));
        assert!(!doc(&s).is_editable(path));
        add(&mut s, true, 40.0); // Positive control: an editable canvas ruler guide.
        let before = serde_json::to_value(doc(&s)).unwrap();
        let undo = s.doc().unwrap().history.undo.len();
        assert_eq!(
            exec(&mut s, "view.guides.clear", json!({}))["count"],
            1,
            "clear the editable ruler while preserving the protected converted path"
        );
        assert!(doc(&s).node(path).is_some());
        assert!(doc(&s).guides.is_empty());
        assert_eq!(s.doc().unwrap().history.undo.len(), undo + 1);
        let after = serde_json::to_value(doc(&s)).unwrap();
        prepublish_assert_document_replay(&s);
        exec(&mut s, "edit.undo", json!({}));
        assert_eq!(serde_json::to_value(doc(&s)).unwrap(), before);
        exec(&mut s, "edit.redo", json!({}));
        assert_eq!(serde_json::to_value(doc(&s)).unwrap(), after);
    }
}

// Existing Artboard resize only; the scaling variant stays in the combined-only artifact.
#[test]
fn guide_batch_during_ordinary_artboard_resize_keeps_preview_replay_and_undo() {
    // Normal resize/release/Undo/replay before the batch boundary regression.
    let mut control = fixtures::session_with(100.0, 100.0);
    let control_art = fixtures::rect(&mut control, 10.0, 20.0, 10.0, 10.0);
    exec(&mut control, "guide.add", json!({"vertical": true, "pos": 40, "artboard": 0}));
    let control_base = serde_json::to_value(doc(&control)).unwrap();
    let view = ViewInfo { smart_guides: false, snap_to_point: false, ..ViewInfo::default() };
    control.select_tool("artboard", view).unwrap();
    pointer(&mut control, PointerKind::Down, 100.0, 100.0, view);
    pointer(&mut control, PointerKind::Drag, 200.0, 200.0, view);
    pointer(&mut control, PointerKind::Up, 200.0, 200.0, view);
    assert_eq!(doc(&control).artboards[0].rect, Rect::new(0.0, 0.0, 200.0, 200.0));
    assert_eq!(doc(&control).node(control_art).unwrap().geometric_bounds().unwrap(), Rect::new(10.0, 20.0, 20.0, 30.0));
    assert_eq!(doc(&control).guides[0].pos, 40.0);
    let control_resized = serde_json::to_value(doc(&control)).unwrap();
    prepublish_assert_document_replay(&control);
    exec(&mut control, "edit.undo", json!({}));
    assert_eq!(serde_json::to_value(doc(&control)).unwrap(), control_base);
    prepublish_assert_document_replay(&control);
    exec(&mut control, "edit.redo", json!({}));
    assert_eq!(serde_json::to_value(doc(&control)).unwrap(), control_resized);
    prepublish_assert_document_replay(&control);

    let mut s = fixtures::session_with(100.0, 100.0);
    let art = fixtures::rect(&mut s, 10.0, 20.0, 10.0, 10.0);
    exec(&mut s, "guide.add", json!({"vertical": true, "pos": 40, "artboard": 0}));
    let base = serde_json::to_value(doc(&s)).unwrap();
    let undo = s.doc().unwrap().history.undo.len();
    let journal = s.journal.len();
    let view = ViewInfo { smart_guides: false, snap_to_point: false, ..ViewInfo::default() };
    s.select_tool("artboard", view).unwrap();
    pointer(&mut s, PointerKind::Down, 100.0, 100.0, view);
    pointer(&mut s, PointerKind::Drag, 200.0, 200.0, view);
    assert!(s.tool_busy());
    assert!(s.doc().unwrap().interaction.is_some());
    assert_eq!(doc(&s).artboards[0].rect, Rect::new(0.0, 0.0, 200.0, 200.0));
    assert_eq!(doc(&s).node(art).unwrap().geometric_bounds().unwrap(), Rect::new(10.0, 20.0, 20.0, 30.0));
    assert_eq!(doc(&s).guides[0].pos, 40.0);
    assert_eq!(s.journal.len(), journal, "the resize is still a preview");

    exec(
        &mut s,
        "command.batch",
        json!({"label": "Hide Owned Guide", "commands": [
            {"command": "guide.setProps", "params": {"index": 0, "visible": false}}
        ]}),
    );
    assert!(!doc(&s).guides[0].visible);
    assert_eq!(doc(&s).artboards[0].rect, Rect::new(0.0, 0.0, 200.0, 200.0));
    prepublish_assert_document_replay(&s); // Check before later commands can conceal the missing resize.
    assert_eq!(s.journal.len(), journal + 2, "commit the prior resize before journaling the batch");
    assert_eq!(s.doc().unwrap().history.undo.len(), undo + 2, "resize and external batch are independent undo steps");
    let accepted = serde_json::to_value(doc(&s)).unwrap();
    pointer(&mut s, PointerKind::Drag, 210.0, 210.0, view);
    pointer(&mut s, PointerKind::Up, 210.0, 210.0, view);
    assert_eq!(serde_json::to_value(doc(&s)).unwrap(), accepted, "late events must not change the accepted edits");
    exec(&mut s, "edit.undo", json!({}));
    assert_eq!(doc(&s).artboards[0].rect, Rect::new(0.0, 0.0, 200.0, 200.0));
    assert!(doc(&s).guides[0].visible);
    exec(&mut s, "edit.undo", json!({}));
    assert_eq!(serde_json::to_value(doc(&s)).unwrap(), base);
    exec(&mut s, "edit.redo", json!({}));
    exec(&mut s, "edit.redo", json!({}));
    assert_eq!(serde_json::to_value(doc(&s)).unwrap(), accepted);
    prepublish_assert_document_replay(&s);
}

// Clear Guides follows Node/ancestor editability and individual ruler protection.

fn ag3_state(s: &Session) -> Value {
    let st = s.doc().unwrap();
    json!({"doc": st.doc.as_ref(), "selection": st.selection})
}

fn ag3_history(s: &Session) -> Value {
    let st = s.doc().unwrap();
    let entries = |v: &[vectorcraft_engine::HistoryEntry]| -> Vec<Value> {
        v.iter().map(|e| json!({"label": e.label, "doc": e.doc.as_ref(), "selection": e.selection})).collect()
    };
    json!({"undo": entries(&st.history.undo), "redo": entries(&st.history.redo), "limit": st.history.limit})
}

fn ag3_replay(s: &Session) {
    let mut replay = Session::new();
    for (command, params) in &s.journal {
        exec(&mut replay, command, params.clone());
    }
    assert_eq!(ag3_state(&replay), ag3_state(s), "replay preserves the whole document and selection");
}

fn ag3_positive_control() {
    let mut s = fixtures::session_with(100.0, 100.0);
    let path = fixtures::rect(&mut s, 10.0, 10.0, 10.0, 10.0);
    assert_eq!(exec(&mut s, "view.guides.make", json!({}))["count"], 1);
    assert!(doc(&s).is_editable(path));
    add(&mut s, true, 40.0);
    let before = ag3_state(&s);
    let prefix = ag3_history(&s);
    assert_eq!(exec(&mut s, "view.guides.clear", json!({}))["count"], 2, "positive: an editable converted path and ruler are deleted");
    assert!(doc(&s).node(path).is_none() && doc(&s).guides.is_empty());
    let after = ag3_state(&s);
    assert_eq!(s.doc().unwrap().history.undo.len(), prefix["undo"].as_array().unwrap().len() + 1);
    ag3_replay(&s);
    exec(&mut s, "edit.undo", json!({}));
    assert_eq!(ag3_state(&s), before);
    assert_eq!(ag3_history(&s)["undo"], prefix["undo"]);
    ag3_replay(&s);
    exec(&mut s, "edit.redo", json!({}));
    assert_eq!(ag3_state(&s), after);
    assert!(s.doc().unwrap().history.redo.is_empty());
    ag3_replay(&s);
}

#[test]
fn ag3_clear_guides_uses_node_and_ancestor_editability_with_full_undo_and_replay() {
    for (ancestor, visible, locked) in [(false, false, false), (false, true, true), (true, false, false), (true, true, true)] {
        ag3_positive_control(); // Establish ordinary Clear behavior before every protected-path negative.
        let mut s = fixtures::session_with(100.0, 100.0);
        let protected = fixtures::rect(&mut s, 10.0, 10.0, 10.0, 10.0);
        assert_eq!(exec(&mut s, "view.guides.make", json!({}))["count"], 1);
        exec(&mut s, "layer.target", json!({"id": protected.0}));
        assert_eq!(s.doc().unwrap().selection.objects, [protected], "ordinary targeting selects the editable converted path");
        let target = if ancestor { doc(&s).layers[0].id } else { protected };
        exec(&mut s, "layer.setProps", json!({"ids": [target.0], "visible": visible, "locked": locked}));
        assert!(!doc(&s).is_editable(protected));
        assert!(s.doc().unwrap().selection.objects.is_empty(), "ordinary protection removes the art selection");
        // Existing Release Guides uses is_editable for its unselected-path fallback.
        let protected_state = ag3_state(&s);
        let protected_history = ag3_history(&s);
        let journal = s.journal.clone();
        assert!(s.execute("view.guides.release", &json!({})).is_err());
        assert_eq!(ag3_state(&s), protected_state);
        assert_eq!(ag3_history(&s), protected_history);
        assert_eq!(s.journal, journal);

        exec(&mut s, "layer.new", json!({"name": "Editable Guide Controls", "top": true}));
        let editable = fixtures::rect(&mut s, 40.0, 10.0, 10.0, 10.0);
        assert_eq!(exec(&mut s, "view.guides.make", json!({}))["count"], 1);
        assert!(doc(&s).is_editable(editable));
        let art = fixtures::rect(&mut s, 60.0, 60.0, 10.0, 10.0);
        let protected_node = serde_json::to_value(doc(&s).node(protected).unwrap()).unwrap();
        let art_node = serde_json::to_value(doc(&s).node(art).unwrap()).unwrap();
        for (i, (show, lock)) in FLAGS.into_iter().enumerate() {
            add(&mut s, true, 30.0 + i as f64 * 10.0);
            props(&mut s, i, show, lock);
        }
        let rulers = doc(&s).guides.clone();
        let before = ag3_state(&s);
        let prefix = ag3_history(&s);
        let result = exec(&mut s, "view.guides.clear", json!({}));
        assert!(doc(&s).node(protected).is_some(), "Clear must preserve the converted path protected by the same Node/ancestor rule");
        assert_eq!(result["count"], 2, "remove only the editable path and editable ruler");
        assert!(doc(&s).node(editable).is_none());
        assert_eq!(serde_json::to_value(doc(&s).node(protected).unwrap()).unwrap(), protected_node);
        assert_eq!(serde_json::to_value(doc(&s).node(art).unwrap()).unwrap(), art_node);
        assert_eq!(doc(&s).guides, rulers[1..]);
        assert_eq!(s.doc().unwrap().selection.objects, [art], "unrelated original art selection survives");
        assert_eq!(s.doc().unwrap().history.undo.len(), prefix["undo"].as_array().unwrap().len() + 1);
        assert_eq!(s.doc().unwrap().history.undo.last().unwrap().label, "Clear Guides");
        let after = ag3_state(&s);
        ag3_replay(&s);
        exec(&mut s, "edit.undo", json!({}));
        assert_eq!(ag3_state(&s), before);
        assert_eq!(ag3_history(&s)["undo"], prefix["undo"], "the whole existing history prefix survives");
        ag3_replay(&s);
        exec(&mut s, "edit.redo", json!({}));
        assert_eq!(ag3_state(&s), after);
        assert!(s.doc().unwrap().history.redo.is_empty());
        ag3_replay(&s);
        let no_op_history = ag3_history(&s);
        assert_eq!(exec(&mut s, "view.guides.clear", json!({}))["count"], 0, "only protected converted/ruler guides remain");
        assert_eq!(ag3_state(&s), after);
        assert_eq!(ag3_history(&s), no_op_history, "protected-only Clear adds no undo entry");
        ag3_replay(&s);
    }
}

// Grouped targets preserve single-target behavior and validate atomically.
#[test]
fn grouped_guide_properties_keep_single_api_unmentioned_flags_undo_and_replay() {
    for eye in [true, false] {
        let mut s = session();
        for pos in [40.0, 80.0, 120.0] {
            add(&mut s, true, pos);
        }
        props(&mut s, 1, true, true);
        props(&mut s, 2, false, true);
        let art = fixtures::rect(&mut s, 200.0, 200.0, 20.0, 10.0);
        let before = ag3_state(&s);
        let prefix = ag3_history(&s);
        let guides = doc(&s).guides.clone();
        let params = if eye { json!({"indexes": [1, 0, 1], "visible": false}) } else { json!({"indexes": [2, 0, 2], "locked": false}) };
        let expected = if eye {
            json!([{"index": 1, "visible": false, "locked": true}, {"index": 0, "visible": false, "locked": false}])
        } else {
            json!([{"index": 2, "visible": false, "locked": false}, {"index": 0, "visible": true, "locked": false}])
        };
        assert_eq!(exec(&mut s, "guide.setProps", params.clone()), expected, "deduplicate in first-occurrence order");
        assert_eq!(s.doc().unwrap().selection.objects, [art]);
        assert_eq!(s.doc().unwrap().history.undo.len(), prefix["undo"].as_array().unwrap().len() + 1);
        assert_eq!(s.doc().unwrap().history.undo.last().unwrap().label, "Guide Properties");
        let current = &doc(&s).guides;
        if eye {
            assert_eq!((current[0].visible, current[0].locked), (false, false));
            assert_eq!((current[1].visible, current[1].locked), (false, true));
            assert_eq!(current[2], guides[2]);
        } else {
            assert_eq!((current[2].visible, current[2].locked), (false, false));
            assert_eq!(current[0], guides[0], "equal target retains its distinct visibility");
            assert_eq!(current[1], guides[1]);
        }
        for (g, old) in current.iter().zip(&guides) {
            assert_eq!((g.vertical, g.pos, g.artboard), (old.vertical, old.pos, old.artboard));
        }
        let changed = ag3_state(&s);
        let history = ag3_history(&s);
        assert_eq!(exec(&mut s, "guide.setProps", params), expected);
        assert_eq!(ag3_state(&s), changed);
        assert_eq!(ag3_history(&s), history, "all-equal grouped values add no history");
        assert_eq!(exec(&mut s, "guide.setProps", json!({"index": 0, "locked": false})), json!({"index": 0, "visible": !eye, "locked": false}));
        assert_eq!(ag3_history(&s), history, "legacy single-index return and equal-value behavior stay intact");
        ag3_replay(&s);
        exec(&mut s, "edit.undo", json!({}));
        assert_eq!(ag3_state(&s), before);
        assert_eq!(ag3_history(&s)["undo"], prefix["undo"]);
        ag3_replay(&s);
        exec(&mut s, "edit.redo", json!({}));
        assert_eq!(ag3_state(&s), changed);
        ag3_replay(&s);
    }
}

#[test]
fn grouped_guide_properties_reject_later_invalid_targets_and_flags_atomically() {
    let mut s = session();
    for pos in [40.0, 80.0] {
        add(&mut s, true, pos);
    }
    exec(&mut s, "guide.select", json!({"indexes": [0, 1]}));
    fixtures::rect(&mut s, 200.0, 200.0, 10.0, 10.0);
    exec(&mut s, "edit.undo", json!({}));
    assert_eq!(s.doc().unwrap().selection.guides, [0, 1]);
    assert_eq!(s.doc().unwrap().history.redo.len(), 1);
    let before = ag3_state(&s);
    let history = ag3_history(&s);
    let journal = s.journal.clone();
    let revision = s.doc().unwrap().revision;
    for params in [
        json!({"indexes": [0, 99], "visible": false}),
        json!({"indexes": [0, -1], "locked": true}),
        json!({"indexes": [0, 0.5], "visible": false}),
        json!({"indexes": [0, "1"], "visible": false}),
        json!({"indexes": [0, null], "visible": false}),
        json!({"indexes": "0", "visible": false}),
        json!({"indexes": [], "visible": false}),
        json!({"index": 0, "indexes": [1], "visible": false}),
        json!({"indexes": [0, 1]}),
        json!({"indexes": [0, 1], "visible": false, "locked": null}),
        json!({"indexes": [0, 1], "visible": 0}),
        json!({"indexes": [0, 1], "locked": "true"}),
    ] {
        assert!(s.execute("guide.setProps", &params).is_err(), "{params}");
        assert_eq!(ag3_state(&s), before, "invalid later target must not mutate the valid first target");
        assert_eq!(ag3_history(&s), history, "full undo/redo snapshots survive rejection");
        assert_eq!(s.journal, journal);
        assert_eq!(s.doc().unwrap().revision, revision);
    }
}

#[test]
fn grouped_equal_properties_prune_stale_guides_only_under_global_lock() {
    let mut s = session();
    add(&mut s, true, 40.0);
    add(&mut s, false, 80.0);
    props(&mut s, 1, false, true);
    let art = fixtures::rect(&mut s, 200.0, 200.0, 20.0, 10.0);
    exec(&mut s, "view.guides.lock", json!({"locked": true}));
    // Existing policy tests use controlled stale history/selection inputs too.
    s.doc_mut().unwrap().selection.guides = vec![0, 1, 99];
    let guides = doc(&s).guides.clone();
    let undo = s.doc().unwrap().history.undo.len();
    assert_eq!(exec(&mut s, "guide.setProps", json!({"indexes": [0, 0], "visible": true})), json!([{"index": 0, "visible": true, "locked": false}]));
    assert!(s.guides_locked());
    assert_eq!(doc(&s).guides, guides, "unmentioned flags and untargeted protected guide stay unchanged");
    assert!(s.doc().unwrap().selection.guides.is_empty());
    assert_eq!(s.doc().unwrap().selection.objects, [art]);
    assert_eq!(s.doc().unwrap().history.undo.len(), undo + 1, "the necessary pruning is one edit");
    let state = ag3_state(&s);
    let history = ag3_history(&s);
    exec(&mut s, "guide.setProps", json!({"indexes": [0], "visible": true}));
    assert_eq!(ag3_state(&s), state);
    assert_eq!(ag3_history(&s), history, "repeat after pruning is a true no-op");
}
