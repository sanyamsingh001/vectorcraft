//! Native placement and linked-document conversion keep inline symbols editable, with their
//! source art and natural size. All scenes are original rectangles and a single inline run.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use serde_json::{Value, json};
use vectorcraft_color::{Color, Paint};
use vectorcraft_doc::{Appearance, CharStyle, Document, InlineArt, Node, NodeId, NodeKind, SYMBOL_SIZES, Symbol, TextObject, TextRun};
use vectorcraft_engine::Session;
use vectorcraft_geom::{Affine, Point, Rect, shapes};
use vectorcraft_testkit::fixtures::{self, exec};

fn look(rgb: [u8; 3]) -> Appearance {
    Appearance::basic(Paint::solid(Color::rgb8(rgb[0], rgb[1], rgb[2])), Paint::None, 0.0)
}

/// App-normalized blue art. Only the text references Dot unless ordinary is requested to isolate
/// metadata copying from inline-reference discovery.
fn child(ordinary: bool, size: Option<[f64; 2]>) -> Document {
    let mut d = Document::new(80.0, 60.0);
    d.title = "Original inline placement scene".into();
    d.symbols.push(Symbol {
        name: "Dot".into(),
        art: Arc::new(Node::path(NodeId(900), shapes::rectangle(Rect::new(-10.0, -10.0, 10.0, 10.0)), look([20, 20, 230]))),
    });
    if let Some(size) = size {
        d.unknown.insert(SYMBOL_SIZES.into(), json!({"Dot": size}));
    }
    let style = CharStyle::default();
    let mut text = TextObject::point(Point::new(10.0, 30.0), "", style.clone());
    text.runs = vec![TextRun::inline(InlineArt::new("Dot"), style)];
    assert!(d.resolve_text_inline(&mut text));
    let layer = d.layers[0].id;
    let id = d.alloc_id();
    d.insert(Some(layer), 0, Node::new(id, NodeKind::Text(Box::new(text)))).unwrap();
    if ordinary {
        let id = d.alloc_id();
        let xf = Affine::translate((60.0, 45.0)) * d.symbol_natural_xf("Dot");
        d.insert(Some(layer), 1, Node::new(id, NodeKind::SymbolInstance { symbol: "Dot".into(), xf })).unwrap();
    }
    let expected = match size {
        Some([w, h]) => Rect::new(-w / 2.0, -h / 2.0, w / 2.0, h / 2.0),
        None => Rect::new(-10.0, -10.0, 10.0, 10.0),
    };
    assert_eq!(d.symbol_art_bounds("Dot"), Some(expected), "positive control: source natural bounds");
    d
}

fn doc(s: &Session) -> &Document {
    &s.doc().unwrap().doc
}

fn parent(source: &Document, collision: bool) -> Session {
    let mut s = fixtures::session_with(120.0, 100.0);
    let d = Arc::make_mut(&mut s.doc_mut().unwrap().doc);
    d.unknown.insert(SYMBOL_SIZES.into(), json!({"Unrelated": [7.0, 9.0]}));
    if collision {
        let mut symbol = source.symbols[0].clone();
        Arc::make_mut(&mut symbol.art).appearance = look([230, 20, 20]);
        d.symbols.push(symbol);
        d.unknown.get_mut(SYMBOL_SIZES).unwrap()["Dot"] = json!([20.0, 20.0]);
    }
    s
}

fn assert_existing_resources(before: &Document, after: &Document) {
    for symbol in &before.symbols {
        assert_eq!(after.symbols.iter().find(|s| s.name == symbol.name), Some(symbol), "existing symbol art stays intact");
    }
    for (name, size) in before.unknown[SYMBOL_SIZES].as_object().unwrap() {
        assert_eq!(&after.unknown[SYMBOL_SIZES][name], size, "existing natural size for {name} stays intact");
    }
}

fn placed_id(result: &Value) -> NodeId {
    assert_eq!(result["ids"].as_array().unwrap().len(), 1, "{result}");
    NodeId(result["ids"][0].as_u64().unwrap())
}

fn place_copy(s: &mut Session, source: &Document) -> NodeId {
    let bytes = vectorcraft_format::save_file(source);
    assert!(!bytes.is_empty());
    let result = exec(s, "file.place", json!({"name": "inline.vectorcraft", "dataBase64": vectorcraft_format::base64_encode(&bytes)}));
    assert_eq!(result["linked"], false, "{result}");
    placed_id(&result)
}

