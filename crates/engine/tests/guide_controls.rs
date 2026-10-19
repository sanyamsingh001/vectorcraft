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

// Native artboard clipboard regressions. The guide snapshot and paste hook are implemented;
// these assertions remain pending parent compilation and runtime CI.
fn clipboard_scene() -> (Session, usize, vectorcraft_doc::NodeId) {
    let mut s = session();
    let index = exec(&mut s, "artboard.new", json!({"x": 120, "y": 75, "width": 160, "height": 120, "name": "Guide Board"}))["index"]
        .as_u64()
        .unwrap() as usize;
    add(&mut s, true, 140.0); // Canvas guide at the same position as one owned by the copied board.
    exec(&mut s, "guide.add", json!({"vertical": true, "pos": 140, "artboard": 0}));
    for vertical in [true, false] {
        for (i, (visible, locked)) in FLAGS.into_iter().enumerate() {
            let pos = if vertical { 140.0 } else { 90.0 } + i as f64 * 10.0;
            let guide = exec(&mut s, "guide.add", json!({"vertical": vertical, "pos": pos, "artboard": index}))["index"].as_u64().unwrap();
            props(&mut s, guide as usize, visible, locked);
        }
    }
    let art = fixtures::rect(&mut s, 140.0, 90.0, 30.0, 20.0);
    fixtures::rect(&mut s, 320.0, 240.0, 10.0, 10.0);
    exec(&mut s, "artboard.reorder", json!({"index": index, "to": 0}));
    assert_ne!(doc(&s).artboards[0].id as usize, 0, "the source's stable id differs from its reordered index");
    (s, 0, art)
}

fn board_guides(d: &Document, id: u32) -> Vec<Guide> {
    d.guides.iter().filter(|g| g.artboard == Some(id)).cloned().collect()
}

fn assert_clipboard_board_guides(s: &Session, index: usize, source: &[Guide], delta: Vec2) {
    let id = doc(s).artboards[index].id;
    let expected: Vec<Guide> = source.iter().map(|g| Guide { artboard: Some(id), ..g.moved(delta) }).collect();
    assert_eq!(board_guides(doc(s), id), expected, "only the copied board's guides arrive, with flags and remapped ownership");
    assert!(s.doc().unwrap().selection.guides.is_empty(), "paste does not select hidden or locked guides");
}

