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

fn active_delete_view() -> ViewInfo {
    ViewInfo { smart_guides: false, ..ViewInfo::default() }
}

fn active_delete_scene(boards: usize) -> Session {
    let mut s = session();
    // Original small boards, spaced far enough apart to pick their centres and corners.
    for index in 1..boards {
        s.execute("artboard.new", &json!({"x":200 * index,"y":0,"width":100,"height":100})).unwrap();
    }
    s
}

fn start_active_delete_gesture(s: &mut Session, active: usize, resize: bool, drag: bool) -> Point {
    use vectorcraft_tools::{PointerEvent, PointerKind};
    let view = active_delete_view();
    let board = s.doc().unwrap().doc.artboards[active].rect;
    let centre = board.center();
    s.select_tool("artboard", view).unwrap();
    s.set_tool_option("moveArt", &json!(false));
    s.set_tool_option("scaleArt", &json!(true));
    for kind in [PointerKind::Down, PointerKind::Up] {
        s.pointer(&PointerEvent::new(kind, centre.x, centre.y), view).unwrap();
    }
    assert_eq!(s.tool_options()["active"], json!(active));
    let at = if resize { Point::new(board.x1, board.y1) } else { centre };
    s.pointer(&PointerEvent::new(PointerKind::Down, at.x, at.y), view).unwrap();
    if drag {
        s.pointer(&PointerEvent::new(PointerKind::Drag, at.x + 20.0, at.y + 10.0), view).unwrap();
        assert_ne!(s.doc().unwrap().doc.artboards[active].rect, board, "the gesture must have a real preview");
    }
    assert!(s.tool_busy());
    assert_eq!(s.in_interaction(), drag);
    at
}

fn assert_active_delete_target(s: &Session, expected_id: u32, expected_index: usize) {
    let index = s.tool_options()["active"].as_u64().unwrap() as usize;
    let board = s.doc().unwrap().doc.artboards.get(index).expect("active artboard index must remain in range after external deletion");
    assert_eq!(board.id, expected_id, "a surviving active board must be followed by its stable ID");
    assert_eq!(index, expected_index);
}

fn external_active_delete_case(active: usize, deleted: usize, resize: bool, drag: bool) {
    use vectorcraft_tools::{Mods, Overlay, PointerEvent, PointerKind, ToolKey};
    let mut s = active_delete_scene(3);
    let at = start_active_delete_gesture(&mut s, active, resize, drag);
    let before = s.doc().unwrap().clone();
    let options = s.tool_options();
    let journal_len = s.journal.len();
    let mut expected = before.doc.artboards.clone();
    expected.remove(deleted);
    let expected_index = if deleted < active { active - 1 } else { active.min(expected.len() - 1) };
    let expected_id = if deleted == active { expected[expected_index].id } else { before.doc.artboards[active].id };

    // This is an external engine command, not the Artboard tool's Delete action.
    s.execute("artboard.delete", &json!({"index":deleted})).unwrap();
    assert_eq!(s.doc().unwrap().doc.artboards, expected, "deletion must keep the gesture's visible geometry");
    assert_active_delete_target(&s, expected_id, expected_index);
    assert_eq!(s.tool_id(), "artboard");
    assert_eq!(s.tool_options()["moveArt"], options["moveArt"]);
    assert_eq!(s.tool_options()["scaleArt"], options["scaleArt"]);
    assert!(!s.tool_busy() && !s.in_interaction());
    let steps = 1 + usize::from(drag);
    assert_eq!(s.doc().unwrap().history.undo.len(), before.history.undo.len() + steps);
    assert_eq!(s.doc().unwrap().history.undo.last().unwrap().label, "Delete Artboard");
    assert_eq!(s.journal.len(), journal_len + steps);
    assert_eq!(s.journal.last().unwrap(), &("artboard.delete".into(), json!({"index":deleted})));
    if drag {
        assert_eq!(s.journal[journal_len].0, if resize { "artboard.setProps" } else { "artboard.move" });
    }
    assert!(
        s.overlays(active_delete_view()).iter().any(|overlay| matches!(overlay, Overlay::Anchor { .. })),
        "the remaining active board needs resize handles"
    );

    let edited = s.doc().unwrap().clone();
    let finished_options = s.tool_options();
    let journal = s.journal.clone();
    for kind in [PointerKind::Drag, PointerKind::Up] {
        s.pointer(&PointerEvent::new(kind, at.x + 50.0, at.y + 30.0), active_delete_view()).unwrap();
    }
    assert_eq!(serde_json::to_value(&*s.doc().unwrap().doc).unwrap(), serde_json::to_value(&*edited.doc).unwrap());
    assert_same_history(&s.doc().unwrap().history, &edited.history);
    assert_eq!(s.tool_options(), finished_options);
    assert_eq!(s.journal, journal);
    assert!(!s.tool_busy() && !s.in_interaction());
    let mut replay = Session::new();
    for (command, params) in &s.journal {
        replay.execute(command, params).unwrap();
    }
    assert_eq!(serde_json::to_value(&*replay.doc().unwrap().doc).unwrap(), serde_json::to_value(&*edited.doc).unwrap());

    // The next ordinary Delete key must target that same remaining board.
    let mut after_key = expected;
    after_key.retain(|board| board.id != expected_id);
    s.tool_key(ToolKey::Delete, Mods::default(), active_delete_view()).unwrap();
    assert_eq!(s.doc().unwrap().doc.artboards, after_key);
    assert_active_delete_target(&s, after_key[0].id, 0);
}