/// Check the resource actually referenced by the editable text, rather than only finding an
/// imported definition. Cached inline bounds cannot hide missing or wrongly shared resources.
fn assert_adopted(d: &Document, id: NodeId, source: &Document, ordinary: bool) -> String {
    let root = d.node(id).unwrap();
    assert!(matches!(root.kind, NodeKind::Group { clip: true, .. }), "placed art is an editable clip group");
    assert!(!d.has_placed());
    let mut texts = vec![];
    let mut instances = vec![];
    root.walk(&mut |n| match &n.kind {
        NodeKind::Text(t) => texts.push((n.id, t.as_ref())),
        NodeKind::SymbolInstance { symbol, .. } => instances.push(symbol.as_str()),
        _ => {}
    });
    assert_eq!(texts.len(), 1, "positive control: the inline text survives placement");
    let (text_id, text) = texts[0];
    assert!(d.is_editable(text_id));
    let inlines: Vec<_> = text.runs.iter().filter_map(|r| r.inline.as_ref()).collect();
    assert_eq!(inlines.len(), 1);
    let name = &inlines[0].symbol;
    let symbol = d.symbols.iter().find(|s| s.name == *name).expect("the adopted inline reference must resolve");
    assert_eq!(symbol.art.as_ref(), source.symbols[0].art.as_ref(), "the inline references the original blue art");
    let expected = source.symbol_art_bounds("Dot").unwrap();
    assert_eq!(d.symbol_art_bounds(name), Some(expected), "natural bounds come from the source, including its aspect ratio");
    assert_eq!(d.symbol_natural_xf(name), source.symbol_natural_xf("Dot"));
    assert_eq!(d.unknown.get(SYMBOL_SIZES).and_then(|m| m.get(name)), source.unknown.get(SYMBOL_SIZES).and_then(|m| m.get("Dot")));
    let resolved = d.inline_resolved(text);
    assert_eq!(resolved.runs[0].inline.as_ref().unwrap().bounds, Some(expected), "the text consumer resolves source natural bounds");
    assert_eq!(instances.len(), usize::from(ordinary));
    if ordinary {
        assert_eq!(instances[0], name.as_str(), "ordinary instances and inline text use the same adopted definition");
    }
    name.clone()
}

fn assert_roundtrip(d: &Document, id: NodeId, source: &Document, ordinary: bool) {
    let reloaded = vectorcraft_format::load(&vectorcraft_format::save_file(d)).unwrap();
    assert_eq!(assert_adopted(&reloaded, id, source, ordinary), assert_adopted(d, id, source, ordinary));
}

#[test]
fn editable_place_adopts_inline_only_symbol_with_or_without_parent_collision() {
    let source = child(false, Some([40.0, 10.0]));
    let source_before = serde_json::to_value(&source).unwrap();
    for collision in [false, true] {
        let mut s = parent(&source, collision);
        let before = doc(&s).clone();
        let undo = s.doc().unwrap().history.undo.len();
        let id = place_copy(&mut s, &source);
        let name = assert_adopted(doc(&s), id, &source, false);
        assert_eq!(name != "Dot", collision);
        assert_eq!(doc(&s).symbols.len(), before.symbols.len() + 1);
        assert_existing_resources(&before, doc(&s));
        assert_roundtrip(doc(&s), id, &source, false);
        assert_eq!(s.doc().unwrap().history.undo.len(), undo + 1);
        exec(&mut s, "edit.undo", json!({}));
        assert_eq!(serde_json::to_value(doc(&s)).unwrap(), serde_json::to_value(&before).unwrap(), "undo removes art and adopted resources");
        exec(&mut s, "edit.redo", json!({}));
        assert_eq!(assert_adopted(doc(&s), id, &source, false), name);
        assert_existing_resources(&before, doc(&s));
    }
    assert_eq!(serde_json::to_value(&source).unwrap(), source_before);
}

/// Repository-local temporary files keep these file-placement regressions inside the checkout.
struct SourceFile {
    directory: PathBuf,
    path: PathBuf,
    bytes: Vec<u8>,
}

impl SourceFile {
    fn new(source: &Document) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.codex-inline-checks/fixtures").join(format!(
            "inline-place-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("inline.vectorcraft");
        let bytes = vectorcraft_format::save_file(source);
        assert!(!bytes.is_empty());
        std::fs::write(&path, &bytes).unwrap();
        Self { directory, path, bytes }
    }
}

