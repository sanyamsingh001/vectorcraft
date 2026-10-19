//! Align panel: Align Objects, Distribute Objects, Distribute Spacing (with a spacing value when
//! aligning to a key object) and Align To (selection / key object / artboard).

use egui::Ui;
use serde_json::{Value, json};

use super::{pstate, selection_len, set_pstate};
use crate::VectorcraftApp;
use crate::widgets::{self, menu_item};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AlignTo {
    #[default]
    Selection,
    Key,
    Artboard,
}

impl AlignTo {
    pub fn param(self) -> &'static str {
        match self {
            AlignTo::Selection => "selection",
            AlignTo::Key => "key",
            AlignTo::Artboard => "artboard",
        }
    }
}

/// `object.align` params: a single object aligns to the artboard (Illustrator does the same).
pub fn align_params(base: Value, to: AlignTo, n_selected: usize) -> Value {
    let mut p = base;
    let to = if n_selected == 1 && to == AlignTo::Selection { AlignTo::Artboard } else { to };
    p["to"] = json!(to.param());
    p
}

/// The canvas key object drives every alignment surface, including when this panel is closed.
/// Clearing it returns Key mode to Selection; a chosen Artboard mode lasts until a key is set.
pub(crate) fn align_to(app: &VectorcraftApp, ctx: &egui::Context) -> AlignTo {
    let stored: AlignTo = pstate(ctx, "align-to");
    let to = if app.session.active().is_some_and(|st| st.selection.key.is_some()) {
        AlignTo::Key
    } else if stored == AlignTo::Key {
        AlignTo::Selection
    } else {
        stored
    };
    set_pstate(ctx, "align-to", to);
    to
}

fn set_align_to(app: &mut VectorcraftApp, ctx: &egui::Context, to: AlignTo) {
    let id =
        app.session.active().filter(|st| st.selection.len() > 1).and_then(|st| st.selection.key.or_else(|| st.selection.objects.first().copied()));
    let params = if to == AlignTo::Key {
        let Some(id) = id else { return };
        json!({ "id": id.0 })
    } else {
        json!({})
    };
    if app.run("select.key", params).is_ok() {
        set_pstate(ctx, "align-to", to);
    }
}

pub const ALIGN: [(&str, &str, &str, &str); 6] = [
    ("dc-al-left", "Horizontal Align Left", "horizontal", "left"),
    ("dc-al-hcenter", "Horizontal Align Center", "horizontal", "center"),
    ("dc-al-right", "Horizontal Align Right", "horizontal", "right"),
    ("dc-al-top", "Vertical Align Top", "vertical", "top"),
    ("dc-al-vcenter", "Vertical Align Center", "vertical", "center"),
    ("dc-al-bottom", "Vertical Align Bottom", "vertical", "bottom"),
];

pub const DISTRIBUTE: [(&str, &str, &str, &str); 6] = [
    ("dc-dist-top", "Vertical Distribute Top", "vertical", "top"),
    ("dc-dist-vcenter", "Vertical Distribute Center", "vertical", "center"),
    ("dc-dist-bottom", "Vertical Distribute Bottom", "vertical", "bottom"),
    ("dc-dist-left", "Horizontal Distribute Left", "horizontal", "left"),
    ("dc-dist-hcenter", "Horizontal Distribute Center", "horizontal", "center"),
    ("dc-dist-right", "Horizontal Distribute Right", "horizontal", "right"),
];

