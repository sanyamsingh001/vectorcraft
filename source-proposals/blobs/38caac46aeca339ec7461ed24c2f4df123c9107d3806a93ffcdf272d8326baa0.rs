//! Type and symbols in perspective (projected, still type and symbol instances, Edit Text),
//! attachment bookkeeping (copy, paste, duplicate, Selection-tool moves, release) and drawing in
//! perspective (flare, spiral, polar grid, click-to-size dialogs).

use serde_json::json;
use vectorcraft_doc::NodeKind;
use vectorcraft_geom::{Point, Rect};
use vectorcraft_tools::Mods;
use vectorcraft_tools::distort::perspective::{self as persp, Plane};

use super::tests_persp_select::{attached, drag, grid, persp_session, undo_len};
use super::*;

/// Point type "HELLO" (48 pt) at (430, 400), attached to the right plane.
fn persp_text(s: &mut Session) -> NodeId {
    let r = s.execute("text.create", &json!({"x": 430, "y": 400, "text": "HELLO", "size": 48})).unwrap();
    let id = NodeId(r["id"].as_u64().unwrap());
    s.execute("perspective.attach", &json!({"ids": [id.0], "plane": "right"})).unwrap();
    id
}

fn node(s: &Session, id: NodeId) -> vectorcraft_doc::Node {
    s.doc().unwrap().doc.node(id).unwrap().clone()
}

/// The art the renderer and exporters draw for `id` (its outlines, projected).
fn drawn_bounds(s: &Session, id: NodeId) -> Rect {
    let n = node(s, id);
    vectorcraft_render::effects::reshape(&n, None).unwrap().geometric_bounds().unwrap()
}

#[test]
fn type_in_perspective_is_projected_and_stays_type() {
    let (mut s, _) = persp_session();
    let id = persp_text(&mut s);
    let n = node(&s, id);
    assert!(matches!(n.kind, NodeKind::Text(_)), "still type");
    assert_eq!(attached(&s, id), Some((Plane::Right, 0.0)));
    let h = n.projection().expect("drawn through a projection");
    // Really foreshortened: the right plane recedes to the right, so the letters' far end is
    // shorter than their near end.
    let out = vectorcraft_render::effects::reshape(&n, None).unwrap();
    let mut pts = vec![];
    out.walk(&mut |c| {
        if let Some(p) = c.path_data() {
            pts.extend(p.anchors().map(|(_, _, a)| a.p));
        }
    });
    let b = drawn_bounds(&s, id);
    let near: Vec<&Point> = pts.iter().filter(|p| p.x < b.x0 + 0.2 * b.width()).collect();
    let far: Vec<&Point> = pts.iter().filter(|p| p.x > b.x1 - 0.2 * b.width()).collect();
    let span = |v: &[&Point]| v.iter().map(|p| p.y).fold(f64::MIN, f64::max) - v.iter().map(|p| p.y).fold(f64::MAX, f64::min);
    assert!(span(&far) < span(&near) * 0.97, "far {} near {}", span(&far), span(&near));
    // Bounds, hit testing and the selection box follow the drawn type.
    let gb = n.geometric_bounds().unwrap();
    assert!((gb.x0 - b.x0).abs() < 12.0 && (gb.x1 - b.x1).abs() < 12.0, "{gb:?} {b:?}");
    let hit = vectorcraft_doc::hit::hit_test(&s.doc().unwrap().doc, b.center(), Default::default()).unwrap();
    assert_eq!(hit.leaf, id);
    let _ = h;
}

#[test]
fn type_in_perspective_exports_projected_outlines() {
    let (mut s, _) = persp_session();
    let id = persp_text(&mut s);
    let b = drawn_bounds(&s, id);
    let doc = s.doc().unwrap().doc.clone();
    let svg = vectorcraft_svg::export(&doc, &Default::default());
    assert!(!svg.contains("HELLO"), "no flat text in the SVG");
    let pdf = vectorcraft_render::effects::bake_document(&doc).unwrap();
    let baked = pdf.node(id).unwrap();
    assert!(!matches!(baked.kind, NodeKind::Text(_)));
    let bb = baked.geometric_bounds().unwrap();
    assert!((bb.x0 - b.x0).abs() < 1e-6 && (bb.y1 - b.y1).abs() < 1e-6, "{bb:?} {b:?}");
}