impl Drop for SourceFile {
    fn drop(&mut self) {
        // Cleanup is best effort so an earlier assertion remains the reported failure.
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}

fn linked_conversion(command: &str) {
    let source = child(false, Some([40.0, 10.0]));
    let file = SourceFile::new(&source);
    for collision in [false, true] {
        let mut s = parent(&source, collision);
        let before = doc(&s).clone();
        let result = exec(&mut s, "file.place", json!({"path": file.path.to_str().unwrap()}));
        assert_eq!(result["linked"], true);
        let id = placed_id(&result);
        assert!(matches!(doc(&s).node(id).unwrap().kind, NodeKind::PlacedDocument(_)), "positive control: this conversion starts linked");
        assert_eq!(doc(&s).symbols, before.symbols, "placing linked keeps resources in the child");
        assert_eq!(doc(&s).unknown[SYMBOL_SIZES], before.unknown[SYMBOL_SIZES]);
        let linked = serde_json::to_value(doc(&s)).unwrap();
        let undo = s.doc().unwrap().history.undo.len();
        let params = if command == "links.embed" { json!({"ids": [id.0]}) } else { json!({}) };
        let result = exec(&mut s, command, params);
        if command == "links.embed" {
            assert_eq!(result["embedded"], json!([id.0]));
            assert_eq!(result["missing"], json!([]));
        } else {
            assert_eq!(result["ids"], json!([id.0]));
        }
        let name = assert_adopted(doc(&s), id, &source, false);
        assert_eq!(name != "Dot", collision);
        assert_eq!(doc(&s).symbols.len(), before.symbols.len() + 1);
        assert_existing_resources(&before, doc(&s));
        assert_roundtrip(doc(&s), id, &source, false);
        assert_eq!(exec(&mut s, "links.list", json!({}))["links"], json!([]));
        assert_eq!(s.doc().unwrap().history.undo.len(), undo + 1);
        let converted = serde_json::to_value(doc(&s)).unwrap();
        exec(&mut s, "edit.undo", json!({}));
        assert_eq!(serde_json::to_value(doc(&s)).unwrap(), linked, "undo restores the link and parent resources");
        exec(&mut s, "edit.redo", json!({}));
        assert_eq!(serde_json::to_value(doc(&s)).unwrap(), converted);
        assert_eq!(assert_adopted(doc(&s), id, &source, false), name);
    }
    assert_eq!(std::fs::read(&file.path).unwrap(), file.bytes, "conversion leaves the original file intact");
}

#[test]
fn linked_embed_adopts_inline_only_symbol_with_or_without_parent_collision() {
    linked_conversion("links.embed");
}

#[test]
fn expand_adopts_inline_only_symbol_with_or_without_parent_collision() {
    linked_conversion("object.expand");
}

#[test]
fn ordinary_instances_and_inline_text_share_the_adopted_symbol_and_natural_size() {
    let source = child(true, Some([40.0, 10.0]));
    for collision in [false, true] {
        let mut s = parent(&source, collision);
        let before = doc(&s).clone();
        let id = place_copy(&mut s, &source);
        assert_adopted(doc(&s), id, &source, true);
        assert_existing_resources(&before, doc(&s));
        assert_roundtrip(doc(&s), id, &source, true);
    }
}

#[test]
fn equal_normalized_definitions_with_different_natural_sizes_are_not_shared() {
    // Include both directions of the implicit 20x20 fallback when a document has no size entry.
    for (source_size, parent_size) in [(Some([40.0, 10.0]), Some([20.0, 20.0])), (Some([40.0, 10.0]), None), (None, Some([40.0, 10.0]))] {
        let source = child(false, source_size);
        let mut s = parent(&source, false);
        let d = Arc::make_mut(&mut s.doc_mut().unwrap().doc);
        d.symbols.push(source.symbols[0].clone());
        if let Some(size) = parent_size {
            d.unknown.get_mut(SYMBOL_SIZES).unwrap()["Dot"] = json!(size);
        }
        assert_eq!(d.symbols[0], source.symbols[0], "positive control: definitions are exactly equal");
        assert_ne!(d.symbol_natural_xf("Dot"), source.symbol_natural_xf("Dot"), "only natural sizes differ");
        let before = doc(&s).clone();
        let id = place_copy(&mut s, &source);
        let name = assert_adopted(doc(&s), id, &source, false);
        assert_ne!(name, "Dot", "a natural-size conflict must get a separate definition");
        assert_eq!(doc(&s).symbols.len(), 2);
        assert_existing_resources(&before, doc(&s));
        assert_roundtrip(doc(&s), id, &source, false);
    }
}

#[test]
fn equal_definitions_with_equal_natural_sizes_are_shared() {
    for size in [Some([40.0, 10.0]), None] {
        let source = child(false, size);
        let mut s = parent(&source, false);
        let d = Arc::make_mut(&mut s.doc_mut().unwrap().doc);
        d.symbols.push(source.symbols[0].clone());
        if let Some(size) = size {
            d.unknown.get_mut(SYMBOL_SIZES).unwrap()["Dot"] = json!(size);
        }
        let before = doc(&s).clone();
        let id = place_copy(&mut s, &source);
        assert_eq!(assert_adopted(doc(&s), id, &source, false), "Dot");
        assert_eq!(doc(&s).symbols.len(), 1, "equal definitions and sizes still deduplicate");
        assert_existing_resources(&before, doc(&s));
        assert_roundtrip(doc(&s), id, &source, false);
    }
}

#[test]
fn source_without_size_metadata_does_not_inherit_an_orphan_parent_size() {
    let source = child(false, None);
    let mut s = parent(&source, false);
    Arc::make_mut(&mut s.doc_mut().unwrap().doc).unknown.get_mut(SYMBOL_SIZES).unwrap()["Dot"] = json!([100.0, 5.0]);
    assert!(doc(&s).symbols.is_empty(), "positive control: the parent's Dot size has no definition");
    let id = place_copy(&mut s, &source);
    assert_eq!(assert_adopted(doc(&s), id, &source, false), "Dot");
    assert_eq!(doc(&s).unknown[SYMBOL_SIZES].get("Dot"), None);
    assert_eq!(doc(&s).unknown[SYMBOL_SIZES]["Unrelated"], json!([7.0, 9.0]));
    assert_roundtrip(doc(&s), id, &source, false);
}

fn two_inline_symbol_source(reversed: bool) -> Document {
    let mut source = child(false, Some([40.0, 10.0]));
    source.symbols.push(Symbol {
        name: "Dot 2".into(),
        art: Arc::new(Node::path(NodeId(901), shapes::rectangle(Rect::new(-10.0, -10.0, 10.0, 10.0)), look([20, 230, 20]))),
    });
    source.unknown.get_mut(SYMBOL_SIZES).unwrap()["Dot 2"] = json!([10.0, 40.0]);
    let text_id = source.layers[0].children().unwrap()[0].id;
    let NodeKind::Text(text) = &mut source.node_mut(text_id).unwrap().kind else { panic!("expected the fixture's point text") };
    let order = if reversed { ["Dot 2", "Dot"] } else { ["Dot", "Dot 2"] };
    text.runs = order.into_iter().map(|name| TextRun::inline(InlineArt::new(name), CharStyle::default())).collect();
    assert_eq!(source.resolve_inline_art(), vec![text_id]);
    assert_eq!(source.symbol_art_bounds("Dot"), Some(Rect::new(-20.0, -5.0, 20.0, 5.0)));
    assert_eq!(source.symbol_art_bounds("Dot 2"), Some(Rect::new(-5.0, -20.0, 5.0, 20.0)));
    source
}

fn assert_two_inline_symbols(d: &Document, id: NodeId, source: &Document, reversed: bool) -> Vec<String> {
    let root = d.node(id).unwrap();
    assert!(matches!(root.kind, NodeKind::Group { clip: true, .. }));
    assert!(!d.has_placed(), "the placed source must remain editable art");
    let mut texts = vec![];
    root.walk(&mut |n| {
        if let NodeKind::Text(t) = &n.kind {
            texts.push((n.id, t.as_ref()));
        }
    });
    assert_eq!(texts.len(), 1, "positive control: the text survives placement");
    let (text_id, text) = texts[0];
    assert!(d.is_editable(text_id));
    assert_eq!(text.runs.len(), 2, "both source inline runs must survive");
    let inlines: Vec<_> = text.runs.iter().map(|r| r.inline.as_ref().expect("each fixture run is an inline symbol")).collect();
    assert_ne!(inlines[0].symbol, inlines[1].symbol, "distinct source icons must not acquire the same destination reference");
    let mut names: Vec<_> = d.symbols.iter().map(|s| s.name.as_str()).collect();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), d.symbols.len(), "every destination symbol definition must have a unique name");

    let order = if reversed { ["Dot 2", "Dot"] } else { ["Dot", "Dot 2"] };
    let resolved = d.inline_resolved(text);
    for (index, (art, source_name)) in inlines.iter().zip(order).enumerate() {
        let original = source.symbols.iter().find(|s| s.name == source_name).unwrap();
        let adopted = d.symbols.iter().find(|s| s.name == art.symbol).expect("the inline reference must resolve");
        assert_eq!(adopted.art, original.art, "each inline must reference its own original coloured art");
        let expected_bounds = source.symbol_art_bounds(source_name).unwrap();
        assert_eq!(d.symbol_art_bounds(&art.symbol), Some(expected_bounds), "each icon keeps its natural aspect ratio");
        assert_eq!(d.symbol_natural_xf(&art.symbol), source.symbol_natural_xf(source_name));
        assert_eq!(d.unknown[SYMBOL_SIZES][&art.symbol], source.unknown[SYMBOL_SIZES][source_name]);
        assert_eq!(resolved.runs[index].inline.as_ref().unwrap().bounds, Some(expected_bounds), "the text consumer resolves the right icon's bounds");
    }
    inlines.iter().map(|art| art.symbol.clone()).collect()
}

