//! Image Trace objects' View: how a live trace draws on screen.
//!
//! An Image Trace object is a group whose `trace` holds `{preset, params, view}` (see
//! `imageTrace.make`). The view is a viewing aid only: exports, printing and Expand always use the
//! tracing result, whatever it is set to.

use crate::node::{Node, NodeKind};

/// Image Trace › View.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TraceView {
    /// The traced shapes as they are filled.
    #[default]
    Result,
    /// The traced shapes, with their outlines over them.
    ResultWithOutlines,
    /// Only the traced shapes' outlines.
    Outlines,
    /// The source image, with the traced shapes' outlines over it.
    OutlinesWithSource,
    /// Only the source image.
    Source,
}

impl TraceView {
    /// Panel order.
    pub const ALL: [Self; 5] = [Self::Result, Self::ResultWithOutlines, Self::Outlines, Self::OutlinesWithSource, Self::Source];

    /// The id used in command params and in the saved `trace` object.
    pub fn id(self) -> &'static str {
        match self {
            Self::Result => "tracingResult",
            Self::ResultWithOutlines => "tracingResultWithOutlines",
            Self::Outlines => "outlines",
            Self::OutlinesWithSource => "outlinesWithSourceImage",
            Self::Source => "sourceImage",
        }
    }

    /// The view an id names (any case).
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|v| v.id().eq_ignore_ascii_case(id))
    }

    /// The view an Image Trace object's `trace` settings ask for (the tracing result when they
    /// name none, or an unknown one).
    pub fn of(trace: &serde_json::Value) -> Self {
        trace.get("view").and_then(serde_json::Value::as_str).and_then(Self::from_id).unwrap_or_default()
    }

    /// Whether the source image shows.
    pub fn shows_source(self) -> bool {
        matches!(self, Self::OutlinesWithSource | Self::Source)
    }

    /// Whether the traced shapes show filled.
    pub fn shows_result(self) -> bool {
        matches!(self, Self::Result | Self::ResultWithOutlines)
    }

    /// Whether the traced shapes' outlines show.
    pub fn shows_outlines(self) -> bool {
        matches!(self, Self::ResultWithOutlines | Self::Outlines | Self::OutlinesWithSource)
    }
}

impl Node {
    /// Whether this is a live Image Trace group with its source image first.
    ///
    /// Stored trace settings keep the identity independent of the editable name. The exact
    /// "Image Trace" name also recognizes older groups that have no stored settings.
    pub fn is_image_trace(&self) -> bool {
        matches!(self.kind, NodeKind::Group { clip: false, .. })
            && self.children().is_some_and(|c| c.first().is_some_and(|i| matches!(i.kind, NodeKind::Image(_))))
            && (self.trace.is_some() || self.name.as_deref() == Some("Image Trace"))
    }

