//! External batches during an idle Selection tool's Color panel preview.
//! One in-memory rectangle per case; no pointer gestures, lenders or filesystem fixtures.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use serde_json::{Value, json};
use std::sync::Arc;
use vectorcraft_color::{Color, GradientPaint, Paint};
use vectorcraft_doc::NodeId;
use vectorcraft_engine::{DocState, History, HistoryEntry, Session, ViewInfo};

const PREVIEW_LABEL: &str = "Idle Fill Preview";
const BATCH_LABEL: &str = "Tiny Rectangle Edit";

#[derive(Clone, Debug, PartialEq)]
struct CommittedPaint {
    solid: Color,
    gradient: GradientPaint,
    recent: Vec<Color>,
}

fn committed(s: &Session) -> CommittedPaint {
    CommittedPaint { solid: s.last_solid, gradient: s.last_gradient.clone(), recent: s.recent_colors.clone() }
}

#[derive(Clone, Copy)]
enum PreviewKind {
    Solid,
    Gradient,
}

impl PreviewKind {
    fn params(self, continued: bool) -> Value {
        match (self, continued) {
            (Self::Solid, false) => json!({"color": "#2468ac"}),
            (Self::Solid, true) => json!({"color": "#abcdef"}),
            (Self::Gradient, false) => gradient("linear", "#2468ac", "#ac6824", 20),
            (Self::Gradient, true) => gradient("radial", "#abcdef", "#fedcba", 60),
        }
    }
}

fn gradient(kind: &str, first: &str, second: &str, angle: i32) -> Value {
    json!({"gradient": {"kind": kind, "angle": angle, "stops": [
        {"offset": 0, "color": first}, {"offset": 1, "color": second}
    ]}})
}

fn movement(dx: i32, dy: i32) -> Value {
    json!({"dx": dx, "dy": dy, "strokes": false, "corners": false, "patterns": false})
}

struct Fixture {
    s: Session,
    id: NodeId,
    base: DocState,
    memory: CommittedPaint,
    journal: Vec<(String, Value)>,
    bounds: [f64; 4],
}

fn fixture(kind: PreviewKind) -> Fixture {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 64, "height": 48, "created": 1_000_000_000})).unwrap();
    s.select_tool("selection", ViewInfo::default()).unwrap();
    let result = s.execute("shape.rectangle", &json!({"x": 10, "y": 10, "width": 12, "height": 8})).unwrap();
    let id = NodeId(result["id"].as_u64().unwrap());
    // Distinct committed solid and gradient memories expose speculative cache changes.
    s.execute("paint.setFill", &gradient("radial", "#102030", "#405060", 0)).unwrap();
    s.execute("paint.setFill", &json!({"color": "#13579b"})).unwrap();
    // Keep a real redo entry: rejection and Cancel must preserve both history branches.
    s.execute("object.move", &movement(1, 1)).unwrap();
    s.execute("edit.undo", &json!({})).unwrap();
    let base = s.doc().unwrap().clone();
    let memory = committed(&s);
    let journal = s.journal.clone();
    let bounds = bounds(&s, id);
    assert!(!base.history.undo.is_empty() && !base.history.redo.is_empty());
    assert_eq!(memory.solid.to_hex(), "#13579b");
    assert_eq!(base.selection.objects, vec![id]);
    assert_idle(&s, false);

    s.begin_interaction(PREVIEW_LABEL).unwrap();
    s.preview("paint.setFill", &kind.params(false)).unwrap();
    assert_idle(&s, true);
    assert_ne!(document(s.doc().unwrap()), document(&base), "the fill preview is visible");
    assert_preview_paint(&s, id, kind, false);
    assert_eq!(committed(&s), memory, "preview paint is still pending");
    assert_history(&s.doc().unwrap().history, &base.history);
    assert_eq!(s.journal, journal);
    Fixture { s, id, base, memory, journal, bounds }
}

