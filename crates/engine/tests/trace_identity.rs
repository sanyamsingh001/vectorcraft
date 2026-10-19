//! Renaming editable Image Trace art must preserve its command identity.
//! Original 8x8 PNG generated in memory; no file, font, UI backend or timing fixture.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use serde_json::{Value, json};
use vectorcraft_doc::{NodeId, NodeKind, TraceView};
use vectorcraft_engine::Session;
use vectorcraft_trace::Raster;

fn node_json(s: &Session, id: NodeId) -> Value {
    serde_json::to_value(s.doc().unwrap().doc.node(id).unwrap()).unwrap()
}

fn renamed_trace() -> (Session, NodeId, NodeId) {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 32, "height": 24, "created": 1_000_000_000})).unwrap();
    let image = Raster::from_fn(8, 8, |x, _| if x < 4 { [0, 0, 0, 255] } else { [255; 4] });
    let png = image.encode_png();
    assert!(!png.is_empty());
    let placed = s
        .execute("file.place", &json!({"name": "original-stripe.png", "dataBase64": vectorcraft_format::base64_encode(&png), "link": false}))
        .unwrap();
    assert_eq!(placed["ids"].as_array().unwrap().len(), 1);
    let source = NodeId(placed["ids"][0].as_u64().unwrap());
    let result = s.execute("imageTrace.make", &json!({"params": {"noise": 1, "ignoreWhite": true}})).unwrap();
    assert_eq!(result["paths"], 1);
    let group = NodeId(result["id"].as_u64().unwrap());
    let before = s.doc().unwrap().doc.node(group).unwrap().trace.clone();
    assert!(before.is_some());
    assert_eq!(s.doc().unwrap().doc.node(group).unwrap().name.as_deref(), Some("Image Trace"), "new traces keep the default name");
    // This is the same ordinary rename command the Layers panel emits.
    s.execute("layer.setProps", &json!({"ids": [group.0], "name": "Logo outline"})).unwrap();
    let renamed = s.doc().unwrap().doc.node(group).unwrap();
    assert_eq!(renamed.name.as_deref(), Some("Logo outline"));
    assert_eq!(renamed.trace, before, "renaming has not expanded or released the trace");
    assert!(matches!(renamed.kind, NodeKind::Group { clip: false, .. }));
    let child = renamed.children().unwrap().first().unwrap();
    assert_eq!(child.id, source);
    assert!(matches!(child.kind, NodeKind::Image(_)) && !child.visible);
    assert_eq!(s.doc().unwrap().selection.objects, vec![group]);
    assert!(!s.in_interaction() && !s.tool_busy());
    (s, group, source)
}

#[test]
fn renamed_trace_can_change_view_and_undo_redo_that_change() {
    let (mut s, group, _) = renamed_trace();
    let before = node_json(&s, group);
    let steps = s.doc().unwrap().history.undo.len();
    let result = s.execute("imageTrace.setView", &json!({"id": group.0, "view": "sourceImage"})).unwrap();
    assert_eq!(result["ids"], json!([group.0]));
    let node = s.doc().unwrap().doc.node(group).unwrap();
    assert_eq!(node.name.as_deref(), Some("Logo outline"));
    assert_eq!(node.trace_view(), TraceView::Source);
    assert_eq!(s.doc().unwrap().history.undo.len(), steps + 1);
    let shown = node_json(&s, group);
    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(node_json(&s, group), before);
    s.execute("edit.redo", &json!({})).unwrap();
    assert_eq!(node_json(&s, group), shown);
}

#[test]
fn renamed_trace_can_release_the_original_image_and_undo_redo() {
    let (mut s, group, source) = renamed_trace();
    let before = node_json(&s, group);
    let result = s.execute("imageTrace.release", &json!({"id": group.0})).unwrap();
    assert_eq!(result["ids"], json!([source.0]));
    assert!(s.doc().unwrap().doc.node(group).is_none());
    let image = s.doc().unwrap().doc.node(source).unwrap();
    assert!(matches!(image.kind, NodeKind::Image(_)) && image.visible);
    assert_eq!(s.doc().unwrap().selection.objects, vec![source]);
    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(node_json(&s, group), before);
    s.execute("edit.redo", &json!({})).unwrap();
    assert!(s.doc().unwrap().doc.node(group).is_none());
    assert!(s.doc().unwrap().doc.node(source).unwrap().visible);
}