pub fn show(app: &mut VectorcraftApp, ui: &mut Ui) {
    let n = selection_len(app);
    let to = align_to(app, ui.ctx());
    widgets::subheader(ui, tl!("Align Objects:"));
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 5.0;
        for (i, (icon, tip, axis, v)) in ALIGN.iter().enumerate() {
            if i == 3 {
                ui.add_space(6.0);
            }
            if widgets::icon_button_enabled(ui, icon, tl!(tip), false, n >= 1, 32.0).clicked() {
                app.run("object.align", align_params(json!({*axis: v}), to, n)).ok();
            }
        }
    });
    widgets::divider(ui);
    widgets::subheader(ui, tl!("Distribute Objects:"));
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 5.0;
        for (i, (icon, tip, axis, v)) in DISTRIBUTE.iter().enumerate() {
            if i == 3 {
                ui.add_space(6.0);
            }
            if widgets::icon_button_enabled(ui, icon, tl!(tip), false, n >= 2, 32.0).clicked() {
                app.run("object.distribute", json!({*axis: v})).ok();
            }
        }
    });
    if pstate::<bool>(ui.ctx(), "align-hide-options") {
        return;
    }
    widgets::divider(ui);
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            widgets::subheader(ui, tl!("Distribute Spacing:"));
            let spacing: f64 = pstate::<Option<f64>>(ui.ctx(), "align-spacing").unwrap_or(0.0);
            let key = to == AlignTo::Key;
            ui.horizontal(|ui| {
                let on = n >= 2;
                let sp = if key { Some(spacing) } else { None };
                if widgets::icon_button_enabled(ui, "dc-dist-vspace", tl!("Vertical Distribute Space"), false, on, 28.0).clicked() {
                    app.run("object.distributeSpacing", json!({"axis": "vertical", "spacing": sp})).ok();
                }
                if widgets::icon_button_enabled(ui, "dc-dist-hspace", tl!("Horizontal Distribute Space"), false, on, 28.0).clicked() {
                    app.run("object.distributeSpacing", json!({"axis": "horizontal", "spacing": sp})).ok();
                }
                ui.add_enabled_ui(key, |ui| {
                    if let Some(v) = widgets::spin_field(ui, "align-spacing", Some(spacing), app.session.general_unit(), 70.0, 1.0, 0.0, &[]) {
                        set_pstate(ui.ctx(), "align-spacing", Some(v));
                    }
                });
            });
        });
        ui.separator();
        ui.vertical(|ui| {
            widgets::subheader(ui, tl!("Align To:"));
            ui.horizontal(|ui| {
                for (v, icon, tip) in [
                    (AlignTo::Selection, "dc-alignto-selection", tl!("Align to Selection")),
                    (AlignTo::Key, "dc-alignto-key", tl!("Align to Key Object")),
                    (AlignTo::Artboard, "dc-alignto-artboard", tl!("Align to Artboard")),
                ] {
                    if widgets::icon_button_enabled(ui, icon, tl!(tip), to == v, v != AlignTo::Key || n >= 2, 28.0).clicked() {
                        set_align_to(app, ui.ctx(), v);
                    }
                }
            });
        });
    });
}

pub fn menu(app: &mut VectorcraftApp, ui: &mut Ui) {
    let hidden: bool = pstate(ui.ctx(), "align-hide-options");
    if menu_item(ui, if hidden { tl!("Show Options") } else { tl!("Hide Options") }, true, false) {
        set_pstate(ui.ctx(), "align-hide-options", !hidden);
    }
    let pb = app.session.prefs.use_preview_bounds;
    if menu_item(ui, tl!("Use Preview Bounds"), true, pb) {
        super::transform::set_pref(app, "usePreviewBounds", !pb);
    }
    let to = align_to(app, ui.ctx());
    if menu_item(ui, tl!("Cancel Key Object"), to == AlignTo::Key, false) {
        set_align_to(app, ui.ctx(), AlignTo::Selection);
    }
    menu_item(ui, tl!("Align to Glyph Bounds"), false, false);
}

#[cfg(test)]
mod tests {
    use super::*;

    use super::super::tests_appearance::{click, frame_events, text_rect};
    use vectorcraft_doc::NodeId;
    use vectorcraft_engine::Session;
    use vectorcraft_geom::{Point, Rect};
    use vectorcraft_tools::{PointerEvent, PointerKind};

    fn selected_art() -> (VectorcraftApp, [NodeId; 3]) {
        let mut app = VectorcraftApp::new(Session::new(), Default::default());
        app.run("file.new", json!({"width": 600, "height": 600})).unwrap();
        let ids = [(100, 100), (240, 240), (400, 400)]
            .map(|(x, y)| NodeId(app.run("shape.rectangle", json!({"x": x, "y": y, "width": 60, "height": 60})).unwrap()["id"].as_u64().unwrap()));
        app.run("select.set", json!({"ids": ids.iter().map(|id| id.0).collect::<Vec<_>>()})).unwrap();
        (app, ids)
    }

    fn bounds(app: &VectorcraftApp, id: NodeId) -> Rect {
        app.session.active().unwrap().doc.node(id).unwrap().geometric_bounds().unwrap()
    }

    fn canvas_click(app: &mut VectorcraftApp, p: Point) {
        for kind in [PointerKind::Down, PointerKind::Up] {
            let view = app.view_info();
            app.session.pointer(&PointerEvent::new(kind, p.x, p.y), view).unwrap();
        }
    }