#[test]
fn clipboard_artboard_guides_follow_paste_modes_and_stable_board_ids() {
    for command in ["edit.paste", "edit.pasteInPlace", "edit.pasteInFront", "edit.pasteInBack"] {
        for include_art in [false, true] {
            for cross_document in [false, true] {
                let (mut s, source_index, _) = clipboard_scene();
                let source_board = doc(&s).artboards[source_index].clone();
                let source_guides = board_guides(doc(&s), source_board.id);
                assert_eq!(source_guides.len(), 8);
                let original = snapshot(&s);
                assert_eq!(exec(&mut s, "artboard.copy", json!({"index": source_index, "art": include_art}))["copied"], usize::from(include_art));
                assert_eq!(snapshot(&s), original, "Copy does not edit the document or add undo");
                let clip = s.clipboard.clone();
                assert_eq!(clip.nodes.len(), usize::from(include_art), "copied counts remain artwork counts");
                assert_eq!(clip.artboard.as_ref().unwrap(), &source_board);
                if cross_document {
                    exec(&mut s, "file.new", json!({"width": 260, "height": 180, "created": fixtures::CREATED}));
                    exec(&mut s, "artboard.new", json!({"x": 350, "y": 40, "width": 80, "height": 60, "name": "Existing"}));
                    exec(&mut s, "artboard.delete", json!({"index": 0}));
                    assert_eq!(doc(&s).artboards[0].id, source_board.id, "destination collision must not capture the copied guides");
                    exec(&mut s, "guide.add", json!({"vertical": true, "pos": 140, "artboard": 0}));
                    add(&mut s, true, 140.0);
                }
                let before = snapshot(&s);
                let existing_guides = doc(&s).guides.clone();
                let right = doc(&s).artboards.iter().map(|a| a.rect.x1).fold(f64::MIN, f64::max);
                let delta = if command == "edit.paste" { Vec2::new(right + 20.0 - source_board.rect.x0, 0.0) } else { Vec2::ZERO };
                let out = exec(&mut s, command, json!({"center": [5, 5], "dx": 999, "dy": 999}));
                let index = out["artboard"].as_u64().unwrap() as usize;
                assert_eq!(doc(&s).artboards[index].rect, source_board.rect + delta, "board paste retains existing placement policy");
                assert_ne!(doc(&s).artboards[index].id, source_board.id);
                if cross_document {
                    assert_ne!(doc(&s).artboards[index].id as usize, index, "paste remaps to an id, not an index");
                }
                assert_eq!(out["ids"].as_array().unwrap().len(), usize::from(include_art));
                if include_art {
                    let id = vectorcraft_doc::NodeId(out["ids"][0].as_u64().unwrap());
                    assert_eq!(doc(&s).node(id).unwrap().geometric_bounds().unwrap(), Rect::new(140.0, 90.0, 170.0, 110.0) + delta);
                }
                assert_clipboard_board_guides(&s, index, &source_guides, delta);
                assert_eq!(&doc(&s).guides[..existing_guides.len()], existing_guides.as_slice(), "canvas and other-board guides stay unchanged");
                assert_eq!(doc(&s).guides.len(), existing_guides.len() + source_guides.len());
                assert_eq!(s.doc().unwrap().history.undo.len(), before.2 + 1);
                assert_eq!(s.clipboard, clip);
                let pasted = serde_json::to_value(doc(&s)).unwrap();
                assert_eq!(vectorcraft_format::load(&vectorcraft_format::save_file(doc(&s))).unwrap().guides, doc(&s).guides);
                exec(&mut s, "edit.undo", json!({}));
                assert_eq!(serde_json::to_value(doc(&s)).unwrap(), before.0);
                assert_eq!(s.doc().unwrap().selection, before.1);
                assert_eq!(s.doc().unwrap().history.undo.len(), before.2);
                assert_eq!(s.clipboard, clip, "Undo keeps the existing reusable clipboard snapshot");
                exec(&mut s, "edit.redo", json!({}));
                assert_eq!(serde_json::to_value(doc(&s)).unwrap(), pasted);
                assert_eq!(s.clipboard, clip);
                if cross_document {
                    exec(&mut s, "document.activate", json!({"index": 0}));
                    assert_eq!(serde_json::to_value(doc(&s)).unwrap(), original.0, "cross-document paste leaves the source alone");
                }
            }
        }
    }
}

#[test]
fn clipboard_artboard_guides_survive_source_changes_closure_and_repeated_paste() {
    for command in ["edit.paste", "edit.pasteInPlace"] {
        let (mut s, index, _) = clipboard_scene();
        let source_board = doc(&s).artboards[index].clone();
        let source_guides = board_guides(doc(&s), source_board.id);
        exec(&mut s, "artboard.copy", json!({"index": index, "art": false}));
        let clip = s.clipboard.clone();
        for guide in 2..10 {
            props(&mut s, guide, true, false);
        }
        exec(&mut s, "artboard.move", json!({"index": index, "dx": 15, "dy": -25, "moveArt": false}));
        exec(&mut s, "file.new", json!({"width": 200, "height": 150, "created": fixtures::CREATED}));
        exec(&mut s, "file.close", json!({"index": 0}));
        assert_eq!(s.clipboard, clip, "source edits and closure cannot alter copied flags or positions");
        for _ in 0..2 {
            let before = doc(&s).guides.clone();
            let right = doc(&s).artboards.iter().map(|a| a.rect.x1).fold(f64::MIN, f64::max);
            let delta = if command == "edit.paste" { Vec2::new(right + 20.0 - source_board.rect.x0, 0.0) } else { Vec2::ZERO };
            let index = exec(&mut s, command, json!({}))["artboard"].as_u64().unwrap() as usize;
            assert_eq!(doc(&s).artboards[index].rect, source_board.rect + delta);
            assert_clipboard_board_guides(&s, index, &source_guides, delta);
            assert_eq!(&doc(&s).guides[..before.len()], before.as_slice());
            assert_eq!(doc(&s).guides.len(), before.len() + 8);
            assert_eq!(s.clipboard, clip);
        }
    }
}

