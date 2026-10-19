//! Small synthetic coverage of optional artwork scaling with an artboard (#602).
use crate::{Session, ViewInfo};
use serde_json::json;
use vectorcraft_doc::NodeId;
use vectorcraft_geom::{Affine, Point, Rect};

fn session() -> Session {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 100, "height": 100})).unwrap();
    s
}
fn rect(s: &mut Session, x: f64, y: f64) -> NodeId {
    NodeId(s.execute("shape.rectangle", &json!({"x":x,"y":y,"width":10,"height":10})).unwrap()["id"].as_u64().unwrap())
}
fn bounds(s: &Session, id: NodeId) -> Rect {
    s.doc().unwrap().doc.node(id).unwrap().geometric_bounds().unwrap()
}
fn close(a: Rect, b: Rect) {
    for (a, b) in [(a.x0, b.x0), (a.y0, b.y0), (a.x1, b.x1), (a.y1, b.y1)] {
        assert!((a - b).abs() < 1e-8, "{a} != {b}");
    }
}
#[test]
fn optional_scale_art_is_off_by_default_and_returns_only_contained_art_when_on() {
    let mut s = session();
    let inside = rect(&mut s, 10.0, 20.0);
    let outside = rect(&mut s, 95.0, 20.0);
    let original = bounds(&s, inside);
    let untouched = bounds(&s, outside);
    s.execute("artboard.setProps", &json!({"index":0,"width":200,"height":200})).unwrap();
    assert_eq!(bounds(&s, inside), original);
    s.execute("edit.undo", &json!({})).unwrap();
    let out = s.execute("artboard.setProps", &json!({"index":0,"x":30,"y":40,"width":200,"height":300,"scaleArt":true})).unwrap();
    assert_eq!(out["ids"], json!([inside.0]));
    close(bounds(&s, inside), Rect::new(50.0, 100.0, 70.0, 130.0));
    assert_eq!(bounds(&s, outside), untouched);
}
#[test]
fn all_nine_reference_anchors_keep_their_page_position_on_resize() {
    for reference in 0..9 {
        let mut s = session();
        let id = rect(&mut s, 10.0, 20.0);
        let old = s.doc().unwrap().doc.artboards[0].rect;
        let anchor = vectorcraft_geom::reference_point(old, reference);
        s.execute("artboard.setProps", &json!({"index":0,"width":200,"height":300,"reference":reference,"scaleArt":true})).unwrap();
        let now = s.doc().unwrap().doc.artboards[0].rect;
        assert_eq!(vectorcraft_geom::reference_point(now, reference), anchor);
        let top = Point::new(anchor.x + (10.0 - anchor.x) * 2.0, anchor.y + (20.0 - anchor.y) * 3.0);
        close(bounds(&s, id), Rect::new(top.x, top.y, top.x + 20.0, top.y + 30.0));
    }
}
#[test]
fn locked_and_hidden_art_follow_the_existing_artboard_policy() {
    for include in [false, true] {
        let mut s = session();
        let locked = rect(&mut s, 10.0, 10.0);
        let hidden = rect(&mut s, 40.0, 10.0);
        let d = std::sync::Arc::make_mut(&mut s.doc_mut().unwrap().doc);
        d.node_mut(locked).unwrap().locked = true;
        d.node_mut(hidden).unwrap().visible = false;
        s.prefs.move_locked_with_artboard = include;
        let out = s.execute("artboard.setProps", &json!({"index":0,"width":200,"height":200,"scaleArt":true})).unwrap();
        if include {
            assert_eq!(out["ids"].as_array().unwrap().len(), 2);
            close(bounds(&s, locked), Rect::new(20.0, 20.0, 40.0, 40.0));
            close(bounds(&s, hidden), Rect::new(80.0, 20.0, 100.0, 40.0));
        } else {
            assert_eq!(out["ids"], json!([]));
            close(bounds(&s, locked), Rect::new(10.0, 10.0, 20.0, 20.0));
            close(bounds(&s, hidden), Rect::new(40.0, 10.0, 50.0, 20.0));
        }
    }
}
#[test]
fn guides_artboard_and_art_are_one_undo_without_changing_selection() {
    let mut s = session();
    let id = rect(&mut s, 10.0, 20.0);
    for (vertical, pos, board) in [(true, 25, true), (false, 30, true), (true, 45, false)] {
        let mut p = json!({"vertical":vertical,"pos":pos});
        if board {
            p["artboard"] = json!(0);
        }
        s.execute("guide.add", &p).unwrap();
    }
    let before = s.doc().unwrap().doc.clone();
    let selection = s.doc().unwrap().selection.clone();
    let undo = s.doc().unwrap().history.undo.len();
    s.execute("artboard.setProps", &json!({"index":0,"x":20,"y":30,"width":200,"height":300,"scaleArt":true})).unwrap();
    let d = s.doc().unwrap();
    assert_eq!(d.history.undo.len(), undo + 1);
    assert_eq!(d.selection.objects, selection.objects);
    assert_eq!(d.doc.guides.iter().map(|g| g.pos).collect::<Vec<_>>(), [70.0, 120.0, 45.0]);
    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(s.doc().unwrap().doc.artboards, before.artboards);
    assert_eq!(s.doc().unwrap().doc.guides, before.guides);
    close(bounds(&s, id), Rect::new(10.0, 20.0, 20.0, 30.0));
}
#[test]
fn scale_strokes_effects_and_corners_match_the_scale_command_preferences() {
    for strokes in [false, true] {
        for corners in [false, true] {
            let setup = || {
                let mut s = session();
                let id =
                    NodeId(s.execute("shape.rectangle", &json!({"x":10,"y":10,"width":40,"height":30,"radius":5})).unwrap()["id"].as_u64().unwrap());
                s.execute("stroke.set", &json!({"weight":4,"dash":[6,2],"dashOffset":3})).unwrap();
                s.execute("effect.apply", &json!({"effect":"stylize.dropShadow","params":{"x":7,"y":5}})).unwrap();
                s.prefs.scale_strokes = strokes;
                s.prefs.scale_corners = corners;
                (s, id)
            };
            let (mut board, id) = setup();
            let (mut scale, other) = setup();
            board.execute("artboard.setProps", &json!({"index":0,"width":200,"height":200,"scaleArt":true})).unwrap();
            scale.execute("object.scale", &json!({"sx":200,"origin":[0,0]})).unwrap();
            let (a, b) = (board.doc().unwrap().doc.node(id).unwrap(), scale.doc().unwrap().doc.node(other).unwrap());
            assert_eq!(a.kind, b.kind);
            assert_eq!(a.appearance, b.appearance);
        }
    }
}
#[test]
fn repeated_tool_resize_previews_scale_once_and_undo_together() {
    use vectorcraft_tools::{PointerEvent, PointerKind};
    let mut s = session();
    let id = rect(&mut s, 10.0, 20.0);
    s.select_tool("artboard", ViewInfo::default()).unwrap();
    s.set_tool_option("scaleArt", &json!(true));
    let undo = s.doc().unwrap().history.undo.len();
    for (kind, x, y) in
        [(PointerKind::Down, 100.0, 100.0), (PointerKind::Drag, 150.0, 150.0), (PointerKind::Drag, 200.0, 200.0), (PointerKind::Up, 200.0, 200.0)]
    {
        s.pointer(&PointerEvent::new(kind, x, y), ViewInfo::default()).unwrap();
    }
    close(bounds(&s, id), Rect::new(20.0, 40.0, 40.0, 60.0));
    assert_eq!(s.doc().unwrap().history.undo.len(), undo + 1);
    s.execute("edit.undo", &json!({})).unwrap();
    close(bounds(&s, id), Rect::new(10.0, 20.0, 20.0, 30.0));
    assert_eq!(s.doc().unwrap().doc.artboards[0].rect, Rect::new(0.0, 0.0, 100.0, 100.0));
}
#[test]
fn bad_scaling_options_and_out_of_canvas_results_do_not_edit_the_document() {
    let mut s = session();
    let id = rect(&mut s, 10.0, 20.0);
    let undo = s.doc().unwrap().history.undo.len();
    for p in [
        json!({"index":0,"width":200,"scaleArt":"yes"}),
        json!({"index":0,"width":200,"reference":9,"scaleArt":true}),
        json!({"index":0,"width":1e50,"scaleArt":true}),
    ] {
        assert!(s.execute("artboard.setProps", &p).is_err());
        assert_eq!(s.doc().unwrap().history.undo.len(), undo);
        close(bounds(&s, id), Rect::new(10.0, 20.0, 20.0, 30.0));
        assert_eq!(s.doc().unwrap().doc.artboards[0].rect, Rect::new(0.0, 0.0, 100.0, 100.0));
    }
}