fn two_inline_symbol_place_case(collision: bool, reversed: bool) {
    let source = two_inline_symbol_source(reversed);
    let source_before = serde_json::to_value(&source).unwrap();
    let mut s = parent(&source, collision);
    let before = doc(&s).clone();
    let selection_before = s.doc().unwrap().selection.clone();
    let undo = s.doc().unwrap().history.undo.len();
    let id = place_copy(&mut s, &source);
    assert_existing_resources(&before, doc(&s));
    assert_eq!(doc(&s).symbols.len(), before.symbols.len() + 2);
    let names = assert_two_inline_symbols(doc(&s), id, &source, reversed);
    let blue_index = usize::from(reversed);
    assert_eq!(names[blue_index] != "Dot", collision, "a conflicting parent Dot must not be reused");
    if !collision {
        let order = if reversed { ["Dot 2", "Dot"] } else { ["Dot", "Dot 2"] };
        assert_eq!(names, order.map(str::to_string));
    }
    let reloaded = vectorcraft_format::load(&vectorcraft_format::save_file(doc(&s))).unwrap();
    assert_eq!(assert_two_inline_symbols(&reloaded, id, &source, reversed), names);
    assert_existing_resources(&before, &reloaded);
    assert_eq!(s.doc().unwrap().history.undo.len(), undo + 1);
    let edited = serde_json::to_value(doc(&s)).unwrap();

    exec(&mut s, "edit.undo", json!({}));
    assert_eq!(serde_json::to_value(doc(&s)).unwrap(), serde_json::to_value(&before).unwrap(), "undo restores the destination and its resources");
    assert_eq!(s.doc().unwrap().selection, selection_before);
    assert_eq!(s.doc().unwrap().history.undo.len(), undo);
    assert_eq!(s.doc().unwrap().history.redo.len(), 1);
    exec(&mut s, "edit.redo", json!({}));
    assert_eq!(serde_json::to_value(doc(&s)).unwrap(), edited);
    assert_eq!(s.doc().unwrap().history.undo.len(), undo + 1);
    assert!(s.doc().unwrap().history.redo.is_empty());
    assert_eq!(assert_two_inline_symbols(doc(&s), id, &source, reversed), names);
    assert_existing_resources(&before, doc(&s));
    assert_eq!(serde_json::to_value(&source).unwrap(), source_before, "placement must not mutate the source");
}