fn document(st: &DocState) -> Value {
    serde_json::to_value(&*st.doc).unwrap()
}

fn bounds(s: &Session, id: NodeId) -> [f64; 4] {
    let b = s.transform_box(&[id]).unwrap();
    assert_eq!(b.angle, 0.0);
    [b.rect.x0, b.rect.y0, b.rect.x1, b.rect.y1]
}

fn assert_idle(s: &Session, interaction: bool) {
    assert_eq!(s.tool_id(), "selection");
    assert!(!s.tool_busy(), "this case exercises an idle tool with a panel interaction");
    assert_eq!(s.in_interaction(), interaction);
    assert_eq!(s.documents().len(), 1);
    assert_eq!(s.active_index(), Some(0));
}

fn assert_entries(actual: &[HistoryEntry], expected: &[HistoryEntry]) {
    assert_eq!(actual.len(), expected.len());
    for (a, e) in actual.iter().zip(expected) {
        assert_eq!(a.label, e.label);
        assert_eq!(a.selection, e.selection);
        assert!(Arc::ptr_eq(&a.doc, &e.doc), "existing history snapshots must be retained");
    }
}

fn assert_history(actual: &History, expected: &History) {
    assert_eq!(actual.limit, expected.limit);
    assert_entries(&actual.undo, &expected.undo);
    assert_entries(&actual.redo, &expected.redo);
}

fn assert_art(actual: &DocState, expected: &DocState) {
    assert_eq!(document(actual), document(expected));
    assert_eq!(actual.selection, expected.selection);
}

fn assert_retained(actual: &DocState, expected: &DocState) {
    assert_art(actual, expected);
    assert_history(&actual.history, &expected.history);
    assert_eq!(actual.uid, expected.uid);
    assert_eq!(actual.path, expected.path);
    assert_eq!(actual.is_dirty(), expected.is_dirty());
    assert_eq!(actual.active_layer, expected.active_layer);
    assert_eq!(actual.layer_rows, expected.layer_rows);
    assert_eq!(actual.isolation, expected.isolation);
    // Revision is a redraw counter; general batch rollback may advance it.
    match (&actual.interaction, &expected.interaction) {
        (Some(a), Some(e)) => {
            assert_eq!(a.label, e.label);
            assert_eq!(*a.doc, *e.doc);
            assert_eq!(a.selection, e.selection);
            assert_eq!(a.preview, e.preview);
            assert_eq!(a.active_layer, e.active_layer);
            assert_eq!(a.layer_rows, e.layer_rows);
            assert_eq!(a.isolation, e.isolation);
            assert_eq!(a.perspective_again, e.perspective_again);
        }
        (None, None) => {}
        _ => panic!("the original interaction must be retained"),
    }
}

fn assert_preview_paint(s: &Session, id: NodeId, kind: PreviewKind, continued: bool) {
    let node = s.doc().unwrap().doc.node(id).unwrap();
    let visible = node.appearance.fill_paint();
    assert_eq!(s.proxy_paints().0, visible);
    assert!(s.fill_active);
    match kind {
        PreviewKind::Solid => {
            let hex = if continued { "#abcdef" } else { "#2468ac" };
            let color = Color::from_hex(hex).unwrap();
            assert_eq!(visible.color(), Some(color));
            assert_eq!(s.paint.fill.color(), Some(color));
        }
        PreviewKind::Gradient => {
            let Paint::Gradient(shown) = visible else { panic!("visible fill is not a gradient") };
            let Paint::Gradient(default) = &s.paint.fill else { panic!("default fill is not a gradient") };
            let colors = if continued { ["#abcdef", "#fedcba"] } else { ["#2468ac", "#ac6824"] };
            assert_eq!(shown.gradient.stops.len(), 2);
            assert_eq!(shown.gradient.stops[0].offset, 0.0);
            assert_eq!(shown.gradient.stops[1].offset, 1.0);
            assert_eq!(shown.gradient.stops[0].color.to_hex(), colors[0]);
            assert_eq!(shown.gradient.stops[1].color.to_hex(), colors[1]);
            assert_eq!(shown.gradient, default.gradient);
            assert_eq!(shown.gradient.kind.label(), if continued { "Radial" } else { "Linear" });
            assert_eq!(shown.angle, if continued { 60.0 } else { 20.0 });
            assert!(default.geom.is_none(), "the remembered default is unplaced");
        }
    }
}