#[test]
fn clipped_descendants_outside_the_mask_must_stay_in_canvas_on_artboard_scale() {
    for selected in [false, true] {
        for invalid in [false, true] {
            let mut s = session();
            let x = if invalid { 2_000_010.0 } else { 1_000_010.0 };
            // Three tiny nodes, all initially valid. The clip's in-board mask bounds hide this
            // off-mask child, whose coordinates would exceed the canvas after a twofold scale.
            let child = rect(&mut s, x, 20.0);
            let mask = rect(&mut s, 10.0, 10.0);
            s.execute("select.set", &json!({"ids":[child.0,mask.0]})).unwrap();
            let group = NodeId(s.execute("object.clippingMask.make", &json!({})).unwrap()["id"].as_u64().unwrap());
            s.execute("select.set", &json!({"ids":if selected { vec![group.0] } else { vec![] }})).unwrap();
            let before = s.doc().unwrap().doc.clone();
            let selection = s.doc().unwrap().selection.clone();
            let undo = s.doc().unwrap().history.undo.len();
            let revision = s.doc().unwrap().revision;
            let journal = s.journal.clone();
            let out = s.execute("artboard.setProps", &json!({"index":0,"width":200,"height":200,"scaleArt":true}));
            assert_eq!(s.doc().unwrap().selection, selection);
            if invalid {
                assert!(out.is_err(), "a clipped descendant must be checked even with empty selection");
                assert_eq!(serde_json::to_value(&*s.doc().unwrap().doc).unwrap(), serde_json::to_value(&*before).unwrap());
                assert_eq!(s.doc().unwrap().history.undo.len(), undo);
                assert_eq!(s.doc().unwrap().revision, revision);
                assert_eq!(s.journal, journal);
            } else {
                assert_eq!(out.unwrap()["ids"], json!([group.0]));
                close(bounds(&s, child), Rect::new(x * 2.0, 40.0, (x + 10.0) * 2.0, 60.0));
                assert_eq!(s.doc().unwrap().history.undo.len(), undo + 1);
                s.execute("edit.undo", &json!({})).unwrap();
                assert_eq!(serde_json::to_value(&*s.doc().unwrap().doc).unwrap(), serde_json::to_value(&*before).unwrap());
            }
        }
    }
}

