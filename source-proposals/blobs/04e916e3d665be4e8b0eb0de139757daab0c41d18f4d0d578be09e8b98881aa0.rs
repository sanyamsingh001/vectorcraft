#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Original finite artwork: Average and Join must preserve a retained B contour's rendered fill.
//! Public commands/direct-anchor selection only; no UI or arbitrary-gradient-type claim.

use serde_json::json;
use std::sync::Arc;
use vectorcraft_color::Paint;
use vectorcraft_doc::{Document, NodeId, NodeKind, Selection};
use vectorcraft_engine::{HistoryEntry, Session};
use vectorcraft_geom::{Affine, PathData, Point, SubPath};
use vectorcraft_render::{RenderOptions, Renderer};

type Entries = Vec<(String, Arc<Document>, Arc<Selection>)>;

fn entries(items: &[HistoryEntry]) -> Entries {
    items.iter().map(|entry| (entry.label.clone(), entry.doc.clone(), entry.selection.clone())).collect()
}

struct Snapshot {
    doc: Arc<Document>,
    selection: Selection,
    undo: Entries,
    redo: Entries,
    dirty: bool,
}

impl Snapshot {
    fn of(s: &Session) -> Self {
        let st = s.doc().unwrap();
        Self {
            doc: st.doc.clone(),
            selection: st.selection.clone(),
            undo: entries(&st.history.undo),
            redo: entries(&st.history.redo),
            dirty: st.is_dirty(),
        }
    }

    fn assert_unchanged(&self, s: &Session) {
        let st = s.doc().unwrap();
        assert_eq!(st.doc, self.doc);
        assert_eq!(st.selection, self.selection);
        assert_eq!(entries(&st.history.undo), self.undo);
        assert_eq!(entries(&st.history.redo), self.redo);
        assert_eq!(st.is_dirty(), self.dirty);
    }
}

fn make(s: &mut Session, d: &str) -> NodeId {
    NodeId(s.execute("path.create", &json!({"d": d})).unwrap()["id"].as_u64().unwrap())
}

fn path(s: &Session, id: NodeId) -> PathData {
    s.doc().unwrap().doc.node(id).unwrap().path_data().unwrap().clone()
}

// This only edits an independent expected-document clone, never the live fixture.
fn expected_path(doc: &mut Document, id: NodeId, value: PathData) {
    let NodeKind::Path { path, .. } = &mut doc.node_mut(id).unwrap().kind else { panic!("expected fixture is a path") };
    *path = value;
}