fn expected_commit(before: &CommittedPaint, fill: &Paint) -> CommittedPaint {
    let mut expected = before.clone();
    match fill {
        Paint::Solid { color, .. } => {
            assert!(!expected.recent.contains(color), "fixture colors are distinct");
            expected.solid = *color;
            expected.recent.insert(0, *color);
        }
        Paint::Gradient(g) => expected.gradient = (**g).clone(),
        _ => panic!("fixture uses only solid colors and two-stop gradients"),
    }
    expected
}

fn assert_replay(s: &Session) {
    let mut replay = Session::new();
    for (command, params) in &s.journal {
        replay.execute(command, params).unwrap();
    }
    assert_idle(&replay, false);
    assert_art(replay.doc().unwrap(), s.doc().unwrap());
    assert_eq!(committed(&replay), committed(s));
    assert_eq!(replay.journal, s.journal);
    // Cancelled panel edits may leave paint defaults changed without committing them.
    // Replay checks the document, selection and committed paint memory instead.
    // Live preview labels and replayed command labels need not be identical.
}

fn assert_preview_commit(f: &Fixture, final_doc: &DocState, fill: &Paint, params: &Value) {
    assert_idle(&f.s, false);
    assert_art(f.s.doc().unwrap(), final_doc);
    let h = &f.s.doc().unwrap().history;
    assert_eq!(h.undo.len(), f.base.history.undo.len() + 1);
    assert!(h.redo.is_empty());
    assert_entries(&h.undo[..f.base.history.undo.len()], &f.base.history.undo);
    let entry = h.undo.last().unwrap();
    assert_eq!(entry.label, PREVIEW_LABEL);
    assert_eq!(*entry.doc, *f.base.doc);
    assert_eq!(entry.selection, f.base.selection);
    let mut journal = f.journal.clone();
    journal.push(("paint.setFill".into(), params.clone()));
    assert_eq!(f.s.journal, journal, "the rejected batch is absent");
    assert_eq!(committed(&f.s), expected_commit(&f.memory, fill));
    assert_replay(&f.s);
}