    #[test]
    fn key_object_panel_follows_canvas_clicks_aligns_and_cancels_from_its_menu() {
        let ctx = egui::Context::default();
        let (mut app, ids) = selected_art();
        set_pstate(&ctx, "align-to", AlignTo::Artboard);
        let key_bounds = bounds(&app, ids[1]);
        canvas_click(&mut app, key_bounds.center());
        let texts = frame_events(&ctx, &mut app, vec![], show);
        assert_eq!(pstate::<AlignTo>(&ctx, "align-to"), AlignTo::Key);
        let label = text_rect(&texts, "Align Objects:");
        let left = egui::pos2(label.left() + 16.0, label.bottom() + ctx.global_style().spacing.item_spacing.y + 16.0);
        click(&ctx, &mut app, left, show);
        assert_eq!(bounds(&app, ids[1]), key_bounds);
        for id in [ids[0], ids[2]] {
            assert_eq!(bounds(&app, id).x0, key_bounds.x0, "Align button uses the canvas key");
        }
        let texts = frame_events(&ctx, &mut app, vec![], menu);
        let cancel = texts.iter().find(|(text, _)| text.trim() == "Cancel Key Object").unwrap().1.center();
        click(&ctx, &mut app, cancel, menu);
        assert_eq!(app.session.active().unwrap().selection.key, None);
        assert_eq!(pstate::<AlignTo>(&ctx, "align-to"), AlignTo::Selection);
        assert_eq!(app.session.active().unwrap().selection.objects, ids);
        canvas_click(&mut app, key_bounds.center());
        frame_events(&ctx, &mut app, vec![], show);
        assert_eq!(pstate::<AlignTo>(&ctx, "align-to"), AlignTo::Key);
        canvas_click(&mut app, key_bounds.center());
        frame_events(&ctx, &mut app, vec![], show);
        assert_eq!(pstate::<AlignTo>(&ctx, "align-to"), AlignTo::Selection);
    }

    #[test]
    fn key_object_panel_spacing_uses_its_value_and_keeps_the_key_in_place() {
        let ctx = egui::Context::default();
        let (mut app, ids) = selected_art();
        let key_bounds = bounds(&app, ids[1]);
        canvas_click(&mut app, key_bounds.center());
        set_pstate(&ctx, "align-spacing", Some(17.0_f64));
        let texts = frame_events(&ctx, &mut app, vec![], show);
        let label = text_rect(&texts, "Distribute Spacing:");
        let horizontal = egui::pos2(
            label.left() + 28.0 + ctx.global_style().spacing.item_spacing.x + 14.0,
            label.bottom() + ctx.global_style().spacing.item_spacing.y + 14.0,
        );
        click(&ctx, &mut app, horizontal, show);
        assert_eq!(bounds(&app, ids[1]), key_bounds);
        assert_eq!(bounds(&app, ids[1]).x0 - bounds(&app, ids[0]).x1, 17.0);
        assert_eq!(bounds(&app, ids[2]).x0 - bounds(&app, ids[1]).x1, 17.0);
    }

    #[test]
    fn key_object_properties_and_control_bar_align_without_opening_the_align_panel() {
        for control_bar in [false, true] {
            let ctx = egui::Context::default();
            crate::theme::install_fonts(&ctx);
            let (mut app, ids) = selected_art();
            let key_bounds = bounds(&app, ids[1]);
            canvas_click(&mut app, key_bounds.center());
            let f: fn(&mut VectorcraftApp, &mut egui::Ui) = if control_bar { crate::chrome::control_bar } else { super::super::properties::show };
            let button = if control_bar {
                let mut out = ctx.run_ui(Default::default(), |ui| f(&mut app, ui));
                out.textures_delta.clear();
                fn buttons(s: &egui::Shape, found: &mut Vec<egui::Rect>) {
                    match s {
                        egui::Shape::Rect(r) if (r.rect.width() - 24.0).abs() < 0.01 && (r.rect.height() - 24.0).abs() < 0.01 => found.push(r.rect),
                        egui::Shape::Vec(v) => v.iter().for_each(|s| buttons(s, found)),
                        _ => {}
                    }
                }
                let mut found = vec![];
                for s in &out.shapes {
                    buttons(&s.shape, &mut found);
                }
                assert_eq!(found.len(), 6, "six alignment icon buttons");
                found[0].center()
            } else {
                let texts = frame_events(&ctx, &mut app, vec![], f);
                let label = text_rect(&texts, "Align");
                egui::pos2(label.left() + 13.0, label.bottom() + ctx.global_style().spacing.item_spacing.y + 13.0)
            };
            click(&ctx, &mut app, button, f);
            assert_eq!(bounds(&app, ids[1]), key_bounds);
            assert_eq!(bounds(&app, ids[0]).x0, key_bounds.x0, "surface control bar: {control_bar}");
            assert_eq!(bounds(&app, ids[2]).x0, key_bounds.x0);
            app.run("select.set", json!({"ids": [ids[0].0, ids[2].0]})).unwrap();
            assert_eq!(align_to(&app, &ctx), AlignTo::Selection);
        }
    }