#[test]
fn editable_place_reserves_two_inline_symbol_names_on_collision() {
    two_inline_symbol_place_case(true, false);
}

#[test]
fn editable_place_two_inline_symbols_without_collision_keep_art_and_sizes() {
    for reversed in [false, true] {
        two_inline_symbol_place_case(false, reversed);
    }
}

#[test]
fn editable_place_two_inline_symbols_reversed_order_keep_art_and_sizes() {
    two_inline_symbol_place_case(true, true);
}

fn nested_inline_source() -> Document {
    let mut d = child(false, Some([40.0, 10.0]));
    d.symbols.push(Symbol {
        name: "Badge".into(),
        art: Arc::new(Node::new(NodeId(901), NodeKind::SymbolInstance { symbol: "Dot".into(), xf: Affine::IDENTITY })),
    });
    d.unknown.get_mut(SYMBOL_SIZES).unwrap()["Badge"] = json!([40.0, 10.0]);
    let text_id = d.layers[0].children().unwrap()[0].id;
    let NodeKind::Text(t) = &mut d.node_mut(text_id).unwrap().kind else { panic!("fixture must contain text") };
    t.runs[0].inline.as_mut().unwrap().symbol = "Badge".into();
    d.resolve_inline_art();
    assert_eq!(d.symbol_art_bounds("Badge"), Some(Rect::new(-20.0, -5.0, 20.0, 5.0)));
    d
}

fn assert_nested_inline_source_art(d: &Document, id: NodeId, source: &Document) -> String {
    let mut refs = vec![];
    d.node(id).unwrap().walk(&mut |n| {
        if let NodeKind::Text(t) = &n.kind {
            refs.extend(t.runs.iter().filter_map(|r| r.inline.as_ref().map(|a| a.symbol.clone())));
        }
    });
    assert_eq!(refs.len(), 1, "the placed inline must survive as editable text");
    let badge = d.symbols.iter().find(|s| s.name == refs[0]).unwrap();
    let NodeKind::SymbolInstance { symbol: inner, xf } = &badge.art.kind else { panic!("Badge must retain its editable nested instance") };
    assert_eq!(*xf, Affine::IDENTITY);
    let dot = d.symbols.iter().find(|s| s.name == *inner).unwrap();
    assert_eq!(dot.art, source.symbols[0].art, "the reachable nested definition must be the source's blue art, not the parent's red art");
    assert_eq!(d.unknown[SYMBOL_SIZES].get(inner), source.unknown[SYMBOL_SIZES].get("Dot"));
    assert_eq!(d.symbol_art_bounds(&refs[0]), source.symbol_art_bounds("Badge"));
    refs[0].clone()
}

