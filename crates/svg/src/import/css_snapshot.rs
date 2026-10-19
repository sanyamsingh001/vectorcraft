//! Keep selector matches stable while the import prepass adds ids and shows hidden art.
//! Uses the same selector parser as usvg; declarations stay in cascade order, including
//! shorthand properties and !important, so usvg still resolves their values and inheritance.

use std::collections::HashSet;
use std::fmt::Write as _;

use super::Edits;
use super::css::XNode;
use crate::{SvgError, xml_escape};

struct Element<'a, 'i>(XNode<'a, 'i>);

impl simplecss::Element for Element<'_, '_> {
    fn parent_element(&self) -> Option<Self> {
        self.0.parent_element().map(Self)
    }

    fn prev_sibling_element(&self) -> Option<Self> {
        self.0.prev_sibling_element().map(Self)
    }

    fn has_local_name(&self, name: &str) -> bool {
        self.0.tag_name().name() == name
    }

    fn attribute_matches(&self, name: &str, op: simplecss::AttributeOperator<'_>) -> bool {
        self.0.attribute(name).is_some_and(|value| op.matches(value))
    }

    fn pseudo_class_matches(&self, class: simplecss::PseudoClass<'_>) -> bool {
        // SVG import is static: only structural pseudo-classes affect selector matching.
        matches!(class, simplecss::PseudoClass::FirstChild) && self.0.prev_sibling_element().is_none()
    }
}

pub(super) fn stylesheet<'a, 'i>(xml: &'a usvg::roxmltree::Document<'i>) -> simplecss::StyleSheet<'a> {
    let mut sheet = simplecss::StyleSheet::new();
    for n in xml.descendants().filter(|n| n.has_tag_name("style")) {
        if n.attribute("type").is_some_and(|t| t != "text/css") {
            continue;
        }
        if let Some(text) = n.text() {
            sheet.parse_more(text);
        }
    }
    sheet
}

/// Use the renderer's selector matching and declaration precedence for display too, including
/// sibling selectors and structural pseudo-classes. Its first important declaration wins.
pub(super) fn hidden(n: XNode, sheet: &simplecss::StyleSheet) -> bool {
    let mut display = n.attribute("display").map(str::to_string);
    let mut important = false;
    let mut put = |d: &simplecss::Declaration| {
        if d.name == "display" && !important {
            display = Some(d.value.to_string());
            important = d.important;
        }
    };
    for rule in &sheet.rules {
        if rule.selector.matches(&Element(n)) {
            for d in &rule.declarations {
                put(d);
            }
        }
    }
    if let Some(inline) = n.attribute("style") {
        for d in simplecss::DeclarationTokenizer::from(inline) {
            put(&d);
        }
    }
    display.as_deref() == Some("none")
}

pub(super) fn freeze(svg: &str, xml: &usvg::roxmltree::Document, edits: &mut Edits, hidden: &HashSet<String>) -> Result<(), SvgError> {
    let sheet = stylesheet(xml);
    for n in xml.descendants().filter(|n| n.has_tag_name("style")) {
        if n.attribute("type").is_some_and(|t| t != "text/css") {
            continue;
        }
        // Every rule is copied to the elements it originally matched. Leaving the sheet in
        // place would also match newly generated ids or the temporary display styles.
        edits.list.push((n.range(), String::new()));
    }
    let mut copied = 0usize;
    for n in xml.descendants().filter(|n| n.is_element() && !n.has_tag_name("style")) {
        let id = n.attribute("id").filter(|id| !id.is_empty()).or_else(|| edits.ids.get(&n.id()).map(String::as_str));
        let shown = id.is_some_and(|id| hidden.contains(id));
        let mut style = String::new();
        for rule in &sheet.rules {
            if rule.selector.matches(&Element(n)) {
                for d in &rule.declarations {
                    // Bound the additional copies caused by matching a rule to many elements.
                    copied = copied.saturating_add(d.name.len() + d.value.len() + 13);
                    if copied > 8 * 1024 * 1024 {
                        return Err(SvgError::Parse("SVG stylesheet copies exceed the import size limit".into()));
                    }
                    declaration(&mut style, d, shown);
                }
            }
        }
        // Preserve inline declaration order, including shorthand/longhand interactions.
        if !style.is_empty() || shown {
            if let Some(inline) = n.attribute("style") {
                for d in simplecss::DeclarationTokenizer::from(inline) {
                    declaration(&mut style, &d, shown);
                }
            }
            if shown {
                style.push_str("display:inline;");
            }
            let escaped = xml_escape(&style).replace('\t', "&#x9;").replace('\n', "&#xA;").replace('\r', "&#xD;");
            edits.set(svg, n, "style", &escaped);
        }
    }
    Ok(())
}

fn declaration(out: &mut String, d: &simplecss::Declaration<'_>, shown: bool) {
    if shown && d.name == "display" {
        return;
    }
    // Writing to String is infallible.
    let _ = write!(out, "{}:{}{};", d.name, d.value, if d.important { "!important" } else { "" });
}