fn opacity_mask_fixture(mask_x: f64, linked: bool, selected: bool) -> (Session, NodeId) {
    let mut s = session();
    // Two tiny rectangles and their layer; mask art leaves the layer tree when attached.
    let art = rect(&mut s, 10.0, 10.0);
    let mask = rect(&mut s, mask_x, 20.0);
    s.execute("transparency.makeOpacityMask", &json!({"ids":[art.0,mask.0],"clip":false})).unwrap();
    if !linked {
        s.execute("transparency.unlinkOpacityMask", &json!({"id":art.0})).unwrap();
    }
    s.execute("select.set", &json!({"ids":if selected { vec![art.0] } else { vec![] }})).unwrap();
    (s, art)
}

fn opacity_mask_bounds(s: &Session, id: NodeId) -> Rect {
    s.doc().unwrap().doc.node(id).unwrap().mask.as_ref().unwrap().art.geometric_bounds().unwrap()
}

#[test]
fn linked_opacity_mask_outside_canvas_rejects_artboard_scale() {
    for selected in [false, true] {
        let (mut s, id) = opacity_mask_fixture(2_000_010.0, true, selected);
        assert!(opacity_mask_bounds(&s, id).x1 <= crate::MAX_COORD, "the original mask is valid");
        // A populated redo stack must survive rejection too.
        s.execute("artboard.setProps", &json!({"index":0,"name":"Temporary"})).unwrap();
        s.execute("edit.undo", &json!({})).unwrap();
        let before = s.doc().unwrap().clone();
        assert!(!before.history.redo.is_empty());
        let journal = s.journal.clone();
        let out = s.execute("artboard.setProps", &json!({"index":0,"width":200,"height":200,"scaleArt":true}));
        assert!(
            matches!(out, Err(crate::EngineError::Other(ref message)) if message == "scaled artwork would exceed the canvas (coordinates out of range)"),
            "linked mask coordinates must be checked outside the layer tree"
        );
        let st = s.doc().unwrap();
        assert_eq!(serde_json::to_value(&*st.doc).unwrap(), serde_json::to_value(&*before.doc).unwrap());
        assert!(std::sync::Arc::ptr_eq(&st.doc, &before.doc));
        assert_eq!(st.selection, before.selection);
        assert_same_history(&st.history, &before.history);
        assert_eq!(st.revision, before.revision);
        assert_eq!(st.is_dirty(), before.is_dirty());
        assert_eq!(s.journal, journal);
        close(opacity_mask_bounds(&s, id), Rect::new(2_000_010.0, 20.0, 2_000_020.0, 30.0));
        // Redo still performs the pre-existing page edit, not the rejected scale.
        s.execute("edit.redo", &json!({})).unwrap();
        assert_eq!(s.doc().unwrap().doc.artboards[0].name, "Temporary");
        assert_eq!(s.doc().unwrap().doc.artboards[0].rect, Rect::new(0.0, 0.0, 100.0, 100.0));
    }
}

