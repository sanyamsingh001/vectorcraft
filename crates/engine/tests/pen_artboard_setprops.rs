//! External artboard edits during ordinary Pen gestures, including artwork scaling (#602).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
use serde_json::json;
use vectorcraft_doc::NodeId;
use vectorcraft_engine::{Session, ViewInfo};
use vectorcraft_geom::{Affine, PathData, Rect};
use vectorcraft_tools::{Mods, PointerEvent, PointerKind, ToolKey};

fn pointer(s: &mut Session, kind: PointerKind, x: f64, y: f64) {
    s.pointer(&PointerEvent::new(kind, x, y), ViewInfo::default()).unwrap();
}

fn path(s: &Session, id: NodeId) -> PathData {
    s.doc().unwrap().doc.node(id).unwrap().path_data().unwrap().clone()
}

fn live_pen() -> (Session, NodeId, PathData, usize) {
    live_pen_with_limits(false, false, false)
}

fn live_pen_with_limits(canvas_edge: bool, guide_edge: bool, mask_edge: bool) -> (Session, NodeId, PathData, usize) {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width":800,"height":600,"created":1_000_000_000})).unwrap();
    if canvas_edge {
        s.execute("artboard.setProps", &json!({"index":0,"x":vectorcraft_engine::MAX_COORD - 800.0})).unwrap();
    }
    if guide_edge {
        s.execute("guide.add", &json!({"vertical":true,"pos":vectorcraft_engine::MAX_COORD,"artboard":0})).unwrap();
    }
    if mask_edge {
        add_opacity_mask(&mut s, 2_000_010.0, true);
    }
    start_live_pen(s)
}

fn add_opacity_mask(s: &mut Session, x: f64, linked: bool) -> NodeId {
    let art = s.execute("shape.rectangle", &json!({"x":10,"y":10,"width":10,"height":10})).unwrap()["id"].as_u64().unwrap();
    let mask = s.execute("shape.rectangle", &json!({"x":x,"y":20,"width":10,"height":10})).unwrap()["id"].clone();
    s.execute("transparency.makeOpacityMask", &json!({"ids":[art,mask],"clip":false})).unwrap();
    if !linked {
        s.execute("transparency.unlinkOpacityMask", &json!({"id":art})).unwrap();
    }
    s.execute("select.set", &json!({"ids":[]})).unwrap();
    NodeId(art)
}

fn start_live_pen(mut s: Session) -> (Session, NodeId, PathData, usize) {
    s.select_tool("pen", ViewInfo::default()).unwrap();
    pointer(&mut s, PointerKind::Down, 100.0, 300.0);
    pointer(&mut s, PointerKind::Up, 100.0, 300.0);
    let id = s.doc().unwrap().selection.objects[0];
    let before = path(&s, id);
    let undo = s.doc().unwrap().history.undo.len();
    pointer(&mut s, PointerKind::Down, 200.0, 300.0);
    pointer(&mut s, PointerKind::Drag, 230.0, 320.0);
    assert!(s.in_interaction() && s.tool_busy());
    (s, id, before, undo)
}