#[test]
fn external_artboard_delete_before_active_preserves_stable_board_id() {
    for resize in [false, true] {
        for drag in [false, true] {
            // Removing board 0 shifts active board 1 to index 0. Keeping index 1 would
            // silently switch to a different surviving board, despite still being in range.
            external_active_delete_case(1, 0, resize, drag);
        }
    }
}

#[test]
fn external_artboard_delete_active_last_clamps_index_during_live_gesture() {
    for resize in [false, true] {
        for drag in [false, true] {
            external_active_delete_case(2, 2, resize, drag);
        }
    }
}

#[test]
fn rejected_external_artboard_delete_keeps_live_preview_options_and_history() {
    use vectorcraft_tools::{PointerEvent, PointerKind};
    for sole_board in [false, true] {
        for resize in [false, true] {
            let mut s = active_delete_scene(if sole_board { 1 } else { 3 });
            // Keep both undo and redo populated, including in the one-board scene.
            s.execute("artboard.setProps", &json!({"index":0,"name":"Kept"})).unwrap();
            s.execute("artboard.setProps", &json!({"index":0,"name":"Temporary"})).unwrap();
            s.execute("edit.undo", &json!({})).unwrap();
            let active = if sole_board { 0 } else { 1 };
            let at = start_active_delete_gesture(&mut s, active, resize, true);
            let before = s.doc().unwrap().clone();
            assert!(!before.history.undo.is_empty() && !before.history.redo.is_empty());
            let interaction = before.interaction.as_ref().unwrap();
            let options = s.tool_options();
            let prefs = s.prefs.clone();
            let journal = s.journal.clone();
            let index = if sole_board { 0 } else { 3 };
            let message = if sole_board { "a document needs at least one artboard" } else { "no such artboard" };

            let result = s.execute("artboard.delete", &json!({"index":index}));
            assert!(matches!(result, Err(crate::EngineError::Other(ref value)) if value == message));
            let st = s.doc().unwrap();
            assert_eq!(serde_json::to_value(&*st.doc).unwrap(), serde_json::to_value(&*before.doc).unwrap());
            assert!(std::sync::Arc::ptr_eq(&st.doc, &before.doc));
            assert_eq!(st.selection, before.selection);
            assert_same_history(&st.history, &before.history);
            assert_eq!(st.revision, before.revision);
            assert_eq!(st.is_dirty(), before.is_dirty());
            assert_eq!(st.active_layer, before.active_layer);
            assert_eq!(st.layer_rows, before.layer_rows);
            assert_eq!(st.isolation, before.isolation);
            assert_eq!(st.last_transform, before.last_transform);
            assert_eq!(st.last_perspective, before.last_perspective);
            let current = st.interaction.as_ref().expect("rejection must keep the live gesture");
            assert!(std::sync::Arc::ptr_eq(&current.doc, &interaction.doc));
            assert_eq!(current.label, interaction.label);
            assert_eq!(current.selection, interaction.selection);
            assert_eq!(current.preview, interaction.preview);
            assert_eq!(current.active_layer, interaction.active_layer);
            assert_eq!(current.layer_rows, interaction.layer_rows);
            assert_eq!(current.isolation, interaction.isolation);
            assert_eq!(current.perspective_again, interaction.perspective_again);
            assert_eq!(s.tool_id(), "artboard");
            assert_eq!(s.tool_options(), options);
            assert_eq!(s.prefs, prefs);
            assert_eq!(s.journal, journal);
            assert!(s.tool_busy() && s.in_interaction());

            // The same gesture must still be editable, then commit just its own preview.
            for kind in [PointerKind::Drag, PointerKind::Up] {
                s.pointer(&PointerEvent::new(kind, at.x + 50.0, at.y + 30.0), active_delete_view()).unwrap();
            }
            assert_ne!(s.doc().unwrap().doc.artboards[active].rect, before.doc.artboards[active].rect);
            assert_eq!(s.tool_options(), options);
            assert_eq!(s.doc().unwrap().history.undo.len(), before.history.undo.len() + 1);
            assert_eq!(&s.journal[..journal.len()], &journal);
            assert_eq!(s.journal.last().unwrap().0, if resize { "artboard.setProps" } else { "artboard.move" });
            assert!(!s.tool_busy() && !s.in_interaction());
            s.execute("edit.undo", &json!({})).unwrap();
            assert_eq!(serde_json::to_value(&*s.doc().unwrap().doc).unwrap(), serde_json::to_value(&*interaction.doc).unwrap());
        }
    }
}