#[test]
fn editable_inline_equal_wrapper_keeps_its_source_dependency_on_conflict() {
    for conflict in [false, true] {
        let source = nested_inline_source();
        let original_source = serde_json::to_value(&source).unwrap();
        let mut s = parent(&source, conflict);
        let d = Arc::make_mut(&mut s.doc_mut().unwrap().doc);
        if !conflict {
            d.symbols.push(source.symbols[0].clone());
        }
        d.unknown.get_mut(SYMBOL_SIZES).unwrap()["Dot"] = source.unknown[SYMBOL_SIZES]["Dot"].clone();
        d.symbols.push(source.symbols[1].clone());
        d.unknown.get_mut(SYMBOL_SIZES).unwrap()["Badge"] = source.unknown[SYMBOL_SIZES]["Badge"].clone();
        assert_eq!(d.symbols[1], source.symbols[1], "the outer definitions are exactly equal");
        assert_eq!(d.symbol_natural_xf("Badge"), source.symbol_natural_xf("Badge"));
        assert_eq!(d.symbols[0] == source.symbols[0], !conflict, "only the referenced resource conflicts");
        let before = doc(&s).clone();
        let id = place_copy(&mut s, &source);
        let name = assert_nested_inline_source_art(doc(&s), id, &source);
        if conflict {
            assert_ne!(name, "Badge", "the parent's Badge must retain its own red dependency");
        } else {
            assert_eq!(name, "Badge");
            assert_eq!(doc(&s).symbols.len(), 2, "the equal-dependency control still shares both definitions");
        }
        assert_existing_resources(&before, doc(&s));
        let names: std::collections::BTreeSet<_> = doc(&s).symbols.iter().map(|s| &s.name).collect();
        assert_eq!(names.len(), doc(&s).symbols.len());
        let reloaded = vectorcraft_format::load(&vectorcraft_format::save_file(doc(&s))).unwrap();
        assert_eq!(assert_nested_inline_source_art(&reloaded, id, &source), name);
        let after = serde_json::to_value(doc(&s)).unwrap();
        s.execute("edit.undo", &json!({})).unwrap();
        assert_eq!(serde_json::to_value(doc(&s)).unwrap(), serde_json::to_value(&before).unwrap());
        s.execute("edit.redo", &json!({})).unwrap();
        assert_eq!(serde_json::to_value(doc(&s)).unwrap(), after);
        assert_eq!(assert_nested_inline_source_art(doc(&s), id, &source), name);
        assert_eq!(serde_json::to_value(&source).unwrap(), original_source);
    }
}

/// Find the resource the placed editable inline actually uses.
fn placed_inline_resource(d: &Document, id: NodeId) -> &str {
    let mut names = vec![];
    d.node(id).unwrap().walk(&mut |n| {
        if let NodeKind::Text(t) = &n.kind {
            names.extend(t.runs.iter().filter_map(|r| r.inline.as_ref().map(|a| a.symbol.as_str())));
        }
    });
    assert_eq!(names.len(), 1);
    names[0]
}

fn assert_resource_undo_roundtrip(s: &mut Session, before: &Document) {
    let after = serde_json::to_value(doc(s)).unwrap();
    let loaded = vectorcraft_format::load(&vectorcraft_format::save_file(doc(s))).unwrap();
    assert_eq!(serde_json::to_value(&loaded).unwrap(), after);
    assert_eq!(loaded.images, doc(s).images, "saved resource bytes and previews survive");
    exec(s, "edit.undo", json!({}));
    assert_eq!(serde_json::to_value(doc(s)).unwrap(), serde_json::to_value(before).unwrap());
    assert_eq!(doc(s).images, before.images);
    exec(s, "edit.redo", json!({}));
    assert_eq!(serde_json::to_value(doc(s)).unwrap(), after);
    assert_eq!(doc(s).images, loaded.images);
}