#[test]
fn external_setprops_keeps_live_pen_and_page_edits_with_separate_undo_and_replay() {
    for scale_art in [false, true] {
        let (mut s, id, before, undo) = live_pen();
        let preview = path(&s, id);
        let boards = s.doc().unwrap().doc.artboards.clone();
        let selection = s.doc().unwrap().selection.objects.clone();
        let (width, height) = if scale_art { (1600.0, 1200.0) } else { (900.0, 650.0) };
        let out = s.execute("artboard.setProps", &json!({"index":0,"width":width,"height":height,"name":"Adjusted","scaleArt":scale_art})).unwrap();
        let expected = if scale_art { preview.transformed(Affine::scale(2.0)) } else { preview.clone() };
        if scale_art {
            assert_eq!(out["ids"], json!([id.0]), "the visible Pen path is eligible artwork");
        }
        let edited = s.doc().unwrap().doc.clone();
        assert_eq!(edited.artboards[0].rect, Rect::new(0.0, 0.0, width, height));
        assert_eq!(edited.artboards[0].name, "Adjusted");
        assert_eq!(path(&s, id), expected);
        assert_eq!(s.doc().unwrap().selection.objects, selection);
        assert_eq!(s.doc().unwrap().history.undo.len(), undo + 2);
        assert_eq!(s.doc().unwrap().history.undo.last().unwrap().label, "Artboard Options");
        assert!(!s.in_interaction() && !s.tool_busy());
        let n = s.journal.len();
        assert_eq!(s.journal[n - 2].0, "path.appendAnchor");
        assert_eq!(s.journal[n - 1].0, "artboard.setProps");
        pointer(&mut s, PointerKind::Drag, 250.0, 350.0);
        pointer(&mut s, PointerKind::Up, 250.0, 350.0);
        assert_eq!(s.doc().unwrap().doc.artboards, edited.artboards);
        assert_eq!(path(&s, id), expected, "late pointer events cannot erase or rescale the command");
        assert_eq!(s.doc().unwrap().history.undo.len(), undo + 2);
        let journal = s.journal.clone();
        let mut replay = Session::new();
        for (command, params) in &journal {
            replay.execute(command, params).unwrap();
        }
        assert_eq!(replay.doc().unwrap().doc.artboards, edited.artboards);
        assert_eq!(path(&replay, id), expected);
        s.execute("edit.undo", &json!({})).unwrap();
        assert_eq!(s.doc().unwrap().doc.artboards, boards);
        assert_eq!(path(&s, id), preview, "one undo reverses only Artboard Options");
        s.execute("edit.undo", &json!({})).unwrap();
        assert_eq!(path(&s, id), before, "the preceding undo reverses the Pen anchor");
        s.execute("edit.redo", &json!({})).unwrap();
        assert_eq!(path(&s, id), preview);
        s.execute("edit.redo", &json!({})).unwrap();
        assert_eq!(path(&s, id), expected);
        assert_eq!(s.doc().unwrap().doc.artboards, edited.artboards);
    }
}

#[test]
fn rejected_setprops_preserves_the_live_gesture_and_its_history() {
    for case in 0..4 {
        // Standard page dimensions and a small Pen gesture. The finite page/guide/mask edge cases
        // reject during the edit's geometry check, after option validation and document changes.
        let (mut s, id, before, undo) = live_pen_with_limits(case == 1, case == 2, case == 3);
        let preview = path(&s, id);
        let doc = s.doc().unwrap().doc.clone();
        let revision = s.doc().unwrap().revision;
        let journal = s.journal.clone();
        let selection = s.doc().unwrap().selection.clone();
        let interaction = s.doc().unwrap().interaction.clone().unwrap();
        let redo = s.doc().unwrap().history.redo.len();
        let dirty = s.doc().unwrap().is_dirty();
        let state = s.doc().unwrap().clone();
        let prefs = s.prefs.clone();
        let paint = s.paint.clone();
        let options = s.tool_options();
        let tool = s.tool_id();
        let params = match case {
            0 => json!({"index":1,"name":"Missing page"}),
            1 => json!({"index":0,"width":900}),
            2 => json!({"index":0,"width":900,"height":650,"name":"Trial scaling","scaleArt":true}),
            _ => json!({"index":0,"width":1600,"height":1200,"name":"Trial mask scaling","scaleArt":true}),
        };
        let out = s.execute("artboard.setProps", &params);
        assert!(out.is_err());
        if case == 3 {
            assert!(
                matches!(out, Err(vectorcraft_engine::EngineError::Other(ref message)) if message == "scaled artwork would exceed the canvas (coordinates out of range)")
            );
        }
        assert_eq!(
            serde_json::to_value(&*s.doc().unwrap().doc).unwrap(),
            serde_json::to_value(&*doc).unwrap(),
            "trial artwork, guides and metadata must roll back"
        );
        assert!(std::sync::Arc::ptr_eq(&s.doc().unwrap().doc, &doc));
        assert_eq!(s.doc().unwrap().selection, selection);
        let current = s.doc().unwrap().interaction.as_ref().unwrap();
        assert!(std::sync::Arc::ptr_eq(&current.doc, &interaction.doc));
        assert_eq!(current.preview, interaction.preview);
        assert_eq!(current.selection, interaction.selection);
        assert_eq!(current.label, interaction.label);
        assert_eq!(current.active_layer, interaction.active_layer);
        assert_eq!(current.layer_rows, interaction.layer_rows);
        assert_eq!(current.isolation, interaction.isolation);
        assert_eq!(current.perspective_again, interaction.perspective_again);
        assert_eq!(s.doc().unwrap().active_layer, state.active_layer);
        assert_eq!(s.doc().unwrap().layer_rows, state.layer_rows);
        assert_eq!(s.doc().unwrap().isolation, state.isolation);
        assert_same_history(&s.doc().unwrap().history, &state.history);
        assert_eq!(s.prefs, prefs);
        assert_eq!(s.paint, paint);
        assert_eq!(s.tool_options(), options);
        assert_eq!(s.tool_id(), tool);
        assert_eq!(s.doc().unwrap().history.redo.len(), redo);
        assert_eq!(s.doc().unwrap().is_dirty(), dirty);
        assert_eq!(path(&s, id), preview);
        assert_eq!(s.doc().unwrap().revision, revision);
        assert_eq!(s.doc().unwrap().history.undo.len(), undo);
        assert_eq!(s.journal, journal);
        assert!(s.in_interaction() && s.tool_busy());
        pointer(&mut s, PointerKind::Drag, 250.0, 350.0);
        pointer(&mut s, PointerKind::Up, 250.0, 350.0);
        assert_ne!(path(&s, id), preview, "the rejected command leaves the handle editable");
        assert_eq!(path(&s, id).anchor_count(), 2);
        assert_eq!(s.doc().unwrap().doc.artboards, doc.artboards);
        assert_eq!(s.doc().unwrap().history.undo.len(), undo + 1);
        s.execute("edit.undo", &json!({})).unwrap();
        assert_eq!(path(&s, id), before);
    }
}