fn locked_hidden_scale_replay_case(include: bool) {
    let mut s = session();
    let locked = rect(&mut s, 10.0, 10.0);
    let hidden = rect(&mut s, 40.0, 10.0);
    let visible = rect(&mut s, 70.0, 10.0);
    // Record the scene's flags through commands so replay starts from the same document.
    s.execute("object.setProps", &json!({"id":locked.0,"locked":true})).unwrap();
    s.execute("object.setProps", &json!({"id":hidden.0,"visible":false})).unwrap();
    assert!(s.doc().unwrap().doc.node(locked).unwrap().locked);
    assert!(!s.doc().unwrap().doc.node(hidden).unwrap().visible);
    assert!(!s.doc().unwrap().doc.node(visible).unwrap().locked);
    assert!(s.doc().unwrap().doc.node(visible).unwrap().visible);
    // Model a saved preference, independent of the recorded commands.
    s.prefs.move_locked_with_artboard = include;
    let before = s.doc().unwrap().clone();
    let before_json = serde_json::to_value(&*before.doc).unwrap();
    let out = s.execute("artboard.setProps", &json!({"index":0,"width":200,"height":200,"scaleArt":true})).unwrap();
    let expected_ids = if include { vec![locked.0, hidden.0, visible.0] } else { vec![visible.0] };
    assert_eq!(out["ids"], json!(expected_ids));
    let expected = [
        (locked, if include { Rect::new(20.0, 20.0, 40.0, 40.0) } else { Rect::new(10.0, 10.0, 20.0, 20.0) }),
        (hidden, if include { Rect::new(80.0, 20.0, 100.0, 40.0) } else { Rect::new(40.0, 10.0, 50.0, 20.0) }),
        (visible, Rect::new(140.0, 20.0, 160.0, 40.0)),
    ];
    for (id, expected_bounds) in expected {
        close(bounds(&s, id), expected_bounds);
    }
    assert_eq!(s.doc().unwrap().doc.artboards[0].rect, Rect::new(0.0, 0.0, 200.0, 200.0));
    assert_eq!(s.doc().unwrap().selection, before.selection);
    assert_eq!(s.doc().unwrap().history.undo.len(), before.history.undo.len() + 1);
    assert_eq!(s.doc().unwrap().history.undo.last().unwrap().label, "Artboard Options");
    assert!(!s.in_interaction());
    let edited_json = serde_json::to_value(&*s.doc().unwrap().doc).unwrap();
    let resize_journal = s.journal.clone();

    // The resize is one undo step; neither history direction changes saved preferences.
    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(serde_json::to_value(&*s.doc().unwrap().doc).unwrap(), before_json);
    assert_eq!(s.doc().unwrap().selection, before.selection);
    assert_eq!(s.doc().unwrap().history.undo.len(), before.history.undo.len());
    assert_eq!(s.doc().unwrap().history.redo.len(), 1);
    s.execute("edit.redo", &json!({})).unwrap();
    assert_eq!(serde_json::to_value(&*s.doc().unwrap().doc).unwrap(), edited_json);
    assert_eq!(s.doc().unwrap().selection, before.selection);
    assert_eq!(s.doc().unwrap().history.undo.len(), before.history.undo.len() + 1);
    assert!(s.doc().unwrap().history.redo.is_empty());
    assert_eq!(s.prefs.move_locked_with_artboard, include);

    // Check both a plain resize replay and a replay that includes Undo/Redo. Same preferences
    // are a positive control; the opposite saved preference must reproduce exactly the same art.
    for replay_include in [include, !include] {
        for journal in [&resize_journal, &s.journal] {
            let mut replay = Session::new();
            replay.prefs.move_locked_with_artboard = replay_include;
            for (command, params) in journal {
                replay.execute(command, params).unwrap();
            }
            close(bounds(&replay, visible), Rect::new(140.0, 20.0, 160.0, 40.0));
            for (id, expected_bounds) in expected {
                assert_eq!(
                    bounds(&replay, id),
                    expected_bounds,
                    "artboard scaling must replay the recorded inclusion policy: original={include}, replay={replay_include}, id={id:?}"
                );
            }
            assert_eq!(serde_json::to_value(&*replay.doc().unwrap().doc).unwrap(), edited_json);
            assert_eq!(replay.doc().unwrap().selection, before.selection);
            assert_eq!(replay.prefs.move_locked_with_artboard, replay_include, "replay must not overwrite saved preferences");
        }
    }
}

#[test]
fn artboard_scale_replay_preserves_locked_hidden_inclusion() {
    locked_hidden_scale_replay_case(true);
}

#[test]
fn artboard_scale_replay_preserves_locked_hidden_exclusion() {
    locked_hidden_scale_replay_case(false);
}
