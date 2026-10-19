//! Actual CLI conversion of a synthetic rig, including a native-format intermediate.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
use std::path::{Path, PathBuf};
use std::process::Command;
use vectorcraft_doc::Document;

const RIG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
  <g id="character" data-pivot="50,90">
    <rect id="shown" width="20" height="20" data-role="arm" fill="#0055ff"/>
    <g id="alternate" display="none" data-pivot="20,30">
      <rect id="hidden-hand" width="80" height="80" data-note="&quot;&amp;&#xA;猫"/>
      <rect id="hidden-fist" width="90" height="90" display="none" data-role="fist"/>
    </g>
  </g>
</svg>"##;

fn convert(input: &Path, output: &Path) {
    let result = Command::new(env!("CARGO_BIN_EXE_vectorcraft-cli")).arg("convert").arg(input).arg(output).output().unwrap();
    assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
}

fn check(doc: &Document) {
    let mut names = vec![];
    doc.walk(|n| {
        if let Some(name) = n.name.as_deref() {
            names.push(name);
            match name {
                "character" => assert_eq!(n.attrs.as_deref().unwrap().svg_data["data-pivot"], "50,90"),
                "alternate" => {
                    assert!(!n.visible);
                    assert_eq!(n.children().unwrap().len(), 2);
                    assert_eq!(n.attrs.as_deref().unwrap().svg_data["data-pivot"], "20,30");
                }
                "hidden-hand" => {
                    assert!(n.visible, "its parent owns the hidden state");
                    assert_eq!(n.attrs.as_deref().unwrap().svg_data["data-note"], "\"&\n猫");
                }
                "hidden-fist" => assert!(!n.visible),
                _ => {}
            }
        }
    });
    for name in ["character", "shown", "alternate", "hidden-hand", "hidden-fist"] {
        assert!(names.contains(&name), "missing {name}: {names:?}");
    }
}

#[test]
fn convert_svg_and_native_intermediate_preserve_hidden_rig() {
    let target =
        std::env::var_os("CARGO_TARGET_DIR").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target"));
    let folder = target.join(format!("svg-rig-cli-{}", std::process::id()));
    std::fs::create_dir_all(&folder).unwrap();
    let source = folder.join("rig.svg");
    let output = folder.join("converted.svg");
    let native = folder.join("rig.vectorcraft");
    let reopened = folder.join("reopened.svg");
    std::fs::write(&source, RIG).unwrap();
    convert(&source, &output);
    let text = std::fs::read_to_string(&output).unwrap();
    assert!(text.contains("display=\"none\""));
    check(&vectorcraft_svg::import(&text).unwrap());
    convert(&source, &native);
    check(&vectorcraft_format::load(&std::fs::read(&native).unwrap()).unwrap());
    convert(&native, &reopened);
    check(&vectorcraft_svg::import(&std::fs::read_to_string(&reopened).unwrap()).unwrap());
    std::fs::remove_dir_all(folder).unwrap();
}

#[test]
fn svg_conversion_flags_do_not_enable_hidden_psd_layers() {
    let target =
        std::env::var_os("CARGO_TARGET_DIR").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target"));
    let folder = target.join(format!("svg-psd-cli-{}", std::process::id()));
    std::fs::create_dir_all(&folder).unwrap();
    let source = folder.join("layers.svg");
    let native = folder.join("layers.vectorcraft");
    let output = folder.join("layers.psd");
    std::fs::write(
        &source,
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="60" height="40">
            <g id="shown-proof-layer"><rect width="20" height="20" fill="blue"/></g>
            <g id="hidden-proof-layer" display="none"><rect width="30" height="30" fill="red"/></g>
        </svg>"#,
    )
    .unwrap();
    convert(&source, &native);
    let doc = vectorcraft_format::load(&std::fs::read(&native).unwrap()).unwrap();
    assert_eq!(doc.layers.len(), 2);
    assert!(doc.layers.iter().any(|n| n.name.as_deref() == Some("hidden-proof-layer") && !n.visible));
    convert(&source, &output);
    let bytes = std::fs::read(&output).unwrap();
    assert_eq!(&bytes[..4], b"8BPS");
    let names = bytes
        .windows(8)
        .enumerate()
        .filter(|(_, w)| w == b"8BIMluni")
        .map(|(i, _)| {
            let count = u32::from_be_bytes(bytes[i + 12..i + 16].try_into().unwrap()) as usize;
            let units = (0..count).map(|k| u16::from_be_bytes(bytes[i + 16 + k * 2..i + 18 + k * 2].try_into().unwrap())).collect::<Vec<_>>();
            String::from_utf16(&units).unwrap()
        })
        .collect::<Vec<_>>();
    assert_eq!(names, ["shown-proof-layer"], "SVG preservation must leave PSD's hidden-layer default alone");
    std::fs::remove_dir_all(folder).unwrap();
}