#[test]
fn setprops_between_pen_anchors_keeps_the_path_in_progress() {
    let (mut s, id, _, undo) = live_pen();
    pointer(&mut s, PointerKind::Up, 230.0, 320.0);
    s.execute("artboard.setProps", &json!({"index":0,"name":"Renamed"})).unwrap();
    pointer(&mut s, PointerKind::Down, 350.0, 400.0);
    pointer(&mut s, PointerKind::Up, 350.0, 400.0);
    assert_eq!(path(&s, id).anchor_count(), 3);
    assert_eq!(s.doc().unwrap().doc.layers[0].children().unwrap().len(), 1);
    assert_eq!(s.doc().unwrap().doc.artboards[0].name, "Renamed");
    assert_eq!(s.doc().unwrap().history.undo.len(), undo + 3);
}

#[test]
fn artboard_tool_resize_previews_still_scale_once_and_commit_one_undo() {
    for scale_art in [false, true] {
        let (mut s, id, _, _) = live_pen();
        pointer(&mut s, PointerKind::Up, 230.0, 320.0);
        s.select_tool("artboard", ViewInfo::default()).unwrap();
        s.set_tool_option("scaleArt", &json!(scale_art));
        let before = path(&s, id);
        let undo = s.doc().unwrap().history.undo.len();
        pointer(&mut s, PointerKind::Down, 800.0, 600.0);
        pointer(&mut s, PointerKind::Drag, 1200.0, 900.0);
        assert!(s.in_interaction() && s.tool_busy());
        assert_eq!(s.doc().unwrap().history.undo.len(), undo);
        pointer(&mut s, PointerKind::Drag, 1600.0, 1200.0);
        pointer(&mut s, PointerKind::Up, 1600.0, 1200.0);
        let expected = if scale_art { before.transformed(Affine::scale(2.0)) } else { before.clone() };
        assert_eq!(path(&s, id), expected);
        assert_eq!(s.doc().unwrap().doc.artboards[0].rect, Rect::new(0.0, 0.0, 1600.0, 1200.0));
        assert_eq!(s.doc().unwrap().history.undo.len(), undo + 1);
        assert_eq!(s.doc().unwrap().history.undo.last().unwrap().label, "Resize Artboard");
        s.execute("edit.undo", &json!({})).unwrap();
        assert_eq!(path(&s, id), before);
        assert_eq!(s.doc().unwrap().doc.artboards[0].rect, Rect::new(0.0, 0.0, 800.0, 600.0));
    }
}