#[test]
fn type_in_perspective_in_a_group_with_a_geometry_effect_exports_projected() {
    let (mut s, rect) = persp_session();
    let id = persp_text(&mut s);
    let b = drawn_bounds(&s, id);
    s.execute("select.set", &json!({"ids": [rect.0, id.0]})).unwrap();
    s.execute("object.group", &json!({})).unwrap();
    // Offset Path 0 on the group leaves every path as it is.
    s.execute("effect.apply", &json!({"effect": "path.offsetPath", "params": {"offset": 0}})).unwrap();
    let doc = s.doc().unwrap().doc.clone();
    let baked = vectorcraft_render::effects::bake_document(&doc).unwrap();
    let bb = baked.node(id).unwrap().geometric_bounds().unwrap();
    assert!((bb.x0 - b.x0).abs() < 1e-6 && (bb.y1 - b.y1).abs() < 1e-6, "{bb:?} {b:?}");
}

#[test]
fn moving_type_in_perspective_moves_its_projection() {
    let (mut s, rect) = persp_session();
    let id = persp_text(&mut s);
    let NodeKind::Text(t) = &node(&s, id).kind else { panic!("type") };
    let flat = t.xf;
    s.execute("perspective.transform", &json!({"ids": [id.0], "matrix": [1, 0, 0, 1, 40, 0]})).unwrap();
    let n = node(&s, id);
    let NodeKind::Text(t) = &n.kind else { panic!() };
    assert_eq!(t.xf, flat, "the type itself is untouched");
    // Its drawn corner moved 40 pt along the plane.
    let g = grid(&s);
    let hi = g.homography(Plane::Right).unwrap().inverse().unwrap();
    let b0 = {
        s.execute("edit.undo", &json!({})).unwrap();
        drawn_bounds(&s, id)
    };
    s.execute("edit.redo", &json!({})).unwrap();
    let b1 = drawn_bounds(&s, id);
    let d = hi.apply(Point::new(b1.x0, b1.y1)).unwrap() - hi.apply(Point::new(b0.x0, b0.y1)).unwrap();
    assert!((d.x - 40.0).abs() < 1.0 && d.y.abs() < 1.0, "{d:?}");
    let _ = rect;
}

#[test]
fn edit_text_shows_it_flat_in_isolation_then_projects_it_again() {
    let (mut s, _) = persp_session();
    let id = persp_text(&mut s);
    let drawn = node(&s, id).geometric_bounds().unwrap();
    s.execute("select.set", &json!({"ids": [id.0]})).unwrap();
    s.execute("perspective.editText", &json!({})).unwrap();
    let st = s.doc().unwrap();
    assert_eq!(st.isolation, Some(id));
    let n = node(&s, id);
    assert!(n.projection().is_none(), "flat while editing");
    // Shown where it is drawn in perspective.
    assert!(n.geometric_bounds().unwrap().center().distance(drawn.center()) < 1e-6);
    s.execute("text.setText", &json!({"id": id.0, "text": "HELLO WORLD"})).unwrap();
    s.execute("object.exitIsolation", &json!({})).unwrap();
    let n = node(&s, id);
    assert!(n.projection().is_some(), "projected again");
    // Nothing left flat anywhere in the history.
    for e in &s.doc().unwrap().history.undo {
        if let Some(p) = e.doc.node(id).and_then(|n| n.extras().perspective.as_deref()) {
            assert!(!p.editing);
        }
    }
    // Not type in perspective: refused.
    let (mut s2, rect) = persp_session();
    assert!(s2.execute("perspective.editText", &json!({"id": rect.0})).is_err());
}

#[test]
fn symbols_in_perspective_stay_instances() {
    let (mut s, rect) = persp_session();
    s.execute("perspective.release", &json!({"ids": [rect.0]})).unwrap();
    s.execute("select.set", &json!({"ids": [rect.0]})).unwrap();
    let sym = s.execute("symbol.new", &json!({"name": "Box"})).unwrap();
    let _ = sym;
    let r = s.execute("symbol.place", &json!({"name": "Box", "x": 470, "y": 380})).unwrap();
    let id = NodeId(r["id"].as_u64().unwrap());
    s.execute("perspective.attach", &json!({"ids": [id.0], "plane": "right"})).unwrap();
    let n = node(&s, id);
    assert!(matches!(n.kind, NodeKind::SymbolInstance { .. }));
    assert!(n.projection().is_some());
    let baked = vectorcraft_render::effects::bake_document(&s.doc().unwrap().doc).unwrap();
    assert!(!matches!(baked.node(id).unwrap().kind, NodeKind::SymbolInstance { .. }), "exports draw the projected art");
}