#[test]
fn clipboard_artboard_cut_and_paste_restore_guides_with_undo_redo() {
    for tool_route in [false, true] {
        for include_art in [false, true] {
            let (mut s, index, art) = clipboard_scene();
            let source_board = doc(&s).artboards[index].clone();
            let source_guides = board_guides(doc(&s), source_board.id);
            if tool_route {
                s.select_tool("artboard", ViewInfo::default()).unwrap();
                s.set_tool_option("active", &json!(index));
                s.set_tool_option("moveArt", &json!(include_art));
            }
            exec(&mut s, "guide.select", json!({"indexes": [2]}));
            let before = snapshot(&s);
            let command = if tool_route { "edit.cut" } else { "artboard.cut" };
            assert_eq!(exec(&mut s, command, json!({"index": index, "art": include_art}))["copied"], usize::from(include_art));
            let cut = serde_json::to_value(doc(&s)).unwrap();
            assert_eq!(s.doc().unwrap().history.undo.len(), before.2 + 1);
            assert_eq!(doc(&s).artboards.len(), 1);
            assert_eq!(doc(&s).guides.len(), 2, "only canvas and the other board's guide remain");
            assert!(board_guides(doc(&s), source_board.id).is_empty());
            assert_eq!(doc(&s).node(art).is_some(), !include_art);
            assert!(s.doc().unwrap().selection.guides.is_empty());
            let clip = s.clipboard.clone();
            exec(&mut s, "edit.undo", json!({}));
            assert_eq!(serde_json::to_value(doc(&s)).unwrap(), before.0);
            assert_eq!(s.doc().unwrap().selection, before.1);
            assert_eq!(s.clipboard, clip);
            exec(&mut s, "edit.redo", json!({}));
            assert_eq!(serde_json::to_value(doc(&s)).unwrap(), cut);
            let undo = s.doc().unwrap().history.undo.len();
            let pasted = exec(&mut s, "edit.pasteInPlace", json!({}));
            let index = pasted["artboard"].as_u64().unwrap() as usize;
            assert_eq!(doc(&s).artboards[index].rect, source_board.rect);
            assert_clipboard_board_guides(&s, index, &source_guides, Vec2::ZERO);
            assert_eq!(doc(&s).guides.len(), 10);
            assert_eq!(pasted["ids"].as_array().unwrap().len(), usize::from(include_art));
            assert_eq!(s.doc().unwrap().history.undo.len(), undo + 1);
            exec(&mut s, "edit.undo", json!({}));
            assert_eq!(serde_json::to_value(doc(&s)).unwrap(), cut, "Paste undo removes the restored board and its guides together");
            exec(&mut s, "edit.undo", json!({}));
            assert_eq!(serde_json::to_value(doc(&s)).unwrap(), before.0, "Cut undo restores original guide indexes and ownership");
            assert_eq!(s.doc().unwrap().selection, before.1);
            assert_eq!(s.clipboard, clip);
        }
    }
}

#[test]
fn clipboard_artboard_move_art_policy_keeps_owned_guides_with_the_board() {
    for move_art in [false, true] {
        let (mut s, index, art) = clipboard_scene();
        let source_id = doc(&s).artboards[index].id;
        let before = doc(&s).guides.clone();
        let source_guides = board_guides(doc(&s), source_id);
        let art_bounds = doc(&s).node(art).unwrap().geometric_bounds().unwrap();
        s.select_tool("artboard", ViewInfo::default()).unwrap();
        s.set_tool_option("active", &json!(index));
        s.set_tool_option("moveArt", &json!(move_art));
        exec(&mut s, "view.guides.lock", json!({"locked": true}));
        let delta = Vec2::new(12.0, 19.0);
        exec(&mut s, "artboard.move", json!({"index": index, "dx": delta.x, "dy": delta.y, "moveArt": move_art}));
        assert_eq!(board_guides(doc(&s), source_id), source_guides.iter().map(|g| g.moved(delta)).collect::<Vec<_>>());
        assert_eq!(&doc(&s).guides[..2], &before[..2]);
        assert_eq!(doc(&s).node(art).unwrap().geometric_bounds().unwrap(), if move_art { art_bounds + delta } else { art_bounds });
        let moved_board = doc(&s).artboards[index].clone();
        let moved_guides = board_guides(doc(&s), source_id);
        assert_eq!(exec(&mut s, "edit.copy", json!({}))["copied"], usize::from(move_art), "Move/Copy Artwork affects artwork, not owned guides");
        let paste = exec(&mut s, "edit.pasteInPlace", json!({}));
        let pasted_index = paste["artboard"].as_u64().unwrap() as usize;
        assert_eq!(doc(&s).artboards[pasted_index].rect, moved_board.rect);
        assert_clipboard_board_guides(&s, pasted_index, &moved_guides, Vec2::ZERO);
        assert_eq!(paste["ids"].as_array().unwrap().len(), usize::from(move_art));
        assert!(s.guides_locked(), "board lifecycle does not unlock the global guide policy");
    }
}