#[test]
fn idle_setprops_batch_keeps_one_undo_and_rolls_back_on_a_missing_page() {
    for fail in [false, true] {
        let (mut s, id, _, _) = live_pen();
        pointer(&mut s, PointerKind::Up, 230.0, 320.0);
        let before = s.doc().unwrap().doc.clone();
        let undo = s.doc().unwrap().history.undo.len();
        let journal = s.journal.clone();
        let last = if fail { json!({"index":1,"name":"Missing"}) } else { json!({"index":0,"name":"Batch page"}) };
        let out = s.execute(
            "command.batch",
            &json!({"label":"Page settings","commands":[
                {"command":"artboard.setProps","params":{"index":0,"width":900,"height":650}},
                {"command":"artboard.setProps","params":last}
            ]}),
        );
        if fail {
            assert!(out.is_err());
            assert_eq!(s.doc().unwrap().doc.artboards, before.artboards);
            assert_eq!(s.doc().unwrap().history.undo.len(), undo);
            assert_eq!(s.journal, journal);
        } else {
            out.unwrap();
            assert_eq!(s.doc().unwrap().doc.artboards[0].rect, Rect::new(0.0, 0.0, 900.0, 650.0));
            assert_eq!(s.doc().unwrap().doc.artboards[0].name, "Batch page");
            assert_eq!(s.doc().unwrap().history.undo.len(), undo + 1);
            assert_eq!(s.doc().unwrap().history.undo.last().unwrap().label, "Page settings");
            s.execute("edit.undo", &json!({})).unwrap();
            assert_eq!(s.doc().unwrap().doc.artboards, before.artboards);
        }
        assert_eq!(path(&s, id), before.node(id).unwrap().path_data().unwrap().clone());
    }
}

#[test]
fn external_setprops_during_artboard_resize_uses_the_validated_visible_geometry() {
    let (mut s, id, _, _) = live_pen();
    pointer(&mut s, PointerKind::Up, 230.0, 320.0);
    s.select_tool("artboard", ViewInfo::default()).unwrap();
    s.set_tool_option("scaleArt", &json!(true));
    let before = path(&s, id);
    let undo = s.doc().unwrap().history.undo.len();
    pointer(&mut s, PointerKind::Down, 800.0, 600.0);
    pointer(&mut s, PointerKind::Drag, 1200.0, 900.0);
    let preview = before.transformed(Affine::scale(1.5));
    assert_eq!(path(&s, id), preview);
    assert!(s.in_interaction() && s.tool_busy());
    s.execute("artboard.setProps", &json!({"index":0,"width":1600,"height":1200,"scaleArt":true})).unwrap();
    let expected = before.transformed(Affine::scale(2.0));
    assert_eq!(path(&s, id), expected, "cancel-on-deactivation must not change the validated starting geometry");
    assert_eq!(s.doc().unwrap().history.undo.len(), undo + 2);
    assert!(!s.in_interaction() && !s.tool_busy());
    pointer(&mut s, PointerKind::Drag, 1800.0, 1350.0);
    pointer(&mut s, PointerKind::Up, 1800.0, 1350.0);
    assert_eq!(path(&s, id), expected);
    assert_eq!(s.doc().unwrap().doc.artboards[0].rect, Rect::new(0.0, 0.0, 1600.0, 1200.0));
    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(path(&s, id), preview);
    assert_eq!(s.doc().unwrap().doc.artboards[0].rect, Rect::new(0.0, 0.0, 1200.0, 900.0));
    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(path(&s, id), before);
    assert_eq!(s.doc().unwrap().doc.artboards[0].rect, Rect::new(0.0, 0.0, 800.0, 600.0));
}

#[test]
fn external_setprops_keeps_the_active_artboard_for_the_next_delete() {
    for drag in [false, true] {
        let mut s = Session::new();
        s.execute("file.new", &json!({"width":100,"height":100})).unwrap();
        s.execute("artboard.new", &json!({"x":200,"y":0,"width":100,"height":100,"name":"Second"})).unwrap();
        let first = s.doc().unwrap().doc.artboards[0].clone();
        s.select_tool("artboard", ViewInfo::default()).unwrap();
        pointer(&mut s, PointerKind::Down, 250.0, 50.0);
        pointer(&mut s, PointerKind::Up, 250.0, 50.0);
        assert_eq!(s.tool_options()["active"], 1);
        pointer(&mut s, PointerKind::Down, 300.0, 100.0);
        if drag {
            pointer(&mut s, PointerKind::Drag, 350.0, 150.0);
        }
        assert!(s.tool_busy());
        s.execute("artboard.setProps", &json!({"index":1,"name":"Second renamed"})).unwrap();
        assert_eq!(s.tool_options()["active"], 1, "the recreated tool must keep its document's active artboard");
        assert!(s.prefs.tool_settings.get("artboard").is_none_or(|options| !options.contains_key("active")));
        s.tool_key(ToolKey::Delete, Mods::default(), ViewInfo::default()).unwrap();
        assert_eq!(s.doc().unwrap().doc.artboards, vec![first], "Delete removes the renamed second board, not the first");
        s.execute("edit.undo", &json!({})).unwrap();
        assert_eq!(s.doc().unwrap().doc.artboards[1].name, "Second renamed");
    }
}