#[test]
fn copies_pastes_and_duplicates_stay_attached() {
    let (mut s, id) = persp_session();
    s.execute("edit.copy", &json!({})).unwrap();
    s.execute("edit.paste", &json!({})).unwrap();
    let pasted = *s.doc().unwrap().selection.objects.first().unwrap();
    assert_ne!(pasted, id);
    assert_eq!(attached(&s, pasted), Some((Plane::Right, 0.0)));
    s.execute("select.set", &json!({"ids": [id.0]})).unwrap();
    s.execute("object.transform", &json!({"matrix": [1, 0, 0, 1, 30, 0], "copy": true})).unwrap();
    let copy = *s.doc().unwrap().selection.objects.first().unwrap();
    assert_eq!(attached(&s, copy), Some((Plane::Right, 0.0)));
    // Deleting leaves nothing stale behind.
    s.execute("edit.clear", &json!({})).unwrap();
    assert!(grid(&s).attached_plane(copy).is_none());
}

#[test]
fn a_selection_tool_move_keeps_the_attachment_and_type_follows() {
    let (mut s, rect) = persp_session();
    let id = persp_text(&mut s);
    let before = drawn_bounds(&s, id);
    s.execute("select.set", &json!({"ids": [id.0, rect.0]})).unwrap();
    let c = node(&s, rect).geometric_bounds().unwrap().center();
    s.select_tool("selection", ViewInfo::default()).unwrap();
    drag(&mut s, &[c, Point::new(c.x + 25.0, c.y + 10.0), Point::new(c.x + 25.0, c.y + 10.0)], Mods::default());
    assert_eq!(attached(&s, rect), Some((Plane::Right, 0.0)));
    assert_eq!(attached(&s, id), Some((Plane::Right, 0.0)));
    let after = drawn_bounds(&s, id);
    assert!((after.x0 - before.x0 - 25.0).abs() < 1e-6 && (after.y0 - before.y0 - 10.0).abs() < 1e-6, "{before:?} {after:?}");
}

#[test]
fn release_keeps_the_look_of_type_in_perspective() {
    let (mut s, _) = persp_session();
    let id = persp_text(&mut s);
    let b = drawn_bounds(&s, id);
    s.execute("perspective.release", &json!({"ids": [id.0]})).unwrap();
    assert_eq!(attached(&s, id), None);
    assert_eq!(drawn_bounds(&s, id), b);
    assert!(persp::attachment(&node(&s, id)).is_none());
}

#[test]
fn flare_spiral_and_polar_grid_draw_in_perspective() {
    let (mut s, _) = persp_session();
    s.execute("perspective.grid.show", &json!({"visible": true})).unwrap();
    s.execute("perspective.plane.set", &json!({"plane": "left"})).unwrap();
    for (tool, pts) in [("spiral", [(300.0, 380.0), (320.0, 390.0), (330.0, 400.0)]), ("polarGrid", [(250.0, 350.0), (280.0, 380.0), (300.0, 410.0)])]
    {
        let n = undo_len(&s);
        s.select_tool(tool, ViewInfo::default()).unwrap();
        drag(&mut s, &pts.map(|(x, y)| Point::new(x, y)), Mods::default());
        assert_eq!(undo_len(&s), n + 1, "{tool}");
        assert_eq!(s.journal.last().unwrap().0, "perspective.draw", "{tool}");
        let id = *s.doc().unwrap().selection.objects.first().unwrap();
        assert_eq!(attached(&s, id).map(|a| a.0), Some(Plane::Left), "{tool}");
    }
    let r = s.execute("perspective.draw", &json!({"command": "shape.flare", "params": {"cx": 300, "cy": 380, "diameter": 40}})).unwrap();
    assert_eq!(attached(&s, NodeId(r["id"].as_u64().unwrap())).map(|a| a.0), Some(Plane::Left));
}

