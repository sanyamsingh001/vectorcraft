//! SVG files that reopen as they were: Preserve Editing restores the document exactly (unless the
//! SVG was edited elsewhere), Save keeps hidden layers, symbols share one def, and linked images
//! stay linked.

use serde_json::{Value, json};
use vectorcraft_doc::{Document, NodeKind};

use super::tests_svg::{image_session, svg};
use super::*;

fn new_id(v: &Value) -> u64 {
    v["id"].as_u64().or_else(|| v["ids"][0].as_u64()).unwrap_or_else(|| panic!("no id in {v}"))
}

fn rect(s: &mut Session, x: f64, y: f64) -> u64 {
    new_id(&s.execute("shape.rectangle", &json!({"x": x, "y": y, "width": 20, "height": 20})).unwrap())
}

fn select(s: &mut Session, ids: &[u64]) {
    s.execute("select.set", &json!({ "ids": ids })).unwrap();
}

/// A roughened square, three instances of a symbol and a square filled with a pattern.
pub(super) fn rich() -> Session {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 300, "height": 200})).unwrap();
    let rough = rect(&mut s, 10.0, 10.0);
    s.execute("effect.apply", &json!({"effect": "distort.roughen", "ids": [rough]})).unwrap();
    let mark = rect(&mut s, 60.0, 10.0);
    s.execute("symbol.new", &json!({"name": "Mark", "ids": [mark]})).unwrap();
    for (x, y) in [(150.0, 50.0), (200.0, 120.0)] {
        s.execute("symbol.place", &json!({"name": "Mark", "x": x, "y": y})).unwrap();
    }
    let dot = rect(&mut s, 0.0, 150.0);
    s.execute("paint.setFill", &json!({"ids": [dot], "color": "#ff0000"})).unwrap();
    select(&mut s, &[dot]);
    s.execute("object.pattern.make", &json!({"name": "Dots", "width": 30, "height": 30})).unwrap();
    s.execute("object.pattern.done", &json!({})).unwrap();
    let filled = rect(&mut s, 100.0, 150.0);
    s.execute("paint.setFill", &json!({"ids": [filled], "swatch": "Dots"})).unwrap();
    s
}

fn open(s: &mut Session, name: &str, text: &str) -> Value {
    s.execute("document.open", &json!({"name": name, "dataBase64": vectorcraft_format::base64_encode(text.as_bytes())})).unwrap()
}

/// `d` without what opening a file sets anew.
pub(super) fn comparable(d: &Document) -> Document {
    let mut d = d.clone();
    d.title.clear();
    d
}

#[test]
fn preserve_editing_restores_effects_symbols_and_patterns_exactly() {
    let mut s = rich();
    let before = comparable(&s.doc().unwrap().doc);
    assert!(before.symbols.iter().any(|x| x.name == "Mark") && before.pattern("Dots").is_some());
    let text = svg(&mut s, json!({"preserveEditing": true, "styling": "css"}));
    assert_eq!(text.matches("<use ").count(), 3, "{text}");
    let r = open(&mut s, "rich.svg", &text);
    assert_eq!(r["warnings"], json!([]), "{r}");
    let after = comparable(&s.doc().unwrap().doc);
    assert_eq!(after, before, "the document came back exactly");
    let mut effects = 0;
    after.walk(|n| effects += n.appearance.effects.len());
    assert_eq!(effects, 1, "the live effect is still live");
}

#[test]
fn an_svg_edited_elsewhere_opens_as_plain_svg_with_a_warning() {
    let mut s = rich();
    let text = svg(&mut s, json!({"preserveEditing": true}));
    // Another app changes a colour (the pattern's red).
    let edited = text.replacen("#ff0000", "#00ff00", 1);
    assert_ne!(edited, text);
    let r = open(&mut s, "edited.svg", &edited);
    assert_eq!(r["warnings"][0], load::EDITING_STALE, "{r}");
    let d = &s.doc().unwrap().doc;
    // (SVG import keeps symbols since M4.41, so the roughen's result tells the plain import.)
    let mut effects = 0;
    d.walk(|n| effects += n.appearance.effects.len());
    assert!(effects == 0 && d.patterns.iter().all(|p| p.name != "Dots"), "plain import: the roughen as its result, no named pattern");
    // Unreadable editing data (the hash only covers the markup around it) falls back too.
    let damaged = text.replacen("<![CDATA[", "<![CDATA[!!", 1);
    let r = open(&mut s, "damaged.svg", &damaged);
    assert_eq!(r["warnings"][0], load::EDITING_DAMAGED, "{r}");
}