fn assert_same_history(actual: &vectorcraft_engine::History, expected: &vectorcraft_engine::History) {
    assert_eq!(actual.limit, expected.limit);
    for (actual, expected) in [(&actual.undo, &expected.undo), (&actual.redo, &expected.redo)] {
        assert_eq!(actual.len(), expected.len());
        for (actual, expected) in actual.iter().zip(expected) {
            assert_eq!(actual.label, expected.label);
            assert!(std::sync::Arc::ptr_eq(&actual.doc, &expected.doc));
            assert_eq!(actual.selection, expected.selection);
        }
    }
}

#[test]
fn external_setprops_during_pen_scales_linked_masks_once_and_keeps_unlinked_masks() {
    for linked in [false, true] {
        let mut s = Session::new();
        s.execute("file.new", &json!({"width":800,"height":600,"created":1_000_000_000})).unwrap();
        let art = add_opacity_mask(&mut s, 40.0, linked);
        let (mut s, id, first_anchor, undo) = start_live_pen(s);
        let preview = s.doc().unwrap().clone();
        let expected = path(&s, id).transformed(Affine::scale(2.0));
        let out = s.execute("artboard.setProps", &json!({"index":0,"width":1600,"height":1200,"scaleArt":true})).unwrap();
        assert_eq!(out["ids"], json!([art.0, id.0]));
        assert_eq!(path(&s, id), expected);
        let st = s.doc().unwrap();
        assert_eq!(st.selection, preview.selection);
        assert_eq!(st.doc.artboards[0].rect, Rect::new(0.0, 0.0, 1600.0, 1200.0));
        let n = st.doc.node(art).unwrap();
        assert_eq!(n.geometric_bounds().unwrap(), Rect::new(20.0, 20.0, 40.0, 40.0));
        let mask = n.mask.as_ref().unwrap();
        assert_eq!(mask.linked, linked);
        let expected_mask = if linked { Rect::new(80.0, 40.0, 100.0, 60.0) } else { Rect::new(40.0, 20.0, 50.0, 30.0) };
        assert_eq!(mask.art.geometric_bounds().unwrap(), expected_mask);
        assert_eq!(st.history.undo.len(), undo + 2);
        assert_eq!(st.history.undo.last().unwrap().label, "Artboard Options");
        assert!(!s.in_interaction() && !s.tool_busy());
        let edited = st.doc.clone();
        pointer(&mut s, PointerKind::Drag, 250.0, 350.0);
        pointer(&mut s, PointerKind::Up, 250.0, 350.0);
        assert_eq!(serde_json::to_value(&*s.doc().unwrap().doc).unwrap(), serde_json::to_value(&*edited).unwrap());
        assert_eq!(s.doc().unwrap().history.undo.len(), undo + 2);
        let mut replay = Session::new();
        for (command, params) in &s.journal {
            replay.execute(command, params).unwrap();
        }
        assert_eq!(path(&replay, id), expected);
        assert_eq!(replay.doc().unwrap().doc.node(art).unwrap().mask, edited.node(art).unwrap().mask);
        assert_eq!(replay.doc().unwrap().doc.artboards, edited.artboards);
        s.execute("edit.undo", &json!({})).unwrap();
        assert_eq!(serde_json::to_value(&*s.doc().unwrap().doc).unwrap(), serde_json::to_value(&*preview.doc).unwrap());
        assert_eq!(s.doc().unwrap().selection, preview.selection);
        s.execute("edit.undo", &json!({})).unwrap();
        assert_eq!(path(&s, id), first_anchor);
        s.execute("edit.redo", &json!({})).unwrap();
        s.execute("edit.redo", &json!({})).unwrap();
        assert_eq!(serde_json::to_value(&*s.doc().unwrap().doc).unwrap(), serde_json::to_value(&*edited).unwrap());
    }
}