    #[test]
    fn key_object_menu_alignment_honors_the_key_and_explicit_targets() {
        for (_, _, axis, value) in ALIGN {
            for to in [None, Some("selection"), Some("artboard"), Some("key")] {
                let (mut app, ids) = selected_art();
                let original = ids.map(|id| bounds(&app, id));
                canvas_click(&mut app, original[1].center());
                let undo = app.session.active().unwrap().history.undo.len();
                let reference = match to {
                    Some("selection") => original.iter().copied().reduce(Rect::union).unwrap(),
                    Some("artboard") => app.session.active().unwrap().doc.artboards[0].rect,
                    _ => original[1],
                };
                let coordinate = |r: Rect| match (axis, value) {
                    ("horizontal", "left") => r.x0,
                    ("horizontal", "center") => r.center().x,
                    ("horizontal", "right") => r.x1,
                    ("vertical", "top") => r.y0,
                    ("vertical", "center") => r.center().y,
                    ("vertical", "bottom") => r.y1,
                    _ => panic!("unexpected alignment direction"),
                };
                let mut params = json!({axis: value});
                if let Some(to) = to {
                    params["to"] = json!(to);
                }
                // Object > Align and shortcut invocations go through this handler.
                crate::menus::invoke(&mut app, "object.align", params);
                if to.is_none() || to == Some("key") {
                    assert_eq!(bounds(&app, ids[1]), original[1], "{axis} {value}: key stays fixed");
                }
                for id in ids {
                    assert_eq!(coordinate(bounds(&app, id)), coordinate(reference), "{axis} {value} to {to:?}");
                }
                let after = ids.map(|id| bounds(&app, id));
                let st = app.session.active().unwrap();
                assert_eq!(st.selection.objects, ids);
                assert_eq!(st.selection.key, Some(ids[1]));
                assert_eq!(st.history.undo.len(), undo + 1);
                app.run("edit.undo", json!({})).unwrap();
                assert_eq!(ids.map(|id| bounds(&app, id)), original);
                app.run("edit.redo", json!({})).unwrap();
                assert_eq!(ids.map(|id| bounds(&app, id)), after);
            }
        }
    }

    #[test]
    fn key_object_align_to_modes_create_and_clear_the_key() {
        let ctx = egui::Context::default();
        let (mut app, ids) = selected_art();
        set_align_to(&mut app, &ctx, AlignTo::Key);
        assert_eq!(app.session.active().unwrap().selection.key, Some(ids[0]));
        assert_eq!(align_to(&app, &ctx), AlignTo::Key);
        set_align_to(&mut app, &ctx, AlignTo::Artboard);
        assert_eq!(app.session.active().unwrap().selection.key, None);
        assert_eq!(align_to(&app, &ctx), AlignTo::Artboard);
        app.run("select.set", json!({"ids": [ids[0].0]})).unwrap();
        set_align_to(&mut app, &ctx, AlignTo::Key);
        assert_eq!(app.session.active().unwrap().selection.key, None);
        assert_eq!(align_to(&app, &ctx), AlignTo::Artboard);
    }

    #[test]
    fn single_object_aligns_to_artboard() {
        let p = align_params(json!({"horizontal": "left"}), AlignTo::Selection, 1);
        assert_eq!(p["to"], "artboard");
        let p = align_params(json!({"vertical": "top"}), AlignTo::Selection, 3);
        assert_eq!(p["to"], "selection");
        assert_eq!(align_params(json!({}), AlignTo::Key, 1)["to"], "key");
    }
}