#[test]
fn clipboard_paste_on_all_artboards_and_object_copy_never_add_ruler_guides() {
    for include_art in [false, true] {
        let (mut s, index, art) = clipboard_scene();
        exec(&mut s, "artboard.copy", json!({"index": index, "art": include_art}));
        let clip = s.clipboard.clone();
        let guides = doc(&s).guides.clone();
        let boards = doc(&s).artboards.clone();
        let paste = exec(&mut s, "edit.pasteOnAllArtboards", json!({}));
        assert!(paste["artboard"].is_null());
        assert_eq!(paste["ids"].as_array().unwrap().len(), if include_art { 2 } else { 0 });
        assert_eq!(doc(&s).guides, guides);
        assert_eq!(doc(&s).artboards, boards);
        assert_eq!(s.clipboard, clip);
        s.select_tool("selection", ViewInfo::default()).unwrap();
        fixtures::select(&mut s, &[art]);
        exec(&mut s, "edit.copy", json!({}));
        assert!(s.clipboard.artboard.is_none(), "ordinary object Copy replaces the artboard clipboard");
        assert_eq!(s.clipboard.nodes.len(), 1);
        let paste = exec(&mut s, "edit.pasteInPlace", json!({}));
        assert!(paste["artboard"].is_null());
        assert_eq!(paste["ids"].as_array().unwrap().len(), 1);
        assert_eq!(doc(&s).guides, guides, "previous board guides must not leak into an object paste");
        assert_eq!(doc(&s).artboards, boards);
    }
}

#[test]
fn clipboard_cut_of_only_artboard_preserves_document_and_existing_clipboard() {
    for tool_route in [false, true] {
        let mut s = session();
        let art = fixtures::rect(&mut s, 20.0, 20.0, 30.0, 20.0);
        exec(&mut s, "edit.copy", json!({}));
        let clip = s.clipboard.clone();
        let index = exec(&mut s, "guide.add", json!({"vertical": true, "pos": 40, "artboard": 0}))["index"].as_u64().unwrap() as usize;
        props(&mut s, index, false, true);
        if tool_route {
            s.select_tool("artboard", ViewInfo::default()).unwrap();
        }
        let command = if tool_route { "edit.cut" } else { "artboard.cut" };
        rejects_without_changes(&mut s, command, json!({"index": 0, "art": true}));
        assert_eq!(s.clipboard, clip);
        assert!(doc(&s).node(art).is_some());
    }
}

// Private prepublication regression proposals. These tests have not been compiled or run.
// Each fixture is original, in memory, and bounded to one artboard and at most two guides.

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
fn prepublish_guide_batch_during_live_artboard_scale_keeps_resize_in_replay_and_undo() {
    let mut s = fixtures::session_with(100.0, 100.0);
    let art = fixtures::rect(&mut s, 10.0, 20.0, 10.0, 10.0);
    exec(&mut s, "guide.add", json!({"vertical": true, "pos": 40, "artboard": 0}));
    let base = serde_json::to_value(doc(&s)).unwrap();
    let undo = s.doc().unwrap().history.undo.len();
    let journal = s.journal.len();
    let view = ViewInfo { smart_guides: false, snap_to_point: false, ..ViewInfo::default() };
    s.select_tool("artboard", view).unwrap();
    s.set_tool_option("scaleArt", &json!(true));
    pointer(&mut s, PointerKind::Down, 100.0, 100.0, view);
    pointer(&mut s, PointerKind::Drag, 200.0, 200.0, view);
    assert!(s.tool_busy());
    assert!(s.doc().unwrap().interaction.is_some());
    assert_eq!(doc(&s).artboards[0].rect, Rect::new(0.0, 0.0, 200.0, 200.0));
    assert_eq!(doc(&s).node(art).unwrap().geometric_bounds().unwrap(), Rect::new(20.0, 40.0, 40.0, 60.0));
    assert_eq!(doc(&s).guides[0].pos, 80.0);
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