#[test]
fn accepted_external_batch_keeps_idle_fill_preview_as_a_separate_undo_and_replay_step() {
    let mut f = fixture(PreviewKind::Solid);
    let preview = f.s.doc().unwrap().clone();
    let preview_fill = f.s.paint.fill.clone();
    let batch = json!({"label": BATCH_LABEL, "commands": [
        {"command": "object.move", "params": movement(2, 3)},
        {"command": "object.setProps", "params": {"id": f.id.0, "name": "Batch Edited"}}
    ]});
    let result = f.s.execute("command.batch", &batch).unwrap();
    assert_eq!(result["results"].as_array().unwrap().len(), 2);
    assert_idle(&f.s, false);
    assert_preview_paint(&f.s, f.id, PreviewKind::Solid, false);
    assert_eq!(bounds(&f.s, f.id), [f.bounds[0] + 2.0, f.bounds[1] + 3.0, f.bounds[2] + 2.0, f.bounds[3] + 3.0]);
    assert_eq!(f.s.doc().unwrap().doc.node(f.id).unwrap().name.as_deref(), Some("Batch Edited"));
    assert_eq!(committed(&f.s), expected_commit(&f.memory, &preview_fill));
    let accepted = f.s.doc().unwrap().clone();
    let mut journal = f.journal.clone();
    journal.push(("paint.setFill".into(), PreviewKind::Solid.params(false)));
    journal.push(("command.batch".into(), batch));
    // Replay now, before any subsequent edit could conceal a missing preview journal entry.
    assert_replay(&f.s);
    assert_eq!(f.s.journal, journal, "journal the visible fill before the independent batch");
    let h = &accepted.history;
    assert_eq!(h.undo.len(), f.base.history.undo.len() + 2);
    assert!(h.redo.is_empty());
    assert_entries(&h.undo[..f.base.history.undo.len()], &f.base.history.undo);
    let preview_entry = &h.undo[f.base.history.undo.len()];
    let batch_entry = h.undo.last().unwrap();
    assert_eq!(preview_entry.label, PREVIEW_LABEL);
    assert_eq!(*preview_entry.doc, *f.base.doc);
    assert_eq!(preview_entry.selection, f.base.selection);
    assert_eq!(batch_entry.label, BATCH_LABEL);
    assert_eq!(*batch_entry.doc, *preview.doc);
    assert_eq!(batch_entry.selection, preview.selection);

    // The accepted batch ends the old panel interaction; a late preview cannot replace its edit.
    let paint = f.s.paint.clone();
    assert!(f.s.preview("paint.setFill", &PreviewKind::Solid.params(true)).is_err());
    assert_retained(f.s.doc().unwrap(), &accepted);
    assert_eq!(f.s.paint, paint);
    assert_eq!(f.s.journal, journal);
    assert_eq!(committed(&f.s), expected_commit(&f.memory, &preview_fill));
    assert_eq!(f.s.execute("edit.undo", &json!({})).unwrap()["undone"], BATCH_LABEL);
    assert_art(f.s.doc().unwrap(), &preview);
    assert_replay(&f.s);
    assert_eq!(f.s.execute("edit.undo", &json!({})).unwrap()["undone"], PREVIEW_LABEL);
    assert_art(f.s.doc().unwrap(), &f.base);
    assert_replay(&f.s);
    f.s.execute("edit.redo", &json!({})).unwrap();
    assert_art(f.s.doc().unwrap(), &preview);
    assert_replay(&f.s);
    f.s.execute("edit.redo", &json!({})).unwrap();
    assert_art(f.s.doc().unwrap(), &accepted);
    assert_replay(&f.s);
}

enum Ending {
    ContinueCancel,
    CommitOriginal,
    ContinueCommit,
}

