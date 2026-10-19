//! Synthetic animation-rig round trips, written from the public SVG/XML specifications.
//! https://www.w3.org/TR/SVG11/struct.html https://www.w3.org/TR/xml/
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use usvg::roxmltree;
use vectorcraft_doc::{Document, Node, svg_data};
use vectorcraft_svg::{ExportOptions, ObjectIds, Styling, export, import, import_with_report};
use vectorcraft_testkit::raster::{Image, assert_similar, render_artboard};

fn svg(body: &str) -> String {
    format!(r#"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="200">{body}</svg>"#)
}

fn named<'a>(doc: &'a Document, name: &str) -> &'a Node {
    let mut found = None;
    doc.walk(|n| {
        if n.name.as_deref() == Some(name) {
            found = Some(n);
        }
    });
    found.unwrap_or_else(|| panic!("missing {name}"))
}

fn data<'a>(n: &'a Node, key: &str) -> &'a str {
    n.attrs.as_deref().unwrap().svg_data.get(key).unwrap()
}

fn rendered(svg: &str) -> Image {
    let tree = resvg::usvg::Tree::from_str(svg, &resvg::usvg::Options::default()).unwrap();
    let mut pm = resvg::tiny_skia::Pixmap::new(200, 200).unwrap();
    pm.fill(resvg::tiny_skia::Color::WHITE);
    resvg::render(&tree, resvg::tiny_skia::Transform::identity(), &mut pm.as_mut());
    Image { width: 200, height: 200, rgba: pm.data().to_vec() }
}

