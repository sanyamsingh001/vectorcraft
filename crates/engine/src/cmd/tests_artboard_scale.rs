//! Small synthetic coverage of optional artwork scaling with an artboard (#602).
use crate::{Session, ViewInfo};
use serde_json::json;
use vectorcraft_doc::NodeId;
use vectorcraft_geom::{Point, Rect};

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