#[test]
fn average_join_preserves_retained_contour_gradient_pixels() {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 600, "height": 400, "colorMode": "rgb"})).unwrap();
    s.select_tool("directSelection", Default::default()).unwrap();
    let a = make(&mut s, "M0 0L10 0");
    s.execute("paint.setStroke", &json!({"color": "#2468d8"})).unwrap();
    let b = make(&mut s, "M12 0L20 0 M100 100L120 100L110 120Z");
    // Ordinary opaque linear fill: red plateau, transition, then blue plateau.
    // The original interior sample is literal blue, away from interpolation and edge rounding.
    s.execute(
        "paint.setFill",
        &json!({"gradient": {"kind": "linear", "angle": 0, "stops": [
            {"offset": 0.0, "color": "#ff0000"},
            {"offset": 0.6, "color": "#ff0000"},
            {"offset": 0.8, "color": "#0000ff"},
            {"offset": 1.0, "color": "#0000ff"}
        ]}}),
    )
    .unwrap();
    s.execute("paint.setStroke", &json!({"none": true})).unwrap();
    let unrelated = make(&mut s, "M300 200L330 210L315 240Z");
    s.execute("paint.setFill", &json!({"color": "#23854a"})).unwrap();
    s.execute("object.move", &json!({"dx": 3, "dy": 7})).unwrap();
    s.execute("edit.undo", &json!({})).unwrap();
    s.execute("select.anchorsMany", &json!({"items": [{"id": a.0, "anchors": [[0, 1]]}, {"id": b.0, "anchors": [[0, 0]]}]})).unwrap();
    s.doc_mut().unwrap().mark_saved();
    let before = Snapshot::of(&s);
    assert_eq!(before.redo.len(), 1, "AVERAGE_JOIN_GRADIENT_SETUP_HISTORY");
    assert!(!before.dirty, "AVERAGE_JOIN_GRADIENT_SETUP_SAVED");
    assert_eq!(before.selection.objects, [a, b], "AVERAGE_JOIN_GRADIENT_SETUP_SELECTION");
    assert_eq!(before.selection.anchors[&a], [(0, 1)].into(), "AVERAGE_JOIN_GRADIENT_SETUP_A_PICK");
    assert_eq!(before.selection.anchors[&b], [(0, 0)].into(), "AVERAGE_JOIN_GRADIENT_SETUP_B_PICK");
    assert!(before.selection.segments.is_empty() && before.selection.guides.is_empty() && before.selection.slices.is_empty());
    let command = vectorcraft_engine::find_command("path.averageJoin").unwrap();
    assert!((command.enabled)(&s).is_ok(), "AVERAGE_JOIN_GRADIENT_SETUP_ELIGIBLE");
    let Paint::Gradient(initial_gradient) = before.doc.node(b).unwrap().appearance.fill_paint() else {
        panic!("AVERAGE_JOIN_GRADIENT_SETUP_PAINT: B must have a gradient")
    };
    assert!(initial_gradient.geom.is_none(), "AVERAGE_JOIN_GRADIENT_SETUP_PAINT: fresh bounds-fitted fill");
    let retained = PathData::new(vec![SubPath::polyline(&[(100.0, 100.0), (120.0, 100.0), (110.0, 120.0)].map(Point::from), true)]);
    let pb = path(&s, b);
    assert_eq!(pb.subpaths.len(), 2, "AVERAGE_JOIN_GRADIENT_SETUP_CONTOURS");
    assert_eq!(pb.subpaths[1], retained.subpaths[0], "AVERAGE_JOIN_GRADIENT_SETUP_TRIANGLE");

    // Public CPU render at identity view. This pixel lies wholly inside the untouched triangle.
    // Fresh renderer instances avoid a stale-cache pass; no gradient helper computes the oracle.
    let pixel = |s: &Session| Renderer::new().render(&s.doc().unwrap().doc, 160, 160, Affine::IDENTITY, &RenderOptions::default()).pixel(110, 110);
    assert_eq!(pixel(&s), [0, 0, 255, 255], "AVERAGE_JOIN_GRADIENT_SETUP_PIXEL: literal original blue");
    eprintln!("AVERAGE_JOIN_GRADIENT_SETUP_READY");

    s.execute("path.averageJoin", &json!({"kind": "corner"})).expect("AVERAGE_JOIN_GRADIENT_COMMAND: valid picked endpoints");
    eprintln!("AVERAGE_JOIN_GRADIENT_COMMAND_COMPLETED");
    assert!(s.doc().unwrap().doc.node(b).is_some(), "AVERAGE_JOIN_GRADIENT_B_MISSING: retained contour owner was deleted");
    eprintln!("AVERAGE_JOIN_GRADIENT_B_EXISTS");
    assert_eq!(path(&s, b), retained, "AVERAGE_JOIN_GRADIENT_B_GEOMETRY: retained triangle changed");
    let joined = PathData::new(vec![SubPath::polyline(&[(0.0, 0.0), (11.0, 0.0), (20.0, 0.0)].map(Point::from), false)]);
    assert_eq!(path(&s, a), joined, "AVERAGE_JOIN_GRADIENT_A_GEOMETRY: independent literal averaged midpoint");
    let mut expected_selection = before.selection.clone();
    expected_selection.set([a]);
    expected_selection.anchors.insert(a, [(0, 1)].into());
    assert_eq!(s.doc().unwrap().selection, expected_selection, "AVERAGE_JOIN_GRADIENT_JOINED_SELECTION");
    eprintln!("AVERAGE_JOIN_GRADIENT_GEOMETRY_PRESERVED");

    let mut expected = before.doc.clone();
    let d = Arc::make_mut(&mut expected);
    expected_path(d, a, joined);
    expected_path(d, b, retained);
    // Placement storage may change to preserve appearance. Only this representation is exempt
    // from the structural comparison; literal rendered blue independently constrains its effect.
    let Paint::Gradient(actual_gradient) = s.doc().unwrap().doc.node(b).unwrap().appearance.fill_paint() else {
        panic!("AVERAGE_JOIN_GRADIENT_PAINT_KIND: B lost its gradient")
    };
    let Paint::Gradient(expected_gradient) = &mut d.node_mut(b).unwrap().appearance.fill_mut().unwrap().paint else {
        panic!("AVERAGE_JOIN_GRADIENT_EXPECTED_PAINT: fixture lost its gradient")
    };
    expected_gradient.geom = actual_gradient.geom;
    assert_eq!(s.doc().unwrap().doc, expected, "AVERAGE_JOIN_GRADIENT_DOCUMENT: unrelated fields changed");
    assert_eq!(s.doc().unwrap().doc.node(unrelated), before.doc.node(unrelated), "AVERAGE_JOIN_GRADIENT_UNRELATED_ART");
    eprintln!("AVERAGE_JOIN_GRADIENT_DOCUMENT_PRESERVED");

    assert_eq!(pixel(&s), [0, 0, 255, 255], "AVERAGE_JOIN_GRADIENT_RECOLOR: retained B interior must remain original blue");
    eprintln!("AVERAGE_JOIN_GRADIENT_PIXEL_PRESERVED");
    let after = Snapshot::of(&s);
    assert_eq!(after.undo.len(), before.undo.len() + 1, "Average and Join is one edit");
    assert_eq!(&after.undo[..before.undo.len()], before.undo.as_slice());
    assert_eq!(after.undo.last().unwrap(), &("Average and Join".into(), before.doc.clone(), Arc::new(before.selection.clone())));
    assert!(after.redo.is_empty(), "successful edit replaces the old Redo branch");
    assert!(after.dirty);

    assert_eq!(s.execute("edit.undo", &json!({})).unwrap()["undone"], "Average and Join");
    let undone = Snapshot::of(&s);
    assert_eq!(undone.doc, before.doc);
    assert_eq!(undone.selection, before.selection);
    assert_eq!(undone.undo, before.undo);
    assert_eq!(undone.redo, vec![("Average and Join".into(), after.doc.clone(), Arc::new(after.selection.clone()))]);
    assert_eq!(undone.dirty, before.dirty);
    assert_eq!(pixel(&s), [0, 0, 255, 255], "AVERAGE_JOIN_GRADIENT_UNDO_PIXEL");

    assert_eq!(s.execute("edit.redo", &json!({})).unwrap()["redone"], "Average and Join");
    after.assert_unchanged(&s);
    assert_eq!(pixel(&s), [0, 0, 255, 255], "AVERAGE_JOIN_GRADIENT_REDO_PIXEL");
    eprintln!("AVERAGE_JOIN_GRADIENT_HISTORY_PRESERVED");
}

