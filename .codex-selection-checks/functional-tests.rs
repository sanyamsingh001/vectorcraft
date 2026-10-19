#[cfg(test)]
mod issue593_functional_tests {
    use super::*;
    use crate::select::SelectionTool;
    use crate::testutil::{cx, paint};
    use vectorcraft_doc::{Appearance, Document, Selection};
    use vectorcraft_geom::SubPath;

    fn add_path(d: &mut Document, points: &[Point]) -> NodeId {
        let id = d.alloc_id();
        let layer = d.layers[0].id;
        let at = d.layers[0].children().unwrap().len();
        d.insert(Some(layer), at, Node::path(id, PathData::single(SubPath::polyline(points, false)), Appearance::default())).unwrap();
        id
    }

    fn highlight(t: &DirectSelectionTool, cx: &ToolContext) -> Option<Point> {
        t.overlays(cx).into_iter().find_map(|o| match o {
            Overlay::Anchor { p, size, .. } if size == 8.0 => Some(p),
            _ => None,
        })
    }

    #[test]
    fn selection_tolerance_is_screen_pixels_for_press_and_hover() {
        for zoom in [0.5, 1.0, 2.0] {
            for tolerance in [1.0, 3.0, 8.0] {
                let mut d = Document::new(500.0, 500.0);
                let first = Point::new(100.0, 300.0);
                let id = add_path(&mut d, &[first, Point::new(200.0, 300.0)]);
                let (s, paint) = (Selection::default(), paint());
                let cx = ToolContext {
                    zoom, selection_tolerance: tolerance, smart_guides: false,
                    snap_to_point: false, corner_widgets: false, show_bbox: false,
                    ..cx(&d, &s, &paint)
                };
                for pixels in [0.0, tolerance * 0.75, tolerance] {
                    let p = Point::new(first.x - pixels / zoom, first.y);
                    let mut t = DirectSelectionTool::new(false);
                    t.pointer(&cx, &PointerEvent::new(PointerKind::Move, p.x, p.y));
                    assert_eq!(highlight(&t, &cx), Some(first), "zoom={zoom}, tolerance={tolerance}, pixels={pixels}");
                    assert_eq!(t.cursor(&cx, p, Mods::default()), Cursor::ArrowHollow);
                    assert_eq!(t.pointer(&cx, &PointerEvent::new(PointerKind::Down, p.x, p.y)),
                        vec![Action::Exec("select.anchors".into(), json!({"id": id.0, "anchors": [[0, 0]], "mode": "set"}))]);
                    let mid = Point::new(150.0, 300.0 + pixels / zoom);
                    assert_eq!(SelectionTool::default().pointer(&cx, &PointerEvent::new(PointerKind::Down, mid.x, mid.y)),
                        vec![Action::Exec("select.set".into(), json!({"ids": [id.0]}))]);
                }
                let outside = Point::new(first.x - (tolerance + 0.5) / zoom, first.y);
                let mut t = DirectSelectionTool::new(false);
                t.pointer(&cx, &PointerEvent::new(PointerKind::Move, outside.x, outside.y));
                assert_eq!(highlight(&t, &cx), None);
                assert!(t.pointer(&cx, &PointerEvent::new(PointerKind::Down, outside.x, outside.y)).is_empty());
                let mid = Point::new(150.0, 300.0 + (tolerance + 0.5) / zoom);
                assert!(SelectionTool::default().pointer(&cx, &PointerEvent::new(PointerKind::Down, mid.x, mid.y)).is_empty());
            }
        }
    }

    #[test]
    fn nearby_anchors_pick_and_highlight_the_nearest_on_the_path() {
        let mut failures = vec![];
        for zoom in [0.5, 1.0, 2.0] {
            for tolerance in [1.0, 3.0, 8.0] {
                let mut d = Document::new(500.0, 500.0);
                let first = Point::new(100.0, 300.0);
                let second = Point::new(100.0 + tolerance * 0.75 / zoom, 300.0);
                let id = add_path(&mut d, &[first, second, Point::new(200.0, 300.0)]);
                let (s, paint) = (Selection::default(), paint());
                let cx = ToolContext { zoom, selection_tolerance: tolerance, smart_guides: false,
                    snap_to_point: false, corner_widgets: false, ..cx(&d, &s, &paint) };
                for (p, ai, anchor) in [(first, 0, first), (second, 1, second),
                    (Point::new(second.x + tolerance * 0.125 / zoom, second.y), 1, second)] {
                    let mut t = DirectSelectionTool::new(false);
                    t.pointer(&cx, &PointerEvent::new(PointerKind::Move, p.x, p.y));
                    let shown = highlight(&t, &cx);
                    let pressed = t.pointer(&cx, &PointerEvent::new(PointerKind::Down, p.x, p.y));
                    let expected = vec![Action::Exec("select.anchors".into(), json!({"id": id.0, "anchors": [[0, ai]], "mode": "set"}))];
                    if shown != Some(anchor) || pressed != expected {
                        failures.push(format!("zoom={zoom}, tolerance={tolerance}, pointer={p:?}: hover={shown:?}, press={pressed:?}, expected anchor={ai}"));
                    }
                }
            }
        }
        assert!(failures.is_empty(), "{} failures:\n{}", failures.len(), failures.join("\n"));
    }

    #[test]
    fn hover_matches_press_when_a_selected_path_is_behind_another_path() {
        let mut failures = vec![];
        for zoom in [0.5, 1.0, 2.0] {
            for tolerance in [1.0, 3.0, 8.0] {
                let mut d = Document::new(500.0, 500.0);
                let rear = add_path(&mut d, &[Point::new(100.0, 300.0), Point::new(200.0, 300.0)]);
                let front_anchor = Point::new(100.0 + tolerance * 0.5 / zoom, 300.0);
                let front = add_path(&mut d, &[front_anchor, Point::new(200.0, 310.0)]);
                let mut s = Selection::default();
                s.set([rear]);
                let paint = paint();
                let cx = ToolContext { zoom, selection_tolerance: tolerance, smart_guides: false,
                    snap_to_point: false, corner_widgets: false, ..cx(&d, &s, &paint) };
                let mut t = DirectSelectionTool::new(false);
                t.pointer(&cx, &PointerEvent::new(PointerKind::Move, front_anchor.x, front_anchor.y));
                let shown = highlight(&t, &cx);
                let pressed = t.pointer(&cx, &PointerEvent::new(PointerKind::Down, front_anchor.x, front_anchor.y));
                let expected = vec![Action::Exec("select.anchors".into(), json!({"id": front.0, "anchors": [[0, 0]], "mode": "set"}))];
                if shown != Some(front_anchor) || pressed != expected {
                    failures.push(format!("zoom={zoom}, tolerance={tolerance}: hover={shown:?}, press={pressed:?}, expected front={front:?}"));
                }
            }
        }
        assert!(failures.is_empty(), "{} failures:\n{}", failures.len(), failures.join("\n"));
    }
}