    /// The View of an Image Trace object (the tracing result for anything else, or an unknown id).
    pub fn trace_view(&self) -> TraceView {
        self.trace.as_deref().map(TraceView::of).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_round_trip_and_unknown_views_show_the_result() {
        for v in TraceView::ALL {
            assert_eq!(TraceView::from_id(v.id()), Some(v));
            assert_eq!(TraceView::from_id(&v.id().to_ascii_uppercase()), Some(v));
            assert!(v.shows_result() || v.shows_outlines() || v.shows_source(), "{v:?} shows something");
        }
        let mut g = Node::group(crate::NodeId(1), vec![]);
        assert_eq!(g.trace_view(), TraceView::Result);
        g.trace = Some(Box::new(serde_json::json!({"view": "outlines"})));
        assert_eq!(g.trace_view(), TraceView::Outlines);
        g.trace = Some(Box::new(serde_json::json!({"view": "sepia"})));
        assert_eq!(g.trace_view(), TraceView::Result);
    }

    fn identity_image(visible: bool) -> Node {
        let mut n = Node::new(
            crate::NodeId(2),
            NodeKind::Image(crate::ImageObject {
                key: "original-2x2".into(),
                width: 2,
                height: 2,
                xf: vectorcraft_geom::Affine::IDENTITY,
                link: None,
                placement: Default::default(),
            }),
        );
        n.visible = visible;
        n
    }

    fn identity_rectangle() -> Node {
        Node::path(
            crate::NodeId(3),
            vectorcraft_geom::shapes::rectangle(vectorcraft_geom::Rect::new(0.0, 0.0, 2.0, 2.0)),
            crate::Appearance::default(),
        )
    }

    fn identity_group(name: Option<&str>, tagged: bool, children: Vec<Node>, clip: bool) -> Node {
        let mut n = Node::new(crate::NodeId(1), NodeKind::Group { children: children.into_iter().map(std::sync::Arc::new).collect(), clip });
        n.name = name.map(str::to_string);
        if tagged {
            n.trace = Some(Box::new(serde_json::json!({"preset": "Custom", "params": {"noise": 1}, "view": "tracingResult"})));
        }
        n
    }

    #[test]
    fn tagged_trace_identity_survives_rename_and_roundtrip() {
        for name in [Some("Image Trace"), Some("Logo outline"), Some(""), None] {
            for visible in [false, true] {
                let n = identity_group(name, true, vec![identity_image(visible), identity_rectangle()], false);
                assert!(n.is_image_trace(), "tagged trace {name:?}, source visible={visible}");
                let encoded = serde_json::to_value(&n).unwrap();
                let decoded: Node = serde_json::from_value(encoded).unwrap();
                assert_eq!(decoded, n);
                assert!(decoded.is_image_trace(), "saved tagged trace {name:?}, source visible={visible}");
            }
        }
        let mut n = identity_group(Some("Logo outline"), true, vec![identity_image(false)], false);
        n.trace = Some(Box::new(serde_json::json!({"view": "unknown"})));
        assert!(n.is_image_trace(), "identity does not validate or rewrite stored trace settings");
    }

    #[test]
    fn metadata_free_legacy_trace_identity_matches_original_rule() {
        // Literal expectations preserve the original exact-name + unclipped-group + first-image rule.
        // Source visibility and a source-only group's lack of result paths never restricted that rule.
        let cases = [
            ("hidden source only", Some("Image Trace"), vec![identity_image(false)], false, true),
            ("visible source only", Some("Image Trace"), vec![identity_image(true)], false, true),
            ("source and rectangle", Some("Image Trace"), vec![identity_image(false), identity_rectangle()], false, true),
            ("different case", Some("image trace"), vec![identity_image(false)], false, false),
            ("trailing space", Some("Image Trace "), vec![identity_image(false)], false, false),
            ("ordinary name", Some("Logo outline"), vec![identity_image(false)], false, false),
            ("empty name", Some(""), vec![identity_image(false)], false, false),
            ("no name", None, vec![identity_image(false)], false, false),
            ("empty group", Some("Image Trace"), vec![], false, false),
            ("paths only", Some("Image Trace"), vec![identity_rectangle()], false, false),
            ("source second", Some("Image Trace"), vec![identity_rectangle(), identity_image(false)], false, false),
            ("clipped group", Some("Image Trace"), vec![identity_image(false)], true, false),
        ];
        for (label, name, children, clip, expected) in cases {
            let n = identity_group(name, false, children, clip);
            assert!(n.trace.is_none());
            assert_eq!(n.is_image_trace(), expected, "{label}");
            let encoded = serde_json::to_value(&n).unwrap();
            assert!(encoded.get("trace").is_none(), "legacy fixture has no serialized trace field");
            let decoded: Node = serde_json::from_value(encoded).unwrap();
            assert_eq!(decoded.is_image_trace(), expected, "{label} after save/open");
        }
        for mut n in [identity_image(false), identity_rectangle()] {
            n.name = Some("Image Trace".into());
            assert!(!n.is_image_trace(), "the legacy name alone cannot identify a non-group");
        }
    }

    #[test]
    fn ordinary_and_expanded_art_are_not_live_traces() {
        let ordinary = identity_group(Some("Photo and rectangle"), false, vec![identity_image(true), identity_rectangle()], false);
        assert!(!ordinary.is_image_trace());
        let mut expanded = identity_group(Some("Logo outline"), true, vec![identity_image(false), identity_rectangle()], false);
        expanded.name = None;
        expanded.trace = None;
        expanded.children_mut().unwrap().retain(|n| !matches!(n.kind, NodeKind::Image(_)));
        assert!(!expanded.is_image_trace(), "Expand removes the source and live trace settings");
        expanded.name = Some("Image Trace".into());
        assert!(!expanded.is_image_trace(), "renaming expanded paths cannot make them live");

        for n in [
            identity_group(Some("Logo outline"), true, vec![], false),
            identity_group(Some("Logo outline"), true, vec![identity_rectangle()], false),
            identity_group(Some("Logo outline"), true, vec![identity_rectangle(), identity_image(false)], false),
            identity_group(Some("Image Trace"), true, vec![identity_image(false)], true),
        ] {
            assert!(!n.is_image_trace(), "stored settings cannot bypass the source-first, unclipped group shape");
        }
        for mut n in [identity_image(false), identity_rectangle()] {
            n.name = Some("Image Trace".into());
            n.trace = Some(Box::new(serde_json::json!({"view": "sourceImage"})));
            assert!(!n.is_image_trace(), "stored settings cannot turn an image or path into a trace group");
        }
    }
}