// Diagnostic hypothesis: an untouched contour on survivor A keeps its original paint placement.
// Existing move/uniform-scale refit policy does not decide this topological-edit case.
#[test]
fn average_join_survivor_contour_gradient_preservation_diagnostic() {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 600, "height": 400, "colorMode": "rgb"})).unwrap();
    s.select_tool("directSelection", Default::default()).unwrap();
    let a = make(&mut s, "M0 0L10 0 M100 100L120 100L110 120Z");
    s.execute(
        "paint.setFill",
        &json!({"gradient": {"kind": "linear", "angle": 0, "stops": [
            {"offset": 0.0, "color": "#ff0000"},
            {"offset": 0.6, "color": "#ff0000"},
            {"offset": 0.8, "color": "#0000ff"},
            {"offset": 1.0, "color": "#0000ff"}
        ]}}),
    )
    .unwrap();
    s.execute("paint.setStroke", &json!({"none": true})).unwrap();
    let b = make(&mut s, "M200 0L220 0");
    s.execute("paint.setFill", &json!({"none": true})).unwrap();
    s.execute("paint.setStroke", &json!({"color": "#2468d8"})).unwrap();
    s.execute("object.move", &json!({"dx": 3, "dy": 7})).unwrap();
    s.execute("edit.undo", &json!({})).unwrap();
    s.execute("select.anchorsMany", &json!({"items": [{"id": a.0, "anchors": [[0, 1]]}, {"id": b.0, "anchors": [[0, 0]]}]})).unwrap();
    s.doc_mut().unwrap().mark_saved();
    let before = Snapshot::of(&s);
    assert_eq!(before.redo.len(), 1, "AVERAGE_JOIN_SURVIVOR_SETUP_HISTORY");
    assert!(!before.dirty, "AVERAGE_JOIN_SURVIVOR_SETUP_SAVED");
    assert_eq!(before.selection.objects, [a, b]);
    assert_eq!(before.selection.anchors[&a], [(0, 1)].into());
    assert_eq!(before.selection.anchors[&b], [(0, 0)].into());
    assert!(before.selection.segments.is_empty() && before.selection.guides.is_empty() && before.selection.slices.is_empty());
    let command = vectorcraft_engine::find_command("path.averageJoin").expect("AVERAGE_JOIN_SURVIVOR_FEATURE_PRESENT");
    assert!((command.enabled)(&s).is_ok(), "AVERAGE_JOIN_SURVIVOR_SETUP_ELIGIBLE");
    let Paint::Gradient(initial_gradient) = before.doc.node(a).unwrap().appearance.fill_paint() else {
        panic!("AVERAGE_JOIN_SURVIVOR_SETUP_PAINT: A must have a gradient")
    };
    assert!(initial_gradient.geom.is_none(), "AVERAGE_JOIN_SURVIVOR_SETUP_PAINT: fresh bounds-fitted fill");
    let retained = SubPath::polyline(&[(100.0, 100.0), (120.0, 100.0), (110.0, 120.0)].map(Point::from), true);
    let pa = path(&s, a);
    assert_eq!(pa.subpaths.len(), 2);
    assert_eq!(pa.subpaths[1], retained, "AVERAGE_JOIN_SURVIVOR_SETUP_TRIANGLE");
    // Fresh renderers and a literal interior plateau pixel; no gradient helper derives the oracle.
    let pixel = |s: &Session| Renderer::new().render(&s.doc().unwrap().doc, 260, 160, Affine::IDENTITY, &RenderOptions::default()).pixel(110, 110);
    assert_eq!(pixel(&s), [0, 0, 255, 255], "AVERAGE_JOIN_SURVIVOR_SETUP_PIXEL: original blue");
    eprintln!("AVERAGE_JOIN_SURVIVOR_SETUP_READY");

    let result = s.execute("path.averageJoin", &json!({"kind": "corner"})).expect("AVERAGE_JOIN_SURVIVOR_COMMAND");
    assert_eq!(result, json!({"id": a.0, "subpath": 0, "anchor": 1}));
    eprintln!("AVERAGE_JOIN_SURVIVOR_COMMAND_COMPLETED");
    let joined = PathData::new(vec![SubPath::polyline(&[(0.0, 0.0), (105.0, 0.0), (220.0, 0.0)].map(Point::from), false), retained.clone()]);
    assert_eq!(path(&s, a).subpaths[1], retained, "AVERAGE_JOIN_SURVIVOR_CONTOUR_GEOMETRY");
    assert_eq!(path(&s, a), joined, "AVERAGE_JOIN_SURVIVOR_JOINED_GEOMETRY");
    assert!(s.doc().unwrap().doc.node(b).is_none(), "AVERAGE_JOIN_SURVIVOR_B_REMOVED");
    eprintln!("AVERAGE_JOIN_SURVIVOR_GEOMETRY_PRESERVED");
    let mut expected_selection = before.selection.clone();
    expected_selection.set([a]);
    expected_selection.anchors.insert(a, [(0, 1)].into());
    assert_eq!(s.doc().unwrap().selection, expected_selection, "AVERAGE_JOIN_SURVIVOR_SELECTION");

    let mut expected = before.doc.clone();
    let d = Arc::make_mut(&mut expected);
    expected_path(d, a, joined);
    d.remove(b).unwrap();
    // As in the retained-B oracle, only placement storage may differ; the late pixel constrains it.
    let Paint::Gradient(actual_gradient) = s.doc().unwrap().doc.node(a).unwrap().appearance.fill_paint() else {
        panic!("AVERAGE_JOIN_SURVIVOR_PAINT_KIND: A lost its gradient")
    };
    let Paint::Gradient(expected_gradient) = &mut d.node_mut(a).unwrap().appearance.fill_mut().unwrap().paint else {
        panic!("AVERAGE_JOIN_SURVIVOR_EXPECTED_PAINT")
    };
    expected_gradient.geom = actual_gradient.geom;
    assert_eq!(s.doc().unwrap().doc, expected, "AVERAGE_JOIN_SURVIVOR_DOCUMENT");
    eprintln!("AVERAGE_JOIN_SURVIVOR_DOCUMENT_PRESERVED");
    let joined_pixel = pixel(&s);
    let after = Snapshot::of(&s);
    assert_eq!(after.undo.len(), before.undo.len() + 1);
    assert_eq!(&after.undo[..before.undo.len()], before.undo.as_slice());
    assert_eq!(after.undo.last().unwrap(), &("Average and Join".into(), before.doc.clone(), Arc::new(before.selection.clone())));
    assert!(after.redo.is_empty() && after.dirty);

    assert_eq!(s.execute("edit.undo", &json!({})).unwrap()["undone"], "Average and Join");
    let undone = Snapshot::of(&s);
    assert_eq!(undone.doc, before.doc);
    assert_eq!(undone.selection, before.selection);
    assert_eq!(undone.undo, before.undo);
    assert_eq!(undone.redo, vec![("Average and Join".into(), after.doc.clone(), Arc::new(after.selection.clone()))]);
    assert_eq!(undone.dirty, before.dirty);
    assert_eq!(pixel(&s), [0, 0, 255, 255], "AVERAGE_JOIN_SURVIVOR_UNDO_PIXEL");
    assert_eq!(s.execute("edit.redo", &json!({})).unwrap()["redone"], "Average and Join");
    after.assert_unchanged(&s);
    let redo_pixel = pixel(&s);
    eprintln!("AVERAGE_JOIN_SURVIVOR_HISTORY_PRESERVED");
    eprintln!("AVERAGE_JOIN_SURVIVOR_APPEARANCE_CHECK: joined={joined_pixel:?}, redo={redo_pixel:?}");
    // Candidate-baseline RED must reach this late oracle; command/setup/history failure is invalid.
    assert_eq!(
        [joined_pixel, redo_pixel],
        [[0, 0, 255, 255]; 2],
        "AVERAGE_JOIN_SURVIVOR_RECOLOR: untouched survivor-A contour must remain original blue"
    );
}