#[test]
fn shared_inline_ancestors_propagate_remaps_and_reserve_incoming_names() {
    for reverse in [false, true] {
        for conflict in [false, true] {
            let mut source = nested_inline_source();
            source.symbols.push(Symbol {
                name: "Outer".into(),
                art: Arc::new(Node::new(NodeId(902), NodeKind::SymbolInstance { symbol: "Badge".into(), xf: Affine::IDENTITY })),
            });
            source.unknown.get_mut(SYMBOL_SIZES).unwrap()["Outer"] = json!([40.0, 10.0]);
            let text_id = source.layers[0].children().unwrap()[0].id;
            let NodeKind::Text(t) = &mut source.node_mut(text_id).unwrap().kind else { panic!("inline fixture") };
            t.runs[0].inline.as_mut().unwrap().symbol = "Outer".into();
            // These incoming definitions reserve the obvious suffixes before shared wrappers
            // are reconsidered. They are also placed ordinary instances, not unused metadata.
            for (i, name) in ["Badge 2", "Outer 2"].into_iter().enumerate() {
                let mut reserved = source.symbols[0].clone();
                reserved.name = name.into();
                source.symbols.push(reserved);
                let layer = source.layers[0].id;
                let id = source.alloc_id();
                source.insert(Some(layer), i + 1, Node::new(id, NodeKind::SymbolInstance { symbol: name.into(), xf: Affine::IDENTITY })).unwrap();
            }
            source.resolve_inline_art();
            if reverse {
                source.symbols.reverse();
            }
            let source_before = serde_json::to_value(&source).unwrap();
            let mut s = fixtures::session_with(120.0, 100.0);
            let d = Arc::make_mut(&mut s.doc_mut().unwrap().doc);
            d.symbols = source.symbols.iter().filter(|s| !s.name.ends_with(" 2")).cloned().collect();
            d.unknown.insert(SYMBOL_SIZES.into(), source.unknown[SYMBOL_SIZES].clone());
            if conflict {
                Arc::make_mut(&mut d.symbols.iter_mut().find(|s| s.name == "Dot").unwrap().art).appearance = look([230, 20, 20]);
            }
            let before = doc(&s).clone();
            let id = place_copy(&mut s, &source);
            let outer_name = placed_inline_resource(doc(&s), id);
            let outer = doc(&s).symbols.iter().find(|s| s.name == outer_name).unwrap();
            let NodeKind::SymbolInstance { symbol: badge_name, .. } = &outer.art.kind else { panic!("Outer instance") };
            let badge = doc(&s).symbols.iter().find(|s| s.name == *badge_name).unwrap();
            let NodeKind::SymbolInstance { symbol: dot_name, .. } = &badge.art.kind else { panic!("Badge instance") };
            let dot = doc(&s).symbols.iter().find(|s| s.name == *dot_name).unwrap();
            assert_eq!(dot.art, source.symbols.iter().find(|s| s.name == "Dot").unwrap().art);
            assert_eq!(doc(&s).symbol_natural_xf(outer_name), source.symbol_natural_xf("Outer"));
            assert_eq!(doc(&s).symbol_natural_xf(badge_name), source.symbol_natural_xf("Badge"));
            assert_eq!(doc(&s).symbol_natural_xf(dot_name), source.symbol_natural_xf("Dot"));
            if conflict {
                assert_eq!(outer_name, "Outer 3");
                assert_eq!(badge_name, "Badge 3");
                assert_ne!(dot_name, "Dot");
            } else {
                assert_eq!((outer_name, badge_name.as_str(), dot_name.as_str()), ("Outer", "Badge", "Dot"));
            }
            assert_eq!(doc(&s).symbols.len(), before.symbols.len() + 2 + if conflict { 3 } else { 0 });
            for name in ["Badge 2", "Outer 2"] {
                assert_eq!(doc(&s).symbols.iter().find(|s| s.name == name), source.symbols.iter().find(|s| s.name == name));
            }
            assert_existing_resources(&before, doc(&s));
            let names: std::collections::BTreeSet<_> = doc(&s).symbols.iter().map(|s| &s.name).collect();
            assert_eq!(names.len(), doc(&s).symbols.len());
            assert_resource_undo_roundtrip(&mut s, &before);
            assert_eq!(serde_json::to_value(&source).unwrap(), source_before);
        }
    }
}

fn review_pixel(rgb: [u8; 3]) -> vectorcraft_doc::ImageBlob {
    let image = vectorcraft_render::Rendered { width: 1, height: 1, pixels: vec![rgb[0], rgb[1], rgb[2], 255] };
    vectorcraft_doc::ImageBlob { mime: "image/png".into(), bytes: Arc::new(image.to_png().unwrap()), proxy: None }
}