#[test]
fn hidden_rig_roundtrip_keeps_children_data_and_appearance() {
    let source = svg(r##"
        <g id="character" data-pivot="100,180">
          <g id="arm" data-pivot="60,100"><path d="M40 100L60 90L80 110Z" fill="#b5835a"/></g>
          <g id="arm-raised" data-pivot="60,100" display="none">
            <path id="alternate" data-role="hand" d="M40 100L50 60L70 70Z" fill="#b5835a"/>
            <g id="nested" display="none"><rect width="20" height="20"/></g>
          </g>
          <g id="eye-closed" display="none"><path d="M90 80Q100 90 110 80" fill="none" stroke="black" stroke-width="3"/></g>
        </g>
    "##);
    let (doc, warnings) = import_with_report(&source).unwrap();
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(data(named(&doc, "character"), "data-pivot"), "100,180");
    assert!(!named(&doc, "arm-raised").visible);
    assert!(named(&doc, "alternate").visible, "visibility is owned by the parent");
    // The canvas and resvg differ slightly along antialiased edges.
    assert_similar(&render_artboard(&doc), &rendered(&source), 24.0, 0.001);
    for styling in [Styling::PresentationAttributes, Styling::InlineStyle, Styling::InternalCss, Styling::StyleEntities] {
        let options = ExportOptions { hidden_layers: true, hidden_objects: true, styling, ..Default::default() };
        let output = export(&doc, &options);
        let xml = roxmltree::Document::parse_with_options(&output, roxmltree::ParsingOptions { allow_dtd: true, ..Default::default() }).unwrap();
        for id in ["character", "arm", "arm-raised", "eye-closed", "alternate", "nested"] {
            assert!(xml.descendants().any(|n| n.attribute("id") == Some(id)), "{id}: {output}");
        }
        assert_similar(&rendered(&source), &rendered(&output), 2.0, 0.001);
        let back = import(&output).unwrap();
        for id in ["arm-raised", "eye-closed", "nested"] {
            assert!(!named(&back, id).visible, "{id}");
        }
        assert_eq!(data(named(&back, "arm-raised"), "data-pivot"), "60,100");
        assert_eq!(data(named(&back, "alternate"), "data-role"), "hand");
    }
    let normal = export(&doc, &ExportOptions::default());
    assert!(!normal.contains("arm-raised") && !normal.contains("eye-closed"));
}

#[test]
fn unnamed_groups_shapes_and_xml_attribute_whitespace_survive() {
    let source = svg(r#"<g data-pivot="1,2"><rect width="15" height="25" data-note="&quot;&amp;&lt;&gt;&apos;&#x9;&#xA;&#xD;猫"/></g>"#);
    let doc = import(&source).unwrap();
    let output = export(&doc, &ExportOptions { object_ids: ObjectIds::Minimal, ..Default::default() });
    let xml = roxmltree::Document::parse(&output).unwrap();
    let group = xml.descendants().find(|n| n.attribute("data-pivot") == Some("1,2")).unwrap();
    assert_eq!(group.tag_name().name(), "g");
    let shape = group.descendants().find(|n| n.attribute("data-note").is_some()).unwrap();
    assert_eq!(shape.attribute("data-note"), Some("\"&<>'\t\n\r猫"));
    let back = import(&output).unwrap();
    assert_eq!(render_artboard(&doc).rgba, render_artboard(&back).rgba);
    let mut attributes = vec![];
    back.walk(|n| {
        if let Some(attrs) = n.attrs.as_deref() {
            attributes.extend(attrs.svg_data.keys().cloned());
        }
    });
    assert!(attributes.contains(&"data-pivot".into()) && attributes.contains(&"data-note".into()));
}

#[test]
fn important_hidden_groups_and_standalone_shapes_stay_hidden() {
    let source = svg(r#"<style>.off { display:none !important }</style>
        <g class="off" data-role="alternate"><rect width="30" height="30"/></g>
        <rect id="hidden-shape" data-pivot="5,6" style="display:none!important" width="50" height="50"/>
        <text id="hidden-text" data-role="label" display="none" x="10" y="80">hidden</text>"#);
    let doc = import(&source).unwrap();
    assert!(!named(&doc, "hidden-shape").visible);
    assert!(!named(&doc, "hidden-text").visible);
    let output = export(&doc, &ExportOptions { hidden_objects: true, ..Default::default() });
    let back = import(&output).unwrap();
    assert!(!named(&back, "hidden-shape").visible);
    assert!(!named(&back, "hidden-text").visible);
    assert_eq!(data(named(&back, "hidden-shape"), "data-pivot"), "5,6");
    assert_eq!(render_artboard(&doc).rgba, rendered(&output).rgba);
    let mut groups = 0;
    back.walk(|n| {
        if n.attrs.as_deref().is_some_and(|a| a.svg_data.get("data-role").is_some_and(|v| v == "alternate")) {
            assert!(!n.visible);
            assert_eq!(n.children().unwrap().len(), 1);
            groups += 1;
        }
    });
    assert_eq!(groups, 1);
}

#[test]
fn excessive_attribute_data_is_rejected_before_retaining_it() {
    let attrs = (0..=svg_data::MAX_ATTRIBUTES).map(|i| format!(" data-a{i}=\"x\"")).collect::<String>();
    assert!(import(&svg(&format!("<rect width=\"10\" height=\"10\"{attrs}/>"))).is_err());
    let value = "x".repeat(svg_data::MAX_VALUE_BYTES + 1);
    assert!(import(&svg(&format!("<rect width=\"10\" height=\"10\" data-big=\"{value}\"/>"))).is_err());
    let value = "x".repeat(svg_data::MAX_VALUE_BYTES);
    let body = format!("<rect width=\"10\" height=\"10\" data-a=\"{value}\" data-b=\"{value}\"/>");
    assert!(import(&svg(&body)).is_err());
    // Individually valid objects must not bypass the document-wide retained-data budget.
    let body = format!("<rect width=\"10\" height=\"10\" data-a=\"{value}\"/>").repeat(129);
    assert!(import(&svg(&body)).is_err());
}

#[test]
fn hidden_knockout_children_do_not_enter_visible_knockout_masks() {
    let source = svg(r##"<g id="rig">
        <rect id="shown" width="40" height="40" fill="#0055ff"/>
        <g id="alternate" data-pivot="10,10" display="none"><rect width="80" height="80" fill="red"/></g>
    </g>"##);
    let mut doc = import(&source).unwrap();
    let id = named(&doc, "rig").id;
    doc.node_mut(id).unwrap().knockout = vectorcraft_doc::Knockout::On;
    let output = export(&doc, &ExportOptions { hidden_objects: true, hidden_layers: true, ..Default::default() });
    let back = import(&output).unwrap();
    assert!(!named(&back, "alternate").visible);
    assert_eq!(data(named(&back, "alternate"), "data-pivot"), "10,10");
    assert_similar(&rendered(&source), &rendered(&output), 2.0, 0.001);
}

#[test]
fn names_and_annotations_survive_without_duplicate_data_name_attributes() {
    let source = svg(r#"<g id="rig" data-name="Friendly&#xA;rig" data-pivot="1,2"><rect width="30" height="30"/></g>"#);
    let mut doc = import(&source).unwrap();
    let id = named(&doc, "Friendly\nrig").id;
    doc.node_mut(id).unwrap().name = Some("Renamed rig".into());
    for object_ids in [ObjectIds::LayerNames, ObjectIds::Unique, ObjectIds::Minimal] {
        let output = export(&doc, &ExportOptions { object_ids, ..Default::default() });
        let xml = roxmltree::Document::parse(&output).unwrap();
        let n = xml.descendants().find(|n| n.attribute("data-pivot").is_some()).unwrap();
        assert_eq!(n.attribute("data-name"), Some("Renamed rig"));
        let back = import(&output).unwrap();
        let mut found = false;
        back.walk(|n| found |= n.attrs.as_deref().is_some_and(|a| a.svg_data.get("data-pivot").is_some_and(|v| v == "1,2")));
        assert!(found);
    }
}

#[test]
fn internal_metadata_ids_do_not_change_visible_css() {
    let source = svg(r#"<style>rect { fill:blue } [id] { fill:red }</style>
        <rect width="40" height="40" data-pivot="10,20"/>"#);
    let doc = import(&source).unwrap();
    assert_similar(&render_artboard(&doc), &rendered(&source), 2.0, 0.001);
}

#[test]
fn source_ids_that_resemble_internal_ids_keep_their_names_and_data() {
    let source = svg(r#"<g id="vectorcraft-id-0" data-pivot="original"><rect width="10" height="10"/></g>
        <g data-pivot="anonymous"><rect x="20" width="10" height="10"/></g>"#);
    let doc = import(&source).unwrap();
    assert_eq!(data(named(&doc, "vectorcraft-id-0"), "data-pivot"), "original");
    let output = export(&doc, &ExportOptions::default());
    let back = import(&output).unwrap();
    assert_eq!(data(named(&back, "vectorcraft-id-0"), "data-pivot"), "original");
    let mut anonymous = 0;
    back.walk(|n| {
        if n.attrs.as_deref().is_some_and(|a| a.svg_data.get("data-pivot").is_some_and(|v| v == "anonymous")) {
            anonymous += 1;
        }
    });
    assert_eq!(anonymous, 1);
}

#[test]
fn clipped_groups_and_hidden_use_nodes_retain_their_own_data() {
    let source = svg(r##"<defs>
        <clipPath id="clip"><rect width="35" height="35"/></clipPath>
        <rect id="template" width="50" height="50" fill="blue"/>
        <symbol id="symbol"><rect width="10" height="10" fill="green"/></symbol>
      </defs>
      <g id="clipped" clip-path="url(#clip)" data-pivot="1,2">
        <use id="shown-use" href="#template" data-pivot="3,4"/>
        <use id="hidden-use" href="#template" display="none" data-pivot="5,6"/>
        <use id="hidden-symbol" href="#symbol" x="50" display="none" data-pivot="7,8"/>
      </g>"##);
    let doc = import(&source).unwrap();
    assert_eq!(data(named(&doc, "clipped"), "data-pivot"), "1,2");
    for (name, pivot, visible) in [("shown-use", "3,4", true), ("hidden-use", "5,6", false), ("hidden-symbol", "7,8", false)] {
        assert_eq!(data(named(&doc, name), "data-pivot"), pivot);
        assert_eq!(named(&doc, name).visible, visible);
    }
    let output = export(&doc, &ExportOptions { hidden_objects: true, hidden_layers: true, ..Default::default() });
    assert_similar(&rendered(&source), &rendered(&output), 2.0, 0.001);
    let back = import(&output).unwrap();
    for (name, pivot, visible) in
        [("clipped", "1,2", true), ("shown-use", "3,4", true), ("hidden-use", "5,6", false), ("hidden-symbol", "7,8", false)]
    {
        assert_eq!(data(named(&back, name), "data-pivot"), pivot);
        assert_eq!(named(&back, name).visible, visible);
    }
}

#[test]
fn generated_ids_do_not_change_descendant_or_structural_css_matches() {
    let source = svg(r##"<style>
        rect { fill:blue } [id] rect { fill:red } #vectorcraft-id-0 { fill:red }
        rect + rect { fill:green } rect:first-child { stroke:purple; stroke-width:2 }
      </style>
      <g data-pivot="1,2"><rect width="30" height="30" data-role="first"/>
        <rect x="40" width="30" height="30" data-role="second"/></g>"##);
    let doc = import(&source).unwrap();
    let output = export(&doc, &ExportOptions::default());
    assert_similar(&rendered(&source), &rendered(&output), 2.0, 0.001);
}

#[test]
fn invalid_custom_attributes_cannot_produce_malformed_xml() {
    let mut doc = import(&svg(r#"<rect id="shape" width="20" height="20" data-ok="safe"/>"#)).unwrap();
    let id = named(&doc, "shape").id;
    let n = doc.node_mut(id).unwrap();
    n.edit_attrs(|a| {
        a.svg_data.insert("data-\" invalid".into(), "bad".into());
        a.svg_data.insert("onclick".into(), "bad".into());
        a.svg_data.insert("data-null".into(), "\0".into());
        a.svg_data.insert("data-newline".into(), "\t\n\r\"&<>".into());
    });
    let output = export(&doc, &ExportOptions::default());
    let xml = roxmltree::Document::parse(&output).unwrap();
    let shape = xml.descendants().find(|n| n.attribute("data-ok").is_some()).unwrap();
    assert_eq!(shape.attribute("data-ok"), Some("safe"));
    assert_eq!(shape.attribute("data-newline"), Some("\t\n\r\"&<>"));
    assert!(shape.attribute("onclick").is_none() && shape.attribute("data-null").is_none());
    for attr in [r#"data-="bad""#, r#"data-bad="&#x0;""#, r#"data-ok="a" data-ok="b""#] {
        assert!(import(&svg(&format!("<rect width=\"10\" height=\"10\" {attr}/>"))).is_err());
    }
}

#[test]
fn clipping_path_object_data_survives_export_and_reimport() {
    let source = svg(r##"<defs><clipPath id="clip"><rect id="clip-shape" data-pivot="10,20" width="35" height="35"/></clipPath></defs>
        <g id="clipped" clip-path="url(#clip)"><rect width="50" height="50" fill="blue"/></g>"##);
    let doc = import(&source).unwrap();
    let clip = doc.layers[0].children().unwrap().first().unwrap();
    assert_eq!(data(clip, "data-pivot"), "10,20");
    let output = export(&doc, &ExportOptions::default());
    let xml = roxmltree::Document::parse(&output).unwrap();
    assert!(xml.descendants().any(|n| n.attribute("id") == Some("clip-shape")));
    let ids: Vec<_> = xml.descendants().filter_map(|n| n.attribute("id")).collect();
    assert_eq!(ids.len(), ids.iter().collect::<std::collections::HashSet<_>>().len());
    let back = import(&output).unwrap();
    assert_eq!(data(back.layers[0].children().unwrap().first().unwrap(), "data-pivot"), "10,20");
    assert_similar(&rendered(&source), &rendered(&output), 2.0, 0.001);
}

#[test]
fn hidden_layer_and_object_switches_gate_art_independently() {
    let source = svg(r#"<g id="off-layer" display="none" data-role="layer">
        <rect id="shown-in-layer" width="20" height="20"/>
        <g id="off-object" display="none"><rect id="child" width="50" height="50"/></g>
      </g>"#);
    let doc = import(&source).unwrap();
    for hidden_layers in [false, true] {
        for hidden_objects in [false, true] {
            let output = export(&doc, &ExportOptions { hidden_layers, hidden_objects, ..Default::default() });
            assert_eq!(output.contains("id=\"off-layer\""), hidden_layers);
            assert_eq!(output.contains("id=\"shown-in-layer\""), hidden_layers);
            assert_eq!(output.contains("id=\"off-object\""), hidden_layers && hidden_objects);
            assert_eq!(output.contains("id=\"child\""), hidden_layers && hidden_objects);
            assert_similar(&rendered(&source), &rendered(&output), 2.0, 0.001);
        }
    }
}

#[test]
fn unnamed_labels_and_cleared_names_do_not_restore_stale_data_name() {
    let mut doc = import(&svg(r#"<rect width="20" height="20" data-name="Friendly" data-pivot="1,2"/>"#)).unwrap();
    let id = named(&doc, "Friendly").id;
    doc.node_mut(id).unwrap().name = None;
    let output = export(&doc, &ExportOptions::default());
    let xml = roxmltree::Document::parse(&output).unwrap();
    let shape = xml.descendants().find(|n| n.attribute("data-pivot").is_some()).unwrap();
    assert!(shape.attribute("data-name").is_none());
    let back = import(&output).unwrap();
    back.walk(|n| {
        if n.attrs.as_deref().is_some_and(|a| a.svg_data.contains_key("data-pivot")) {
            assert!(n.name.is_none());
        }
    });
    doc.node_mut(id).unwrap().name = Some("Invalid\0label".into());
    roxmltree::Document::parse(&export(&doc, &ExportOptions::default())).unwrap();
}

#[test]
fn shared_clip_data_copies_cannot_bypass_the_retained_document_budget() {
    let value = "x".repeat(svg_data::MAX_VALUE_BYTES);
    let definition = format!("<defs><clipPath id=\"shared\" data-large=\"{value}\"><rect width=\"10\" height=\"10\"/></clipPath></defs>");
    let instances = r#"<g clip-path="url(#shared)"><rect width="20" height="20"/></g>"#.repeat(129);
    assert!(import(&svg(&format!("{definition}{instances}"))).is_err());
}

#[test]
fn clipping_definition_and_members_keep_distinct_data_and_fill_rules() {
    for members in [
        r#"<rect id="clip-a" data-first="first" width="40" height="40"/>"#,
        r#"<rect id="clip-a" data-first="first" width="40" height="40"/>
            <path id="clip-b" data-second="second" clip-rule="evenodd" d="M20 20H80V80H20Z M30 30H60V60H30Z"/>"#,
    ] {
        let source = svg(&format!(
            r#"<defs><clipPath id="clip" data-clip="parent">{members}</clipPath></defs>
            <g id="rig" clip-path="url(#clip)"><rect width="100" height="100" fill="blue"/></g>"#
        ));
        let doc = import(&source).unwrap();
        assert_eq!(data(named(&doc, "clip"), "data-clip"), "parent");
        assert_eq!(data(named(&doc, "clip-a"), "data-first"), "first");
        let output = export(&doc, &ExportOptions::default());
        let back = import(&output).unwrap();
        assert_eq!(data(named(&back, "clip"), "data-clip"), "parent");
        assert_eq!(data(named(&back, "clip-a"), "data-first"), "first");
        if members.contains("clip-b") {
            assert_eq!(data(named(&back, "clip-b"), "data-second"), "second");
        }
        assert_similar(&render_artboard(&doc), &rendered(&source), 24.0, 0.001);
        assert_similar(&rendered(&output), &rendered(&source), 2.0, 0.001);
    }
}

#[test]
fn annotated_clipping_definition_survives_unannotated_named_members() {
    for members in [
        r#"<rect id="member-a" width="40" height="40"/>"#,
        r#"<rect id="member-a" width="40" height="40"/>
            <path id="member-b" clip-rule="evenodd" d="M20 20H80V80H20Z M30 30H60V60H30Z"/>"#,
    ] {
        let source = svg(&format!(
            r#"<defs><clipPath id="clip" data-clip="parent">{members}</clipPath></defs>
                <g id="rig" clip-path="url(#clip)"><rect width="100" height="100" fill="blue"/></g>"#
        ));
        let doc = import(&source).unwrap();
        assert_eq!(data(named(&doc, "clip"), "data-clip"), "parent");
        named(&doc, "member-a");
        let output = export(&doc, &ExportOptions::default());
        roxmltree::Document::parse(&output).unwrap();
        let back = import(&output).unwrap();
        assert_eq!(data(named(&back, "clip"), "data-clip"), "parent");
        named(&back, "member-a");
        if members.contains("member-b") {
            named(&back, "member-b");
        }
        assert_similar(&render_artboard(&doc), &rendered(&source), 24.0, 0.001);
        assert_similar(&rendered(&source), &rendered(&output), 2.0, 0.001);
    }
}

#[test]
fn structural_css_hidden_objects_keep_their_children_and_data() {
    let source = svg(r#"<style>#shown + g {display:none !important} #rig > rect:first-child {display:none}</style>
        <g id="rig">
          <rect id="first-hidden" width="80" height="80" data-role="first"/>
          <g id="shown"><rect width="20" height="20" fill="blue"/></g>
          <g id="alternate" style="display:block" data-pivot="10,20"><rect id="child" width="60" height="60" fill="red"/></g>
        </g>"#);
    let doc = import(&source).unwrap();
    assert!(!named(&doc, "first-hidden").visible);
    assert!(!named(&doc, "alternate").visible);
    assert!(named(&doc, "child").visible);
    assert_eq!(data(named(&doc, "alternate"), "data-pivot"), "10,20");
    let output = export(&doc, &ExportOptions { hidden_objects: true, ..Default::default() });
    let back = import(&output).unwrap();
    assert!(!named(&back, "first-hidden").visible);
    assert!(!named(&back, "alternate").visible);
    assert!(named(&back, "child").visible);
    assert_eq!(data(named(&back, "alternate"), "data-pivot"), "10,20");
    assert_similar(&rendered(&source), &rendered(&output), 2.0, 0.001);
}

#[test]
fn an_empty_source_id_does_not_drop_hidden_group_data() {
    let source = svg(r#"<g id="" display="none" data-role="alternate"><rect width="60" height="60"/></g>"#);
    let doc = import(&source).unwrap();
    let output = export(&doc, &ExportOptions { hidden_objects: true, ..Default::default() });
    let xml = roxmltree::Document::parse(&output).unwrap();
    let n = xml.descendants().find(|n| n.attribute("data-role") == Some("alternate")).unwrap();
    assert_eq!(n.tag_name().name(), "g");
    let back = import(&output).unwrap();
    let mut count = 0;
    back.walk(|n| {
        if n.attrs.as_deref().is_some_and(|a| a.svg_data.get("data-role").is_some_and(|v| v == "alternate")) {
            count += 1;
            assert!(!n.visible);
            assert_eq!(n.children().unwrap().len(), 1);
        }
    });
    assert_eq!(count, 1);
    assert_similar(&rendered(&source), &rendered(&output), 2.0, 0.001);
}

#[test]
fn referenced_template_data_loss_is_reported_without_mixing_instance_data() {
    for definition in [
        r#"<g id="template" data-template="root"><rect data-member="child" width="20" height="20"/></g>"#,
        r#"<symbol id="template" data-template="root"><rect data-member="child" width="20" height="20"/></symbol>"#,
    ] {
        let source = svg(&format!(
            r##"<defs>{definition}</defs>
                <use id="shown" href="#template" data-pivot="1,2" fill="blue"/>
                <use id="alternate" href="#template" data-pivot="3,4" x="40" display="none" fill="red"/>"##
        ));
        let (doc, warnings) = import_with_report(&source).unwrap();
        assert!(warnings.iter().any(|w| w.contains("referenced template contents")), "{warnings:?}");
        assert_eq!(data(named(&doc, "shown"), "data-pivot"), "1,2");
        assert_eq!(data(named(&doc, "alternate"), "data-pivot"), "3,4");
        assert!(!named(&doc, "alternate").visible);
        let output = export(&doc, &ExportOptions { hidden_objects: true, hidden_layers: true, ..Default::default() });
        let back = import(&output).unwrap();
        assert_eq!(data(named(&back, "shown"), "data-pivot"), "1,2");
        assert_eq!(data(named(&back, "alternate"), "data-pivot"), "3,4");
        assert!(!named(&back, "alternate").visible);
        assert_similar(&rendered(&source), &rendered(&output), 2.0, 0.001);
    }
}

#[test]
fn annotated_clip_with_one_anonymous_member_stays_one_clipping_object() {
    let source = svg(r#"<defs><clipPath id="clip" data-pivot="10,20"><rect width="35" height="35"/></clipPath></defs>
        <g id="rig" clip-path="url(#clip)"><rect width="50" height="50" fill="blue"/></g>"#);
    let doc = import(&source).unwrap();
    assert!(named(&doc, "clip").path_data().is_some());
    assert_eq!(data(named(&doc, "clip"), "data-pivot"), "10,20");
    let output = export(&doc, &ExportOptions::default());
    let back = import(&output).unwrap();
    assert!(named(&back, "clip").path_data().is_some());
    assert_eq!(data(named(&back, "clip"), "data-pivot"), "10,20");
    assert_similar(&rendered(&source), &rendered(&output), 2.0, 0.001);
}