#[test]
fn valid_linked_opacity_mask_scales_once_and_undoes_with_artboard() {
    for selected in [false, true] {
        let (mut s, id) = opacity_mask_fixture(1_000_010.0, true, selected);
        let before = s.doc().unwrap().clone();
        let out = s.execute("artboard.setProps", &json!({"index":0,"width":200,"height":200,"scaleArt":true})).unwrap();
        assert_eq!(out["ids"], json!([id.0]));
        assert_eq!(s.doc().unwrap().selection, before.selection);
        close(bounds(&s, id), Rect::new(20.0, 20.0, 40.0, 40.0));
        close(opacity_mask_bounds(&s, id), Rect::new(2_000_020.0, 40.0, 2_000_040.0, 60.0));
        assert_eq!(s.doc().unwrap().history.undo.len(), before.history.undo.len() + 1);
        assert_eq!(s.doc().unwrap().history.undo.last().unwrap().label, "Artboard Options");
        s.execute("edit.undo", &json!({})).unwrap();
        assert_eq!(serde_json::to_value(&*s.doc().unwrap().doc).unwrap(), serde_json::to_value(&*before.doc).unwrap());
        assert_eq!(s.doc().unwrap().selection, before.selection);
        s.execute("edit.redo", &json!({})).unwrap();
        close(opacity_mask_bounds(&s, id), Rect::new(2_000_020.0, 40.0, 2_000_040.0, 60.0));
    }
}

#[test]
fn unlinked_opacity_mask_stays_put_without_blocking_artboard_scale() {
    for selected in [false, true] {
        let (mut s, id) = opacity_mask_fixture(2_000_010.0, false, selected);
        let before = s.doc().unwrap().clone();
        let mask = s.doc().unwrap().doc.node(id).unwrap().mask.clone();
        let out = s.execute("artboard.setProps", &json!({"index":0,"width":200,"height":200,"scaleArt":true})).unwrap();
        assert_eq!(out["ids"], json!([id.0]));
        close(bounds(&s, id), Rect::new(20.0, 20.0, 40.0, 40.0));
        assert_eq!(s.doc().unwrap().doc.node(id).unwrap().mask, mask, "an unlinked mask is not transformed");
        assert_eq!(s.doc().unwrap().selection, before.selection);
        assert_eq!(s.doc().unwrap().history.undo.len(), before.history.undo.len() + 1);
        s.execute("edit.undo", &json!({})).unwrap();
        assert_eq!(serde_json::to_value(&*s.doc().unwrap().doc).unwrap(), serde_json::to_value(&*before.doc).unwrap());
        assert_eq!(s.doc().unwrap().selection, before.selection);
    }
}