#[test]
fn renamed_trace_can_expand_then_behaves_as_an_ordinary_group() {
    let (mut s, group, source) = renamed_trace();
    let before = node_json(&s, group);
    let result = s.execute("imageTrace.expand", &json!({"id": group.0})).unwrap();
    assert_eq!(result["ids"], json!([group.0]));
    let expanded = s.doc().unwrap().doc.node(group).unwrap();
    assert_eq!(expanded.name.as_deref(), Some("Logo outline"), "expanding a trace keeps the user's layer name");
    assert!(expanded.trace.is_none());
    assert!(expanded.children().unwrap().iter().all(|n| !matches!(n.kind, NodeKind::Image(_))));
    assert!(s.doc().unwrap().doc.node(source).is_none());
    let expanded = node_json(&s, group);
    assert!(s.execute("imageTrace.release", &json!({"id": group.0})).is_err());
    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(node_json(&s, group), before);
    s.execute("edit.redo", &json!({})).unwrap();
    assert_eq!(node_json(&s, group), expanded);
}

#[test]
fn renamed_trace_can_be_retraced_from_its_original_image() {
    let (mut s, group, source) = renamed_trace();
    let before = node_json(&s, group);
    let result = s.execute("imageTrace.make", &json!({"id": group.0, "params": {"noise": 1, "ignoreWhite": true, "paths": 60}})).unwrap();
    assert_eq!(result["paths"], 1);
    let next = NodeId(result["id"].as_u64().unwrap());
    assert!(s.doc().unwrap().doc.node(group).is_none());
    let retraced = s.doc().unwrap().doc.node(next).unwrap();
    assert_eq!(retraced.name.as_deref(), Some("Logo outline"), "changing trace settings preserves the user's layer name");
    assert_eq!(retraced.children().unwrap().first().unwrap().id, source);
    assert!(!retraced.children().unwrap().first().unwrap().visible);
    assert_eq!(retraced.trace.as_ref().unwrap()["params"]["paths"], 60.0);
    assert_eq!(s.doc().unwrap().selection.objects, vec![next]);
    let after = node_json(&s, next);
    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(node_json(&s, group), before);
    s.execute("edit.redo", &json!({})).unwrap();
    assert_eq!(node_json(&s, next), after);
}

#[test]
fn make_and_expand_keeps_custom_trace_name_and_default_name_control() {
    for custom in [false, true] {
        let (mut s, group, source) = renamed_trace();
        if !custom {
            s.execute("layer.setProps", &json!({"ids": [group.0], "name": "Image Trace"})).unwrap();
        }
        let before = serde_json::to_value(s.doc().unwrap().doc.as_ref()).unwrap();
        let steps = s.doc().unwrap().history.undo.len();
        let result = s.execute("imageTrace.makeAndExpand", &json!({"id": group.0, "params": {"noise": 1, "ignoreWhite": true}})).unwrap();
        assert_eq!(result["paths"], 1);
        let next = NodeId(result["id"].as_u64().unwrap());
        assert_ne!(next, group);
        assert!(s.doc().unwrap().doc.node(group).is_none());
        assert!(s.doc().unwrap().doc.node(source).is_none());
        let expanded = s.doc().unwrap().doc.node(next).unwrap();
        assert!(expanded.trace.is_none());
        assert!(expanded.children().unwrap().iter().all(|n| !matches!(n.kind, NodeKind::Image(_))));
        assert!(!expanded.is_image_trace());
        assert_eq!(s.doc().unwrap().selection.objects, vec![next]);
        assert_eq!(s.doc().unwrap().history.undo.len(), steps + 1);
        assert_eq!(
            expanded.name.as_deref(),
            custom.then_some("Logo outline"),
            "Make and Expand keeps a custom name and clears the default trace marker"
        );
        let after = serde_json::to_value(s.doc().unwrap().doc.as_ref()).unwrap();
        s.execute("edit.undo", &json!({})).unwrap();
        assert_eq!(serde_json::to_value(s.doc().unwrap().doc.as_ref()).unwrap(), before);
        s.execute("edit.redo", &json!({})).unwrap();
        assert_eq!(serde_json::to_value(s.doc().unwrap().doc.as_ref()).unwrap(), after);
    }
}