#[test]
fn save_keeps_hidden_layers_and_export_leaves_them_out() {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 100, "height": 100})).unwrap();
    rect(&mut s, 10.0, 10.0);
    let notes = new_id(&s.execute("layer.new", &json!({"name": "Notes"})).unwrap());
    rect(&mut s, 50.0, 50.0);
    s.execute("layer.setProps", &json!({"id": notes, "visible": false})).unwrap();

    let exported = svg(&mut s, json!({}));
    assert!(!exported.contains("Notes"), "{exported}");
    let saved = s.execute("document.save", &json!({"format": "svg"})).unwrap();
    let saved = String::from_utf8(vectorcraft_format::base64_decode(saved["dataBase64"].as_str().unwrap()).unwrap()).unwrap();
    assert!(saved.contains("id=\"Notes\"") && saved.contains("display=\"none\""), "{saved}");
    // Asked not to, Save leaves them out too.
    let r = s.execute("document.save", &json!({"format": "svg", "svg": {"hiddenLayers": false}})).unwrap();
    assert!(!String::from_utf8(vectorcraft_format::base64_decode(r["dataBase64"].as_str().unwrap()).unwrap()).unwrap().contains("Notes"));

    open(&mut s, "saved.svg", &saved);
    let d = &s.doc().unwrap().doc;
    let layers: Vec<(Option<&str>, bool, usize)> =
        d.layers.iter().map(|l| (l.name.as_deref(), l.visible, l.children().map_or(0, Vec::len))).collect();
    assert_eq!(layers, [(Some("Layer 1"), true, 1), (Some("Notes"), false, 1)]);
}

#[test]
fn link_mode_writes_no_data_uri() {
    let mut s = image_session();
    // An image placed linked: it has both its bytes and its file.
    s.edit("Link", |d, _| {
        let id = d.layers[0].children().and_then(|c| c.first()).map(|n| n.id).unwrap();
        if let Some(NodeKind::Image(im)) = d.node_mut(id).map(|n| &mut n.kind) {
            im.link = Some("art/dot.png".into());
        }
        Ok(())
    })
    .unwrap();
    let linked = svg(&mut s, json!({"images": "link"}));
    assert!(linked.contains("xlink:href=\"art/dot.png\"") && !linked.contains("data:"), "{linked}");
    let embedded = svg(&mut s, json!({"images": "embed"}));
    assert!(embedded.contains("href=\"data:image/png;base64,") && !embedded.contains("art/dot.png"), "{embedded}");
}

// A clean-room synthetic rig: one visible arm and hidden alternates with pivots.
const RIG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
    <g id="character" data-pivot="50,90">
        <g id="arm" data-pivot="20,30"><rect width="20" height="20" fill="#0055ff"/></g>
        <g id="alternate" display="none" data-pivot="20,30">
            <rect id="hand" width="50" height="50" data-note="&quot;&amp;&#xA;猫"/>
            <rect id="hidden-child" width="60" height="60" display="none" data-role="fist"/>
        </g>
    </g>
</svg>"##;

fn rig_node<'a>(doc: &'a Document, name: &str) -> &'a vectorcraft_doc::Node {
    let mut found = None;
    doc.walk(|n| {
        if n.name.as_deref() == Some(name) {
            found = Some(n);
        }
    });
    found.unwrap()
}

fn saved_svg(s: &mut Session, options: Value) -> String {
    let r = s.execute("document.save", &options).unwrap();
    String::from_utf8(vectorcraft_format::base64_decode(r["dataBase64"].as_str().unwrap()).unwrap()).unwrap()
}

#[test]
fn svg_rig_native_save_reopen_and_svg_save_preserve_hidden_art_and_data() {
    let mut s = Session::new();
    open(&mut s, "rig.svg", RIG);
    let before = comparable(&s.doc().unwrap().doc);
    let arm_id = rig_node(&before, "arm").id;
    assert!(!rig_node(&before, "alternate").visible);
    assert!(rig_node(&before, "hand").visible);
    assert_eq!(rig_node(&before, "hand").attrs.as_deref().unwrap().svg_data["data-note"], "\"&\n猫");
    for compress in [false, true] {
        let r = s.execute("document.save", &json!({"format": "vectorcraft", "compress": compress, "modified": null})).unwrap();
        let bytes = vectorcraft_format::base64_decode(r["dataBase64"].as_str().unwrap()).unwrap();
        let native = vectorcraft_format::load(&bytes).unwrap();
        assert_eq!(comparable(&native), before);
        s.execute("document.open", &json!({"name": "rig.vectorcraft", "dataBase64": r["dataBase64"]})).unwrap();
        assert_eq!(comparable(&s.doc().unwrap().doc), before);
    }
    // Changing existing Attributes-panel fields must retain SVG metadata.
    s.execute("attributes.set", &json!({"ids": [arm_id.0], "note": "Rig arm"})).unwrap();
    let output = saved_svg(&mut s, json!({"format": "svg", "modified": null}));
    assert!(output.contains("id=\"alternate\"") && output.contains("id=\"hidden-child\""));
    open(&mut s, "roundtrip.svg", &output);
    let doc = &s.doc().unwrap().doc;
    assert!(!rig_node(doc, "alternate").visible && !rig_node(doc, "hidden-child").visible);
    assert!(rig_node(doc, "hand").visible);
    assert_eq!(rig_node(doc, "character").attrs.as_deref().unwrap().svg_data["data-pivot"], "50,90");
    assert_eq!(rig_node(doc, "arm").attrs.as_deref().unwrap().svg_data["data-pivot"], "20,30");
    assert_eq!(rig_node(doc, "hand").attrs.as_deref().unwrap().svg_data["data-note"], "\"&\n猫");
}