fn assert_same_history(actual: &crate::History, expected: &crate::History) {
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
fn clipped_opacity_mask_descendants_follow_linking_on_artboard_scale() {
    for selected in [false, true] {
        for linked in [false, true] {
            for invalid in [false, true] {
                let mut s = session();
                let art = rect(&mut s, 10.0, 10.0);
                let x = if invalid { 2_000_010.0 } else { 40.0 };
                let child = rect(&mut s, x, 40.0);
                let clip = rect(&mut s, 30.0, 20.0);
                s.execute("select.set", &json!({"ids":[child.0,clip.0]})).unwrap();
                let mask = s.execute("object.clippingMask.make", &json!({})).unwrap()["id"].clone();
                s.execute("transparency.makeOpacityMask", &json!({"ids":[art.0,mask],"clip":false})).unwrap();
                if !linked {
                    s.execute("transparency.unlinkOpacityMask", &json!({"id":art.0})).unwrap();
                }
                s.execute("select.set", &json!({"ids":if selected { vec![art.0] } else { vec![] }})).unwrap();
                let before = s.doc().unwrap().clone();
                let journal = s.journal.clone();
                // The opacity mask's clip bounds are small even when its child is far away.
                close(opacity_mask_bounds(&s, art), Rect::new(30.0, 20.0, 40.0, 30.0));
                let out = s.execute("artboard.setProps", &json!({"index":0,"width":200,"height":200,"scaleArt":true}));
                let st = s.doc().unwrap();
                assert_eq!(st.selection, before.selection);
                if linked && invalid {
                    assert!(
                        matches!(out, Err(crate::EngineError::Other(ref message)) if message == "scaled artwork would exceed the canvas (coordinates out of range)"),
                        "clip bounds must not hide transformed mask descendants"
                    );
                    assert!(std::sync::Arc::ptr_eq(&st.doc, &before.doc));
                    assert_same_history(&st.history, &before.history);
                    assert_eq!(st.revision, before.revision);
                    assert_eq!(st.is_dirty(), before.is_dirty());
                    assert_eq!(s.journal, journal);
                } else {
                    assert_eq!(out.unwrap()["ids"], json!([art.0]));
                    close(bounds(&s, art), Rect::new(20.0, 20.0, 40.0, 40.0));
                    let mask = &st.doc.node(art).unwrap().mask.as_ref().unwrap().art;
                    let child = mask.children().unwrap().iter().find(|n| n.id == child).unwrap();
                    let k = if linked { 2.0 } else { 1.0 };
                    close(child.geometric_bounds().unwrap(), Rect::new(x * k, 40.0 * k, (x + 10.0) * k, 50.0 * k));
                    close(opacity_mask_bounds(&s, art), Rect::new(30.0 * k, 20.0 * k, 40.0 * k, 30.0 * k));
                    assert_eq!(st.history.undo.len(), before.history.undo.len() + 1);
                    let edited = st.doc.clone();
                    s.execute("edit.undo", &json!({})).unwrap();
                    assert_eq!(serde_json::to_value(&*s.doc().unwrap().doc).unwrap(), serde_json::to_value(&*before.doc).unwrap());
                    s.execute("edit.redo", &json!({})).unwrap();
                    assert_eq!(serde_json::to_value(&*s.doc().unwrap().doc).unwrap(), serde_json::to_value(&*edited).unwrap());
                }
            }
        }
    }
}

#[test]
fn grouped_art_with_nested_opacity_masks_checks_only_transformed_links() {
    for selected in [false, true] {
        for outer_linked in [false, true] {
            for inner_linked in [false, true] {
                for invalid in [false, true] {
                    let mut s = session();
                    let art = rect(&mut s, 10.0, 10.0);
                    let mask = rect(&mut s, 30.0, 20.0);
                    let x = if invalid { 2_000_010.0 } else { 40.0 };
                    let inner = rect(&mut s, x, 40.0);
                    s.execute("transparency.makeOpacityMask", &json!({"ids":[mask.0,inner.0],"clip":false})).unwrap();
                    if !inner_linked {
                        s.execute("transparency.unlinkOpacityMask", &json!({"id":mask.0})).unwrap();
                    }
                    s.execute("transparency.makeOpacityMask", &json!({"ids":[art.0,mask.0],"clip":false})).unwrap();
                    if !outer_linked {
                        s.execute("transparency.unlinkOpacityMask", &json!({"id":art.0})).unwrap();
                    }
                    let sibling = rect(&mut s, 60.0, 60.0);
                    s.execute("select.set", &json!({"ids":[art.0,sibling.0]})).unwrap();
                    let group = s.execute("object.group", &json!({})).unwrap()["id"].clone();
                    s.execute("select.set", &json!({"ids":if selected { vec![group.clone()] } else { vec![] }})).unwrap();
                    let before = s.doc().unwrap().clone();
                    let journal = s.journal.clone();
                    let out = s.execute("artboard.setProps", &json!({"index":0,"width":200,"height":200,"scaleArt":true}));
                    let st = s.doc().unwrap();
                    assert_eq!(st.selection, before.selection);
                    if outer_linked && inner_linked && invalid {
                        assert!(
                            matches!(out, Err(crate::EngineError::Other(ref message)) if message == "scaled artwork would exceed the canvas (coordinates out of range)"),
                            "a child's nested linked mask must be validated"
                        );
                        assert!(std::sync::Arc::ptr_eq(&st.doc, &before.doc));
                        assert_same_history(&st.history, &before.history);
                        assert_eq!(st.revision, before.revision);
                        assert_eq!(st.is_dirty(), before.is_dirty());
                        assert_eq!(s.journal, journal);
                    } else {
                        assert_eq!(out.unwrap()["ids"], json!([group]));
                        close(bounds(&s, art), Rect::new(20.0, 20.0, 40.0, 40.0));
                        close(bounds(&s, sibling), Rect::new(120.0, 120.0, 140.0, 140.0));
                        let k = if outer_linked { 2.0 } else { 1.0 };
                        close(opacity_mask_bounds(&s, art), Rect::new(30.0 * k, 20.0 * k, 40.0 * k, 30.0 * k));
                        let inner = &st.doc.node(art).unwrap().mask.as_ref().unwrap().art.mask.as_ref().unwrap().art;
                        let k = if outer_linked && inner_linked { 2.0 } else { 1.0 };
                        close(inner.geometric_bounds().unwrap(), Rect::new(x * k, 40.0 * k, (x + 10.0) * k, 50.0 * k));
                        assert_eq!(st.history.undo.len(), before.history.undo.len() + 1);
                        let edited = st.doc.clone();
                        s.execute("edit.undo", &json!({})).unwrap();
                        assert_eq!(serde_json::to_value(&*s.doc().unwrap().doc).unwrap(), serde_json::to_value(&*before.doc).unwrap());
                        s.execute("edit.redo", &json!({})).unwrap();
                        assert_eq!(serde_json::to_value(&*s.doc().unwrap().doc).unwrap(), serde_json::to_value(&*edited).unwrap());
                    }
                }
            }
        }
    }
}

fn mask_pattern_fixture(linked: bool, grouped: bool) -> (Session, NodeId, NodeId) {
    let mut s = session();
    // Three original 10x10 rectangles; the tile has one detached copy in its pattern definition.
    // Create all scene geometry before setting the mask paint, which also changes paint defaults.
    let tile = rect(&mut s, 0.0, 0.0);
    let art = rect(&mut s, 10.0, 10.0);
    let mask = rect(&mut s, 40.0, 20.0);
    s.execute("select.set", &json!({"ids":[tile.0]})).unwrap();
    s.execute("object.pattern.make", &json!({"name":"Crossreview Mask Tile","width":12,"height":12,"edit":false})).unwrap();
    s.execute("edit.clear", &json!({"ids":[tile.0]})).unwrap();
    s.execute("paint.setFill", &json!({"ids":[mask.0],"swatch":"Crossreview Mask Tile"})).unwrap();
    s.execute("transparency.makeOpacityMask", &json!({"ids":[art.0,mask.0],"clip":false})).unwrap();
    if !linked {
        s.execute("transparency.unlinkOpacityMask", &json!({"id":art.0})).unwrap();
    }
    let target = if grouped { NodeId(s.execute("object.group", &json!({})).unwrap()["id"].as_u64().unwrap()) } else { art };
    assert!(!matches!(s.doc().unwrap().doc.node(art).unwrap().appearance.fill_paint(), vectorcraft_color::Paint::Pattern { .. }));
    assert_eq!(s.doc().unwrap().doc.pattern("Crossreview Mask Tile").unwrap().art.len(), 1);
    (s, art, target)
}

fn mask_pattern_placement(s: &Session, art: NodeId) -> Affine {
    let mask = &s.doc().unwrap().doc.node(art).unwrap().mask.as_ref().unwrap().art;
    match mask.appearance.fill_paint() {
        vectorcraft_color::Paint::Pattern { pattern, xf, .. } => {
            assert_eq!(pattern, "Crossreview Mask Tile");
            xf
        }
        paint => panic!("the mask fixture must keep its pattern paint: {paint:?}"),
    }
}

fn replay_with_pattern_preference(s: &Session, patterns: bool) -> Session {
    let mut replay = Session::new();
    replay.prefs.transform_pattern_tiles = patterns;
    for (command, params) in &s.journal {
        replay.execute(command, params).unwrap();
    }
    replay
}

#[test]
fn artboard_scale_linked_mask_pattern_replay_preserves_loaded_preference() {
    for grouped in [false, true] {
        for patterns in [true, false] {
            for selected in [false, true] {
                let (mut s, art, target) = mask_pattern_fixture(true, grouped);
                s.execute("select.set", &json!({"ids":if selected { vec![target.0] } else { vec![] }})).unwrap();
                // Model preferences loaded before the session, without journaling a prefs.set.
                s.prefs.transform_pattern_tiles = patterns;
                let before = s.doc().unwrap().clone();
                let placement = mask_pattern_placement(&s, art);
                let expected = if patterns { Affine::scale(2.0) * placement } else { placement };
                let out = s.execute("artboard.setProps", &json!({"index":0,"width":200,"height":200,"scaleArt":true})).unwrap();
                assert_eq!(out["ids"], json!([target.0]));
                assert_eq!(mask_pattern_placement(&s, art), expected);
                close(bounds(&s, art), Rect::new(20.0, 20.0, 40.0, 40.0));
                close(opacity_mask_bounds(&s, art), Rect::new(80.0, 40.0, 100.0, 60.0));
                assert_eq!(s.doc().unwrap().selection, before.selection);
                assert_eq!(s.doc().unwrap().history.undo.len(), before.history.undo.len() + 1);
                let edited = s.doc().unwrap().doc.clone();
                let replay = replay_with_pattern_preference(&s, !patterns);
                // Test the observable placement first: a missing journal option changes the mask.
                assert_eq!(
                    mask_pattern_placement(&replay, art),
                    expected,
                    "linked-mask pattern placement must survive replay under opposite preferences"
                );
                assert_eq!(serde_json::to_value(&*replay.doc().unwrap().doc).unwrap(), serde_json::to_value(&*edited).unwrap());
                assert_eq!(s.journal.last().unwrap().0, "artboard.setProps");
                assert_eq!(s.journal.last().unwrap().1["patterns"], json!(patterns));
                assert_eq!(s.doc().unwrap().doc.patterns, before.doc.patterns, "scaling changes placement, not the original pattern tile");
                s.execute("edit.undo", &json!({})).unwrap();
                assert_eq!(serde_json::to_value(&*s.doc().unwrap().doc).unwrap(), serde_json::to_value(&*before.doc).unwrap());
                s.execute("edit.redo", &json!({})).unwrap();
                assert_eq!(serde_json::to_value(&*s.doc().unwrap().doc).unwrap(), serde_json::to_value(&*edited).unwrap());
            }
        }
    }
}

#[test]
fn artboard_scale_unlinked_mask_pattern_stays_put_and_replays() {
    for grouped in [false, true] {
        for patterns in [true, false] {
            let (mut s, art, target) = mask_pattern_fixture(false, grouped);
            s.execute("select.set", &json!({"ids":[]})).unwrap();
            s.prefs.transform_pattern_tiles = patterns;
            let before = s.doc().unwrap().clone();
            let mask = before.doc.node(art).unwrap().mask.clone();
            let out = s.execute("artboard.setProps", &json!({"index":0,"width":200,"height":200,"scaleArt":true})).unwrap();
            assert_eq!(out["ids"], json!([target.0]));
            close(bounds(&s, art), Rect::new(20.0, 20.0, 40.0, 40.0));
            assert_eq!(s.doc().unwrap().doc.node(art).unwrap().mask, mask, "unlinked mask art and pattern placement stay unchanged");
            assert_eq!(s.doc().unwrap().selection, before.selection);
            assert_eq!(s.doc().unwrap().history.undo.len(), before.history.undo.len() + 1);
            assert!(s.journal.last().unwrap().1.get("patterns").is_none(), "only an untransformed mask uses a pattern");
            let edited = s.doc().unwrap().doc.clone();
            let replay = replay_with_pattern_preference(&s, !patterns);
            assert_eq!(serde_json::to_value(&*replay.doc().unwrap().doc).unwrap(), serde_json::to_value(&*edited).unwrap());
            s.execute("edit.undo", &json!({})).unwrap();
            assert_eq!(serde_json::to_value(&*s.doc().unwrap().doc).unwrap(), serde_json::to_value(&*before.doc).unwrap());
            s.execute("edit.redo", &json!({})).unwrap();
            assert_eq!(serde_json::to_value(&*s.doc().unwrap().doc).unwrap(), serde_json::to_value(&*edited).unwrap());
        }
    }
}