#[test]
fn click_to_size_dialogs_draw_on_the_active_plane_in_plane_units() {
    let (mut s, _) = persp_session();
    s.execute("perspective.grid.show", &json!({"visible": true})).unwrap();
    s.execute("perspective.plane.set", &json!({"plane": "right"})).unwrap();
    let at = Point::new(470.0, 300.0);
    let (cmd, p) = crate::perspective_click(&s, "shape.rectangle", &json!({"x": at.x, "y": at.y, "width": 50, "height": 30}), at).unwrap();
    assert_eq!(cmd, "perspective.draw");
    let r = s.execute(&cmd, &p).unwrap();
    let id = NodeId(r["id"].as_u64().unwrap());
    let g = grid(&s);
    let hi = g.homography(Plane::Right).unwrap().inverse().unwrap();
    let q: Vec<Point> = node(&s, id).path_data().unwrap().anchors().map(|(_, _, a)| hi.apply(a.p).unwrap()).collect();
    let b = q.iter().skip(1).fold(Rect::from_points(q[0], q[0]), |r, p| r.union_pt(*p));
    assert!((b.width() - 50.0).abs() < 1e-6 && (b.height() - 30.0).abs() < 1e-6, "{b:?}");
    assert!(q.iter().any(|p| (g.homography(Plane::Right).unwrap().apply(*p).unwrap() - at).hypot() < 1e-6), "the click stays a corner");
    // Without a shown grid (or with no active plane) nothing changes.
    s.execute("perspective.plane.set", &json!({"plane": "none"})).unwrap();
    assert!(crate::perspective_click(&s, "shape.rectangle", &json!({}), at).is_none());
}

#[test]
fn the_canvas_draws_type_in_perspective_foreshortened() {
    let (mut s, rect) = persp_session();
    s.execute("edit.clear", &json!({"ids": [rect.0]})).ok();
    let id = persp_text(&mut s);
    let b = drawn_bounds(&s, id);
    let img = vectorcraft_render::Renderer::new().render_region(&s.doc().unwrap().doc, b, 2.0, true);
    // The inked height of a strip of columns near each end of the line.
    let height = |x0: f64, x1: f64| {
        let (mut lo, mut hi) = (u32::MAX, 0);
        for x in (x0 * img.width as f64) as u32..(x1 * img.width as f64) as u32 {
            for y in 0..img.height {
                if img.pixel(x, y)[0] < 128 {
                    lo = lo.min(y);
                    hi = hi.max(y);
                }
            }
        }
        hi.saturating_sub(lo)
    };
    let (near, far) = (height(0.0, 0.25), height(0.75, 1.0));
    assert!(near > 10 && far * 100 < near * 97, "near {near} far {far}");
}

#[test]
fn double_clicking_type_in_perspective_edits_it_and_projections_round_trip() {
    let (mut s, _) = persp_session();
    let id = persp_text(&mut s);
    // Saved and opened again: still type, still projected.
    let doc = vectorcraft_format::load(&vectorcraft_format::save(&s.doc().unwrap().doc, false)).unwrap();
    assert_eq!(doc.node(id).unwrap().projection(), node(&s, id).projection());
    let v = ViewInfo::default();
    s.select_tool("perspectiveSelection", v).unwrap();
    let c = node(&s, id).geometric_bounds().unwrap().center();
    let r = s.pointer(&vectorcraft_tools::PointerEvent::new(vectorcraft_tools::PointerKind::DoubleClick, c.x, c.y), v).unwrap();
    assert_eq!(r, vec![crate::UiRequest::SwitchTool("type".into())]);
    assert_eq!(s.doc().unwrap().isolation, Some(id));
    // Saved while editing: the file has it projected.
    let doc = vectorcraft_format::load(&vectorcraft_format::save(&s.doc().unwrap().doc, false)).unwrap();
    assert!(doc.node(id).unwrap().projection().is_some());
    s.execute("object.exitIsolation", &json!({})).unwrap();
    assert!(node(&s, id).projection().is_some());
}