#[test]
fn svg_rig_export_options_and_selection_stay_scoped() {
    let mut s = Session::new();
    open(&mut s, "rig.svg", RIG);
    let arm_id = rig_node(&s.doc().unwrap().doc, "arm").id;
    let normal = svg(&mut s, json!({}));
    assert!(!normal.contains("alternate") && normal.contains("data-pivot"));
    let kept = svg(&mut s, json!({"hiddenObjects": true, "hiddenLayers": true}));
    assert!(kept.contains("id=\"alternate\"") && kept.contains("display=\"none\""));
    s.execute("select.set", &json!({"ids": [arm_id.0]})).unwrap();
    let selection = svg(&mut s, json!({"selectedOnly": true, "hiddenObjects": true, "hiddenLayers": true}));
    assert!(selection.contains("id=\"arm\"") && selection.contains("data-pivot"));
    assert!(!selection.contains("alternate") && !selection.contains("hidden-child"));
    let omitted = saved_svg(&mut s, json!({"format": "svg", "svg": {"hiddenObjects": false}}));
    assert!(!omitted.contains("alternate") && !omitted.contains("hidden-child"));
}

#[test]
fn svg_rig_editing_data_retains_hidden_art_even_when_plain_markup_omits_it() {
    let mut s = Session::new();
    open(&mut s, "rig.svg", RIG);
    let before = comparable(&s.doc().unwrap().doc);
    let output = svg(&mut s, json!({"preserveEditing": true, "hiddenObjects": false}));
    assert!(!output.contains("id=\"alternate\""));
    let result = open(&mut s, "editable.svg", &output);
    assert_eq!(result["warnings"], json!([]));
    assert_eq!(comparable(&s.doc().unwrap().doc), before);
    // Editing the visible SVG invalidates the native snapshot: only plain markup comes back.
    let changed = output.replacen("#0055ff", "#00ff00", 1);
    assert_ne!(changed, output);
    let result = open(&mut s, "changed.svg", &changed);
    assert_eq!(result["warnings"][0], load::EDITING_STALE);
    let doc = &s.doc().unwrap().doc;
    let mut hidden = false;
    doc.walk(|n| hidden |= n.name.as_deref() == Some("alternate"));
    assert!(!hidden);
    assert_eq!(rig_node(doc, "arm").attrs.as_deref().unwrap().svg_data["data-pivot"], "20,30");
}

#[test]
fn invalid_svg_data_in_native_files_is_rejected_without_replacing_the_document() {
    let mut s = Session::new();
    open(&mut s, "rig.svg", RIG);
    let before = comparable(&s.doc().unwrap().doc);
    let id = rig_node(&before, "arm").id;
    for data in [
        [("onclick".to_string(), "invalid attribute name".to_string())].into_iter().collect(),
        [("data-invalid".to_string(), "invalid\0XML character".to_string())].into_iter().collect(),
        (0..=vectorcraft_doc::svg_data::MAX_ATTRIBUTES).map(|i| (format!("data-{i}"), "x".to_string())).collect(),
    ] {
        let mut damaged = before.clone();
        damaged.node_mut(id).unwrap().edit_attrs(|a| a.svg_data = data);
        let bytes = vectorcraft_format::save_with(&damaged, &vectorcraft_format::SaveOptions::default()).unwrap();
        assert!(vectorcraft_format::load(&bytes).is_err());
        assert!(
            s.execute("document.open", &json!({"name": "invalid.vectorcraft", "dataBase64": vectorcraft_format::base64_encode(&bytes)})).is_err()
        );
        assert_eq!(comparable(&s.doc().unwrap().doc), before);
    }
}
