//! Custom SVG data attributes retained on objects. Limits apply before import copies strings
//! and while native files deserialize their maps, rather than trusting a map's size hint.

use std::collections::BTreeMap;

use serde::Deserializer;
use serde::de::{Error, MapAccess, Visitor};

pub const MAX_ATTRIBUTES: usize = 128;
pub const MAX_NAME_BYTES: usize = 256;
pub const MAX_VALUE_BYTES: usize = 64 * 1024;
pub const MAX_BYTES: usize = 128 * 1024;

/// A namespace-free XML attribute name starting with data-.
/// XML 1.0 NameChar (excluding colon): https://www.w3.org/TR/xml/#NT-NameChar.
pub fn valid_name(name: &str) -> bool {
    name.len() <= MAX_NAME_BYTES
        && name.strip_prefix("data-").is_some_and(|suffix| {
            !suffix.is_empty()
                && suffix.chars().all(|c| {
                    matches!(c,
                        'a'..='z' | 'A'..='Z' | '0'..='9' | '_' | '-' | '.' | '\u{B7}' |
                        '\u{C0}'..='\u{D6}' | '\u{D8}'..='\u{F6}' | '\u{F8}'..='\u{2FF}' |
                        '\u{300}'..='\u{37D}' | '\u{37F}'..='\u{1FFF}' | '\u{200C}'..='\u{200D}' |
                        '\u{203F}'..='\u{2040}' | '\u{2070}'..='\u{218F}' | '\u{2C00}'..='\u{2FEF}' |
                        '\u{3001}'..='\u{D7FF}' | '\u{F900}'..='\u{FDCF}' | '\u{FDF0}'..='\u{FFFD}' |
                        '\u{10000}'..='\u{EFFFF}'
                    )
                })
        })
}

/// XML 1.0 character data, bounded before being retained or emitted.
pub fn valid_value(value: &str) -> bool {
    value.len() <= MAX_VALUE_BYTES
        && value.chars().all(|c| {
            matches!(c,
                '\t' | '\n' | '\r' | '\u{20}'..='\u{D7FF}' | '\u{E000}'..='\u{FFFD}' | '\u{10000}'..='\u{10FFFF}'
            )
        })
}

pub(crate) fn deserialize<'de, D: Deserializer<'de>>(de: D) -> Result<BTreeMap<String, String>, D::Error> {
    struct Data;
    impl<'de> Visitor<'de> for Data {
        type Value = BTreeMap<String, String>;
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("a bounded map of SVG data-* attributes")
        }
        fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<Self::Value, M::Error> {
            let mut out = BTreeMap::new();
            let mut bytes = 0;
            while let Some(name) = map.next_key::<String>()? {
                if out.len() >= MAX_ATTRIBUTES || !valid_name(&name) {
                    return Err(M::Error::custom("invalid or excessive SVG data attributes"));
                }
                let value = map.next_value::<String>()?;
                bytes += name.len() + value.len();
                if !valid_value(&value) || bytes > MAX_BYTES {
                    return Err(M::Error::custom("SVG data attributes exceed the size limit"));
                }
                out.insert(name, value);
            }
            Ok(out)
        }
    }
    de.deserialize_map(Data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::ObjectAttributes;
    use serde_json::json;

    #[test]
    fn old_attributes_default_and_data_maps_validate_names_and_sizes() {
        let old: ObjectAttributes = serde_json::from_value(json!({"note": "old"})).unwrap();
        assert!(old.svg_data.is_empty());
        for name in ["data-pivot", "data-1", "data-猫", "data-a.b_c"] {
            assert!(valid_name(name), "{name}");
        }
        for name in ["onclick", "data-", "data-a:b", "data-a\" onclick", "data-<bad>"] {
            assert!(!valid_name(name), "{name}");
            assert!(serde_json::from_value::<ObjectAttributes>(json!({"svgData": {name: "value"}})).is_err());
        }
        assert!(serde_json::from_value::<ObjectAttributes>(json!({"svgData": {"data-bad": "\u{0}"}})).is_err());
        let large = "x".repeat(MAX_VALUE_BYTES + 1);
        assert!(serde_json::from_value::<ObjectAttributes>(json!({"svgData": {"data-pivot": large}})).is_err());
        let many = (0..=MAX_ATTRIBUTES).map(|i| (format!("data-{i}"), "x")).collect::<BTreeMap<_, _>>();
        assert!(serde_json::from_value::<ObjectAttributes>(json!({"svgData": many})).is_err());
        let full = "x".repeat(MAX_VALUE_BYTES);
        assert!(serde_json::from_value::<ObjectAttributes>(json!({"svgData": {"data-a": full, "data-b": full}})).is_err());
        let attrs: ObjectAttributes = serde_json::from_value(json!({"svgData": {"data-pivot": "1,2"}})).unwrap();
        assert_eq!(serde_json::from_value::<ObjectAttributes>(serde_json::to_value(&attrs).unwrap()).unwrap(), attrs);
    }
}