/// Diagnostic: cancellation must not restore a flat perspective-editing view after isolation ends.
#[test]
fn cancelled_group_redo_keeps_exited_perspective_text_projected() {
    fn payloads(entries: &[HistoryEntry]) -> Vec<(String, Arc<Document>, Arc<Selection>)> {
        entries.iter().map(|e| (e.label.clone(), e.doc.clone(), e.selection.clone())).collect()
    }
    fn editing(doc: &Document, id: NodeId) -> bool {
        let n = doc.node(id).expect("SCRUB03_NORMALIZATION_SETUP: text node must exist");
        assert!(matches!(&n.kind, NodeKind::Text(_)), "SCRUB03_NORMALIZATION_SETUP: target must remain Text");
        n.extras().perspective.as_deref().expect("SCRUB03_NORMALIZATION_SETUP: perspective record must exist").editing
    }
    fn projected_copy(doc: &Document, id: NodeId) -> Document {
        // Expected values only: never repair the live document or any stored history entry.
        let mut copy = doc.clone();
        let n = copy.node_mut(id).expect("SCRUB03_NORMALIZATION_SETUP: comparison target exists");
        let p = n.extras_if_any().and_then(|e| e.perspective.as_deref_mut()).expect("SCRUB03_NORMALIZATION_SETUP: comparison record exists");
        p.editing = false;
        copy
    }
    fn visible_history_projected(st: &DocState, id: NodeId) {
        for e in st.history.undo.iter().chain(&st.history.redo) {
            if let Some(p) = e.doc.node(id).and_then(|n| n.extras().perspective.as_deref()) {
                assert!(!p.editing, "SCRUB03_NORMALIZATION_SETUP: visible history and group baseline must not already be flat");
            }
        }
    }

    let (mut s, rectangle) = persp_session();
    let id = persp_text(&mut s);
    s.execute("select.set", &json!({"ids": [id.0]})).expect("SCRUB03_NORMALIZATION_SETUP: select projected text");
    assert_eq!(s.tool_id(), "selection", "SCRUB03_NORMALIZATION_SETUP: no Type-tool cleanup or typing preview");
    let st = s.doc().unwrap();
    assert!(st.doc.mask_edit.is_none() && st.isolation.is_none(), "SCRUB03_NORMALIZATION_SETUP: outside editing modes");
    assert!(st.interaction.is_none() && st.undo_group.is_none(), "SCRUB03_NORMALIZATION_SETUP: no prior transaction");
    assert!(st.history.redo.is_empty(), "SCRUB03_NORMALIZATION_SETUP: clean initial Redo");
    assert!(!editing(&st.doc, id), "SCRUB03_NORMALIZATION_SETUP: initial text is projected");
    let n = st.doc.node(id).unwrap();
    let NodeKind::Text(text) = &n.kind else { panic!("SCRUB03_NORMALIZATION_SETUP: target is Text") };
    let flat = text.bounds().expect("SCRUB03_NORMALIZATION_SETUP: actual laid-out text bounds are required");
    let projection = n.projection().expect("SCRUB03_NORMALIZATION_SETUP: actual valid projection is required");
    let drawn = projection.map_rect_bbox(flat).expect("SCRUB03_NORMALIZATION_SETUP: projection must map the actual bounds");
    for b in [flat, drawn] {
        assert!(
            [b.x0, b.y0, b.x1, b.y1].into_iter().all(f64::is_finite) && b.width() > 0.0 && b.height() > 0.0,
            "SCRUB03_NORMALIZATION_SETUP: finite, nonempty real text bounds"
        );
    }
    visible_history_projected(st, id);
    let a = st.doc.clone();
    let s0 = st.selection.clone();
    assert_eq!(s0.objects, vec![id], "SCRUB03_NORMALIZATION_SETUP: one selected text object");
    let u = payloads(&st.history.undo);
    let saved = st.saved_doc.clone();
    let before_dirty = st.is_dirty();
    let (limit, budget) = (st.history.limit, st.history.byte_budget);
    let preserved = |st: &DocState| {
        assert!(Arc::ptr_eq(&st.saved_doc, &saved), "SCRUB03_NORMALIZATION_SETUP: saved snapshot unchanged");
        assert_eq!((st.history.limit, st.history.byte_budget), (limit, budget), "SCRUB03_NORMALIZATION_SETUP: limits unchanged");
        assert!(st.doc.mask_edit.is_none() && st.interaction.is_none(), "SCRUB03_NORMALIZATION_SETUP: no mask or preview");
    };
    let mut with_edit = u.clone();
    with_edit.push(("Edit Text".into(), a.clone(), Arc::new(s0.clone())));

    let r = s.execute("perspective.editText", &json!({})).expect("SCRUB03_NORMALIZATION_SETUP: first Edit Text");
    assert_eq!(r["id"], json!(id.0), "SCRUB03_NORMALIZATION_SETUP: correct first edit target");
    let st = s.doc().unwrap();
    preserved(st);
    assert!(st.undo_group.is_none() && st.history.redo.is_empty(), "SCRUB03_NORMALIZATION_SETUP: first edit is standalone");
    assert!(editing(&st.doc, id), "SCRUB03_NORMALIZATION_SETUP: first edit really entered flat editing");
    assert!(st.doc.node(id).unwrap().projection().is_none(), "SCRUB03_NORMALIZATION_SETUP: first edit is drawn flat");
    assert_eq!(st.isolation, Some(id), "SCRUB03_NORMALIZATION_SETUP: first edit entered isolation");
    assert_eq!(st.selection, s0);
    assert_eq!(payloads(&st.history.undo), with_edit, "SCRUB03_NORMALIZATION_SETUP: one complete Edit Text entry");
    assert_eq!(st.doc.node(rectangle), a.node(rectangle), "SCRUB03_NORMALIZATION_SETUP: control rectangle unchanged");
    let b = st.doc.clone();
    let bs = st.selection.clone();
    assert_ne!(b, a, "SCRUB03_NORMALIZATION_SETUP: command success alone cannot establish a real edit");
    let expected_redo = vec![("Edit Text".into(), b.clone(), Arc::new(bs.clone()))];

    let r = s.execute("edit.undo", &json!({})).expect("SCRUB03_NORMALIZATION_SETUP: undo first Edit Text");
    assert_eq!(r["undone"], "Edit Text");
    let st = s.doc().unwrap();
    preserved(st);
    assert!(st.undo_group.is_none());
    assert_eq!(st.doc, a, "SCRUB03_NORMALIZATION_SETUP: Undo restored the complete projected document");
    assert_eq!(st.selection, s0);
    assert_eq!(payloads(&st.history.undo), u);
    assert_eq!(payloads(&st.history.redo), expected_redo);
    assert_eq!(st.isolation, Some(id), "SCRUB03_NORMALIZATION_SETUP: inherited perspective isolation survives Undo");
    assert!(!editing(&st.doc, id) && st.doc.node(id).unwrap().projection().is_some());
    assert_eq!(st.is_dirty(), before_dirty);

    s.execute("edit.undoGroup.begin", &json!({})).expect("SCRUB03_NORMALIZATION_SETUP: begin public undo group");
    let st = s.doc().unwrap();
    let group = st.undo_group.as_ref().expect("SCRUB03_NORMALIZATION_SETUP: group opened");
    assert!(group.first.is_none() && group.saved_redo.is_empty(), "SCRUB03_NORMALIZATION_SETUP: no group edit yet");
    assert_eq!(st.doc, a);
    assert_eq!(st.selection, s0);
    assert_eq!(payloads(&st.history.undo), u);
    assert_eq!(payloads(&st.history.redo), expected_redo);

    let r = s.execute("perspective.editText", &json!({})).expect("SCRUB03_NORMALIZATION_SETUP: real grouped Edit Text");
    assert_eq!(r["id"], json!(id.0));
    let st = s.doc().unwrap();
    preserved(st);
    assert_eq!(st.doc, b, "SCRUB03_NORMALIZATION_SETUP: second real edit reaches the same complete edited document");
    assert!(editing(&st.doc, id) && st.doc.node(id).unwrap().projection().is_none());
    assert_eq!(st.isolation, Some(id));
    assert_eq!(st.selection, s0);
    assert_eq!(payloads(&st.history.undo), with_edit);
    assert!(st.history.redo.is_empty(), "SCRUB03_NORMALIZATION_SETUP: prior Redo moved out of visible history");
    let group = st.undo_group.as_ref().unwrap();
    assert_eq!(payloads(&group.saved_redo), expected_redo, "SCRUB03_NORMALIZATION_SETUP: exactly one retained original Redo");
    let first = group.first.as_ref().expect("SCRUB03_NORMALIZATION_SETUP: group recorded its baseline");
    assert!(
        Arc::ptr_eq(first, &a) && Arc::ptr_eq(first, &st.history.undo.last().unwrap().doc),
        "SCRUB03_NORMALIZATION_SETUP: newest-step guard matches"
    );
    visible_history_projected(st, id);

    s.execute("object.exitIsolation", &json!({})).expect("SCRUB03_NORMALIZATION_SETUP: public isolation exit");
    let st = s.doc().unwrap();
    preserved(st);
    assert!(st.isolation.is_none(), "SCRUB03_NORMALIZATION_SETUP: isolation really exited");
    assert!(!editing(&st.doc, id) && st.doc.node(id).unwrap().projection().is_some());
    assert_eq!(st.selection, s0);
    assert_eq!(payloads(&st.history.undo), with_edit);
    assert!(st.history.redo.is_empty());
    visible_history_projected(st, id);
    let group = st.undo_group.as_ref().unwrap();
    let first = group.first.as_ref().unwrap();
    assert!(
        Arc::ptr_eq(first, &a) && Arc::ptr_eq(first, &st.history.undo.last().unwrap().doc),
        "SCRUB03_NORMALIZATION_SETUP: cleanup retained the cancel guard"
    );
    assert_eq!(group.saved_redo.len(), 1);
    let retained = &group.saved_redo[0];
    assert_eq!(retained.label, "Edit Text");
    assert_eq!(*retained.selection, bs);
    let hidden_editing = editing(&retained.doc, id);
    let d = st.doc.clone();
    assert_eq!(*d, projected_copy(&b, id), "SCRUB03_NORMALIZATION_SETUP: cleanup changed only the comparison's editing flag");
    assert_eq!(projected_copy(&retained.doc, id), *d, "SCRUB03_NORMALIZATION_SETUP: hidden Redo has no unrelated document changes");

    s.execute("edit.undoGroup.end", &json!({"cancel": true})).expect("SCRUB03_NORMALIZATION_SETUP: cancel the public group");
    let st = s.doc().unwrap();
    preserved(st);
    assert!(st.undo_group.is_none() && st.isolation.is_none(), "SCRUB03_NORMALIZATION_SETUP: group closed outside isolation");
    assert!(Arc::ptr_eq(&st.doc, &a), "SCRUB03_NORMALIZATION_SETUP: cancellation really restored the baseline Arc");
    assert_eq!(st.doc, a);
    assert_eq!(st.selection, s0);
    assert_eq!(st.is_dirty(), before_dirty);
    assert_eq!(payloads(&st.history.undo), u);
    assert_eq!(st.history.redo.len(), 1, "SCRUB03_NORMALIZATION_SETUP: retained Redo is reachable after cancellation");
    let redo = &st.history.redo[0];
    assert_eq!(redo.label, "Edit Text");
    assert_eq!(*redo.selection, bs);
    assert_eq!(projected_copy(&redo.doc, id), *d, "SCRUB03_NORMALIZATION_SETUP: restored Redo differs only in its observed flag");

    let r = s.execute("edit.redo", &json!({})).expect("SCRUB03_NORMALIZATION_SETUP: restored Redo must execute");
    assert_eq!(r["redone"], "Edit Text");
    let st = s.doc().unwrap();
    preserved(st);
    assert!(st.undo_group.is_none() && st.isolation.is_none(), "SCRUB03_NORMALIZATION_SETUP: Redo outside isolation and groups");
    assert_eq!(st.selection, s0);
    assert_eq!(payloads(&st.history.undo), with_edit);
    assert!(st.history.redo.is_empty());
    assert_eq!(projected_copy(&st.doc, id), *d, "SCRUB03_NORMALIZATION_SETUP: complete Redo result except the flag");
    let editing = editing(&st.doc, id);
    eprintln!("SCRUB03_NORMALIZATION_REDO_REACHED: hidden_editing={hidden_editing}, redo_editing={editing}");
    assert!(!editing, "SCRUB03_NORMALIZATION_REDO_MUST_STAY_PROJECTED");
    assert!(st.doc.node(id).unwrap().projection().is_some(), "SCRUB03_NORMALIZATION: text must be projected outside isolation");
    assert_eq!(st.doc, d, "SCRUB03_NORMALIZATION: complete desired document");
    assert_eq!(payloads(&st.history.undo), with_edit);
    assert!(st.history.redo.is_empty());
    eprintln!("SCRUB03_NORMALIZATION_DESIRED_STATE_VERIFIED");
}