#[test]
fn shared_inline_pattern_preserves_equal_resources_and_remaps_each_conflict() {
    // Equal-resource reuse, then one changed reachable resource at a time: image, swatch, symbol.
    for conflict in [None, Some("image"), Some("swatch"), Some("symbol")] {
        let mut source = nested_inline_source();
        let blue = Color::rgb8(20, 20, 230);
        source.swatches.push(vectorcraft_color::Swatch { name: "Review blue".into(), paint: Paint::solid(blue), global: true, spot: false });
        source.images.insert("Review pixel".into(), review_pixel([20, 20, 230]));
        let image = Node::new(
            NodeId(910),
            NodeKind::Image(vectorcraft_doc::ImageObject {
                key: "Review pixel".into(),
                width: 1,
                height: 1,
                xf: Affine::scale(5.0),
                link: None,
                placement: Default::default(),
            }),
        );
        let swatch = Node::path(
            NodeId(911),
            shapes::rectangle(Rect::new(5.0, 0.0, 10.0, 5.0)),
            Appearance::basic(Paint::Solid { color: blue, swatch: Some("Review blue".into()), tint: 1.0 }, Paint::None, 0.0),
        );
        let symbol = Node::new(NodeId(912), NodeKind::SymbolInstance { symbol: "Dot".into(), xf: Affine::IDENTITY });
        let mut tile = vectorcraft_doc::PatternDef::new("Review tile", vec![Arc::new(image), Arc::new(swatch), Arc::new(symbol)]);
        tile.tile = Rect::new(-10.0, -10.0, 10.0, 10.0);
        tile.h_spacing = 2.0;
        source.patterns.push(tile);
        let pattern_xf = Affine::new([2.0, 0.0, 0.0, 3.0, 4.0, 5.0]);
        source.symbols[1].art = Arc::new(Node::path(
            NodeId(901),
            shapes::rectangle(Rect::new(-10.0, -10.0, 10.0, 10.0)),
            Appearance::basic(Paint::Pattern { pattern: "Review tile".into(), xf: pattern_xf }, Paint::None, 0.0),
        ));
        source.resolve_inline_art();
        let source_before = serde_json::to_value(&source).unwrap();
        let mut s = fixtures::session_with(120.0, 100.0);
        let d = Arc::make_mut(&mut s.doc_mut().unwrap().doc);
        d.symbols = source.symbols.clone();
        d.patterns = source.patterns.clone();
        d.images = source.images.clone();
        d.swatches.push(source.swatch("Review blue").unwrap().clone());
        d.unknown.insert(SYMBOL_SIZES.into(), source.unknown[SYMBOL_SIZES].clone());
        match conflict {
            Some("image") => {
                d.images.insert("Review pixel".into(), review_pixel([230, 20, 20]));
            }
            Some("swatch") => {
                d.swatch_mut("Review blue").unwrap().paint = Paint::solid(Color::rgb8(230, 20, 20));
            }
            Some("symbol") => {
                Arc::make_mut(&mut d.symbols[0].art).appearance = look([230, 20, 20]);
            }
            _ => {}
        }
        assert_eq!(d.symbols[1], source.symbols[1]);
        assert_eq!(d.pattern("Review tile"), source.pattern("Review tile"));
        let before = doc(&s).clone();
        let id = place_copy(&mut s, &source);
        let badge_name = placed_inline_resource(doc(&s), id);
        let badge = doc(&s).symbols.iter().find(|s| s.name == badge_name).unwrap();
        let Paint::Pattern { pattern, xf } = badge.art.appearance.fill_paint() else { panic!("editable pattern fill") };
        assert_eq!(xf, pattern_xf);
        let adopted = doc(&s).pattern(&pattern).unwrap();
        let mut expected = source.pattern("Review tile").unwrap().clone();
        expected.name = pattern.clone();
        let NodeKind::Image(image) = &adopted.art[0].kind else { panic!("pattern image") };
        assert_eq!(doc(&s).images.get(&image.key), source.images.get("Review pixel"));
        let NodeKind::Image(expected_image) = &mut Arc::make_mut(&mut expected.art[0]).kind else { panic!("source image") };
        expected_image.key = image.key.clone();
        if conflict == Some("swatch") {
            let paint = &mut Arc::make_mut(&mut expected.art[1]).appearance.fill_mut().unwrap().paint;
            let Paint::Solid { swatch, .. } = paint else { panic!("source swatch") };
            *swatch = None;
        }
        let NodeKind::SymbolInstance { symbol, .. } = &adopted.art[2].kind else { panic!("pattern symbol") };
        assert_eq!(doc(&s).symbols.iter().find(|s| s.name == *symbol).unwrap().art, source.symbols[0].art);
        let NodeKind::SymbolInstance { symbol: expected_symbol, .. } = &mut Arc::make_mut(&mut expected.art[2]).kind else { panic!("source symbol") };
        *expected_symbol = symbol.clone();
        assert_eq!(adopted, &expected, "tile properties and source art are preserved apart from necessary references");
        assert_eq!(doc(&s).symbol_natural_xf(badge_name), source.symbol_natural_xf("Badge"));
        assert_eq!(badge_name != "Badge", conflict.is_some());
        assert_eq!(pattern != "Review tile", conflict.is_some());
        assert_eq!(doc(&s).symbols.len(), before.symbols.len() + usize::from(conflict.is_some()) + usize::from(conflict == Some("symbol")));
        assert_eq!(doc(&s).patterns.len(), before.patterns.len() + usize::from(conflict.is_some()));
        assert_eq!(doc(&s).images.len(), before.images.len() + usize::from(conflict == Some("image")));
        assert_eq!(doc(&s).swatches, before.swatches, "a conflicting colour remains unlinked, without lookalike swatches");
        assert_eq!(doc(&s).pattern("Review tile"), before.pattern("Review tile"));
        assert_eq!(doc(&s).images.get("Review pixel"), before.images.get("Review pixel"));
        assert_existing_resources(&before, doc(&s));
        assert_resource_undo_roundtrip(&mut s, &before);
        assert_eq!(serde_json::to_value(&source).unwrap(), source_before);
    }
}