fn rejected_external_batch(kind: PreviewKind, ending: Ending) {
    let mut f = fixture(kind);
    let preview = f.s.doc().unwrap().clone();
    let paint = f.s.paint.clone();
    let fill_active = f.s.fill_active;
    let options = f.s.tool_options();
    let error =
        f.s.execute(
            "command.batch",
            &json!({"label": BATCH_LABEL, "commands": [
                {"command": "object.move", "params": movement(2, 3)},
                {"command": "no.such.command", "params": {}}
            ]}),
        )
        .unwrap_err()
        .to_string();
    assert!(error.contains("batch step 1") && error.contains("no.such.command"), "{error}");
    assert_idle(&f.s, true);
    assert_retained(f.s.doc().unwrap(), &preview);
    assert_eq!(bounds(&f.s, f.id), f.bounds);
    assert_eq!(f.s.paint, paint, "visible panel defaults survive rejection");
    assert_eq!(f.s.fill_active, fill_active);
    assert_eq!(f.s.tool_options(), options);
    assert_eq!(f.s.journal, f.journal, "no speculative preview commit or failed batch entry");
    assert_eq!(committed(&f.s), f.memory, "rejection must restore all committed paint memory");
    assert_preview_paint(&f.s, f.id, kind, false);

    if matches!(ending, Ending::CommitOriginal) {
        // No new paint call: this detects pending paint lost during the rejected batch.
        f.s.commit_interaction().unwrap();
        assert_preview_commit(&f, &preview, &paint.fill, &kind.params(false));
        f.s.execute("edit.undo", &json!({})).unwrap();
        assert_art(f.s.doc().unwrap(), &f.base);
        assert_replay(&f.s);
        f.s.execute("edit.redo", &json!({})).unwrap();
        assert_art(f.s.doc().unwrap(), &preview);
        assert_replay(&f.s);
        return;
    }

    f.s.preview("paint.setFill", &kind.params(true)).unwrap();
    assert_idle(&f.s, true);
    assert_preview_paint(&f.s, f.id, kind, true);
    assert_eq!(bounds(&f.s, f.id), f.bounds, "the failed move cannot leak into continuation");
    assert_ne!(document(f.s.doc().unwrap()), document(&preview));
    assert_eq!(committed(&f.s), f.memory, "continuation is still pending");
    assert_history(&f.s.doc().unwrap().history, &f.base.history);
    assert_eq!(f.s.journal, f.journal);
    let continued = f.s.doc().unwrap().clone();
    let continued_fill = f.s.paint.fill.clone();
    match ending {
        Ending::ContinueCancel => {
            f.s.cancel_interaction().unwrap();
            assert_idle(&f.s, false);
            assert_retained(f.s.doc().unwrap(), &f.base);
            assert_eq!(committed(&f.s), f.memory, "Cancel discards both preview paints");
            assert_eq!(f.s.journal, f.journal);
            // Commit without an interaction must not remember stale pending paint.
            f.s.commit_interaction().unwrap();
            assert_eq!(committed(&f.s), f.memory);
            assert_eq!(f.s.journal, f.journal);
            assert_replay(&f.s);
            f.s.execute("edit.redo", &json!({})).unwrap();
            assert_eq!(bounds(&f.s, f.id), [f.bounds[0] + 1.0, f.bounds[1] + 1.0, f.bounds[2] + 1.0, f.bounds[3] + 1.0]);
            assert_replay(&f.s);
            f.s.execute("edit.undo", &json!({})).unwrap();
            assert_art(f.s.doc().unwrap(), &f.base);
            assert_eq!(committed(&f.s), f.memory);
            assert_replay(&f.s);
        }
        Ending::ContinueCommit => {
            f.s.commit_interaction().unwrap();
            assert_preview_commit(&f, &continued, &continued_fill, &kind.params(true));
            f.s.execute("edit.undo", &json!({})).unwrap();
            assert_art(f.s.doc().unwrap(), &f.base);
            assert_replay(&f.s);
            f.s.execute("edit.redo", &json!({})).unwrap();
            assert_art(f.s.doc().unwrap(), &continued);
            assert_replay(&f.s);
        }
        Ending::CommitOriginal => panic!("handled before continuation"),
    }
}

#[test]
fn rejected_external_batch_retains_idle_solid_preview_for_continuation_and_cancel() {
    rejected_external_batch(PreviewKind::Solid, Ending::ContinueCancel);
}

#[test]
fn rejected_external_batch_retains_pending_solid_for_commit_without_another_preview() {
    rejected_external_batch(PreviewKind::Solid, Ending::CommitOriginal);
}

#[test]
fn rejected_external_batch_retains_idle_solid_preview_for_continuation_and_commit() {
    rejected_external_batch(PreviewKind::Solid, Ending::ContinueCommit);
}

#[test]
fn rejected_external_batch_retains_idle_two_stop_gradient_for_continuation_and_cancel() {
    rejected_external_batch(PreviewKind::Gradient, Ending::ContinueCancel);
}

#[test]
fn rejected_external_batch_retains_pending_two_stop_gradient_for_commit_without_another_preview() {
    rejected_external_batch(PreviewKind::Gradient, Ending::CommitOriginal);
}

#[test]
fn rejected_external_batch_retains_idle_two_stop_gradient_for_continuation_and_commit() {
    rejected_external_batch(PreviewKind::Gradient, Ending::ContinueCommit);
}
