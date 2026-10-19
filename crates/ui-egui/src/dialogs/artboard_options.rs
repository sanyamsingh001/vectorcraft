//! Artboard Options (double-click with the Artboard tool): the artboard's fields, sizes parsed
//! with units.

use serde_json::{Value, json};

use super::{DialogSpec, run_and_close};
use crate::VectorcraftApp;
use crate::state::Dialog;

pub(super) const SPEC: DialogSpec = DialogSpec { heading: |_| tl!("Artboard Options").into(), confirm, ..DialogSpec::FORM };

fn confirm(app: &mut VectorcraftApp, d: &Dialog) -> Result<Value, String> {
    let mut p = Value::Object(d.fields.clone());
    for k in ["x", "y", "width", "height"] {
        if d.fields.contains_key(k) {
            p[k] = json!(d.f64(k, 0.0));
        }
    }
    if p.get("scaleArt").is_none() {
        p["scaleArt"] = json!(crate::panels::artboards::scale_art(app));
    }
    run_and_close(app, "artboard.setProps", p)
}

#[cfg(test)]
mod tests {
    use crate::{VectorcraftApp, panels};
    use egui::{Event, Pos2, Rect, Shape, Ui, vec2};
    use serde_json::json;
    use vectorcraft_doc::NodeId;
    use vectorcraft_engine::Session;
    use vectorcraft_geom::Rect as DRect;

    struct Harness {
        app: VectorcraftApp,
        ctx: egui::Context,
        time: f64,
        art: NodeId,
        width: f32,
    }
    impl Harness {
        fn new() -> Self {
            let mut app = VectorcraftApp::new(Session::new(), Default::default());
            app.run("file.new", json!({"width":100,"height":100})).unwrap();
            let art = NodeId(app.run("shape.rectangle", json!({"x":10,"y":20,"width":10,"height":10})).unwrap()["id"].as_u64().unwrap());
            app.select_tool("artboard");
            let ctx = egui::Context::default();
            crate::theme::install_fonts(&ctx);
            Self { app, ctx, time: 0.0, art, width: 700.0 }
        }
        fn frame(&mut self, events: Vec<Event>, draw: fn(&mut VectorcraftApp, &mut Ui)) -> egui::FullOutput {
            self.time += 0.1;
            let mut out = self.ctx.run_ui(
                egui::RawInput {
                    events,
                    time: Some(self.time),
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(self.width, 600.0))),
                    ..Default::default()
                },
                |ui| draw(&mut self.app, ui),
            );
            out.textures_delta.clear();
            out
        }
        fn settle(&mut self, draw: fn(&mut VectorcraftApp, &mut Ui)) -> egui::FullOutput {
            self.frame(vec![], draw);
            self.frame(vec![], draw)
        }
        fn click(&mut self, p: Pos2, draw: fn(&mut VectorcraftApp, &mut Ui)) -> egui::FullOutput {
            self.frame(vec![Event::PointerMoved(p)], draw);
            for pressed in [true, false] {
                self.frame(vec![Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() }], draw);
            }
            self.settle(draw)
        }
        fn bounds(&self) -> DRect {
            self.app.session.active().unwrap().doc.node(self.art).unwrap().geometric_bounds().unwrap()
        }
    }
    fn at(out: &egui::FullOutput, label: &str) -> Pos2 {
        fn find(shape: &Shape, label: &str) -> Option<Pos2> {
            match shape {
                Shape::Text(t) if t.galley.text().trim() == label => Some(t.pos + vec2(4.0, 4.0)),
                Shape::Vec(v) => v.iter().find_map(|s| find(s, label)),
                _ => None,
            }
        }
        out.shapes.iter().find_map(|s| find(&s.shape, label)).unwrap_or_else(|| panic!("missing {label}: {}", crate::tests_labels::shapes_text(out)))
    }
    fn dialog(app: &mut VectorcraftApp, ui: &mut Ui) {
        super::super::show(app, ui.ctx());
    }
    fn control(app: &mut VectorcraftApp, ui: &mut Ui) {
        crate::chrome::control_bar(app, ui);
    }
    /// The full Control bar supplies the clipping and neighboring controls omitted by a
    /// tool-options-only harness. The entire label must be visible before a click is tried.
    fn control_text_rect(out: &egui::FullOutput, label: &str, width: f32, stage: &str) -> (Rect, Rect) {
        fn label_rect(shape: &Shape, label: &str) -> Option<Rect> {
            match shape {
                Shape::Text(t) if t.galley.text().trim() == label => Some(Rect::from_min_size(t.pos, t.galley.size())),
                Shape::Vec(v) => v.iter().find_map(|s| label_rect(s, label)),
                _ => None,
            }
        }
        out.shapes
            .iter()
            .find_map(|s| label_rect(&s.shape, label).map(|r| (r, s.clip_rect)))
            .unwrap_or_else(|| panic!("{stage} at {width} pt: missing {label}; shapes_text:\n{}", crate::tests_labels::shapes_text(out)))
    }
    fn visible_control(out: &egui::FullOutput, label: &str, width: f32, stage: &str) -> Pos2 {
        let (rect, clip) = control_text_rect(out, label, width, stage);
        assert!(
            clip.contains_rect(rect),
            "{stage}: {label} is clipped at {width} pt: {rect:?} within {clip:?}; shapes_text:\n{}",
            crate::tests_labels::shapes_text(out)
        );
        assert!(
            Rect::from_min_size(Pos2::ZERO, vec2(width, 600.0)).contains_rect(rect),
            "{stage}: {label} is outside the viewport at {width} pt: {rect:?}; shapes_text:\n{}",
            crate::tests_labels::shapes_text(out)
        );
        rect.center()
    }
    /// Text alone can fit while the surrounding input is cut off. Check its painted frame too.
    fn visible_field(out: &egui::FullOutput, label: &str, width: f32, stage: &str) -> Pos2 {
        visible_control(out, label, width, stage);
        let (text, _) = control_text_rect(out, label, width, stage);
        fn frames(shape: &Shape, text: Rect, found: &mut Vec<Rect>) {
            match shape {
                Shape::Rect(r) if r.rect.contains_rect(text) && r.rect.width() > text.width() && r.rect.height() > text.height() => {
                    found.push(r.rect)
                }
                Shape::Vec(v) => v.iter().for_each(|s| frames(s, text, found)),
                _ => {}
            }
        }
        let mut candidates = vec![];
        for s in &out.shapes {
            let mut found = vec![];
            frames(&s.shape, text, &mut found);
            candidates.extend(found.into_iter().map(|r| (r, s.clip_rect)));
        }
        let (rect, clip) = candidates
            .into_iter()
            .min_by(|(a, _), (b, _)| a.area().total_cmp(&b.area()))
            .unwrap_or_else(|| panic!("{stage} at {width} pt: missing {label} field frame; shapes_text:\n{}", crate::tests_labels::shapes_text(out)));
        assert!(
            clip.contains_rect(rect),
            "{stage}: {label} field frame is clipped at {width} pt: {rect:?} within {clip:?}; shapes_text:\n{}",
            crate::tests_labels::shapes_text(out)
        );
        assert!(
            Rect::from_min_size(Pos2::ZERO, vec2(width, 34.0)).contains_rect(rect),
            "{stage}: {label} field frame is outside the fixed Control bar at {width} pt: {rect:?}; shapes_text:\n{}",
            crate::tests_labels::shapes_text(out)
        );
        rect.center()
    }
    fn visible_neighbors(h: &Harness, out: &egui::FullOutput, width: f32, stage: &str) -> Pos2 {
        visible_control(out, "Stroke:", width, stage);
        visible_control(out, "Opacity:", width, stage);
        let stroke = h.app.session.stroke_unit().format(h.app.session.paint.stroke_width);
        visible_field(out, &stroke, width, stage);
        visible_field(out, "100%", width, stage)
    }
    #[test]
    fn scale_art_control_toggle_persists_and_properties_can_turn_it_off() {
        for width in [800.0, 1200.0] {
            let mut h = Harness::new();
            h.width = width;
            // Same selected original rectangle and generic controls, without Artboard options:
            // the Selection frame is a baseline for any neighbor displaced by the new controls.
            h.app.select_tool("selection");
            let baseline = h.settle(control);
            visible_neighbors(&h, &baseline, width, "Selection baseline");
            h.app.select_tool("artboard");
            let mut out = h.settle(control);
            visible_neighbors(&h, &out, width, "Artboard initial");
            if width == 800.0 {
                let options_at = visible_control(&out, "Artboard Options", width, "Artboard compact options");
                out = h.click(options_at, control);
                visible_neighbors(&h, &out, width, "Artboard compact options open");
            }
            let move_at = visible_control(&out, "Move Artwork with Artboard", width, "Artboard initial");
            visible_control(&out, "Scale Artwork with Artboard", width, "Artboard initial");
            h.click(move_at, control);
            assert_eq!(h.app.session.tool_options()["moveArt"], false);
            let out = h.settle(control);
            h.click(visible_control(&out, "Scale Artwork with Artboard", width, "Artboard after Move toggle"), control);
            assert_eq!(h.app.session.tool_options()["scaleArt"], true);
            h.app.select_tool("selection");
            h.app.select_tool("artboard");
            assert_eq!(h.app.session.tool_options()["scaleArt"], true);
            assert_eq!(h.app.session.tool_options()["moveArt"], false);
            assert_eq!(h.app.session.prefs.tool_settings["artboard"]["scaleArt"], true);
            // A neighboring field must still receive real pointer and keyboard input after
            // both toggles, not merely have its label painted somewhere in the output.
            let out = h.settle(control);
            let opacity = visible_neighbors(&h, &out, width, "Artboard after tool reselection");
            h.click(opacity, control);
            h.frame(
                vec![Event::Key { key: egui::Key::A, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::COMMAND }],
                control,
            );
            h.frame(vec![Event::Text("75%".into())], control);
            for pressed in [true, false] {
                h.frame(
                    vec![Event::Key { key: egui::Key::Enter, physical_key: None, pressed, repeat: false, modifiers: Default::default() }],
                    control,
                );
            }
            let out = h.settle(control);
            visible_field(&out, "75%", width, "Artboard after opacity edit");
            assert!((panels::current_transparency(&h.app).unwrap().0 - 0.75).abs() < 1e-6);
            // Preferences use the same serde roundtrip when they are saved and reopened.
            let mut reopened = Session::new();
            reopened.prefs = serde_json::from_value(serde_json::to_value(&h.app.session.prefs).unwrap()).unwrap();
            reopened.execute("file.new", &json!({"width":100,"height":100})).unwrap();
            reopened.select_tool("artboard", Default::default()).unwrap();
            assert_eq!(reopened.tool_options()["scaleArt"], true);
            assert_eq!(reopened.tool_options()["moveArt"], false);
            let out = h.settle(panels::properties::show);
            h.click(at(&out, "Scale Artwork with Artboard"), panels::properties::show);
            assert_eq!(h.app.session.tool_options()["scaleArt"], false);
            assert_eq!(h.app.session.prefs.tool_settings["artboard"]["scaleArt"], false);
            assert_eq!(h.bounds(), DRect::new(10.0, 20.0, 20.0, 30.0));
        }
    }
    #[test]
    fn scale_art_dialog_uses_current_option_for_one_undoable_resize_and_cancel_keeps_it() {
        for scale in [false, true] {
            let mut h = Harness::new();
            let out = h.settle(panels::properties::show);
            if scale {
                h.click(at(&out, "Scale Artwork with Artboard"), panels::properties::show);
            }
            assert_eq!(h.app.session.tool_options()["scaleArt"], scale);
            super::super::open_tool_dialog(&mut h.app, "artboardOptions", json!({"index":0,"width":100,"height":100,"name":"Page"}));
            h.settle(dialog);
            let d = h.app.ui.dialog.as_mut().unwrap();
            d.fields.insert("width".into(), json!(200));
            d.fields.insert("height".into(), json!(200));
            let undo = h.app.session.active().unwrap().history.undo.len();
            let out = h.settle(dialog);
            h.click(at(&out, "OK"), dialog);
            assert!(h.app.ui.dialog.is_none());
            assert_eq!(h.app.session.tool_options()["scaleArt"], scale);
            let expected = if scale { DRect::new(20.0, 40.0, 40.0, 60.0) } else { DRect::new(10.0, 20.0, 20.0, 30.0) };
            assert_eq!(h.bounds(), expected);
            assert_eq!(h.app.session.active().unwrap().doc.artboards[0].rect, DRect::new(0.0, 0.0, 200.0, 200.0));
            assert_eq!(h.app.session.active().unwrap().history.undo.len(), undo + 1);
            h.app.run("edit.undo", json!({})).unwrap();
            assert_eq!(h.bounds(), DRect::new(10.0, 20.0, 20.0, 30.0));
            super::super::open_tool_dialog(&mut h.app, "artboardOptions", json!({"index":0,"width":300,"height":300}));
            let out = h.settle(dialog);
            h.click(at(&out, "Cancel"), dialog);
            assert_eq!(h.app.session.tool_options()["scaleArt"], scale);
            assert_eq!(h.bounds(), DRect::new(10.0, 20.0, 20.0, 30.0));
            assert_eq!(h.app.session.active().unwrap().doc.artboards[0].rect, DRect::new(0.0, 0.0, 100.0, 100.0));
            assert_eq!(h.app.session.active().unwrap().history.undo.len(), undo);
        }
    }
    #[test]
    fn scale_art_transform_width_edit_uses_the_artboard_reference_point() {
        let mut h = Harness::new();
        h.app.run("tool.setOption", json!({"key":"scaleArt","value":true})).unwrap();
        let undo = h.app.session.active().unwrap().history.undo.len();
        let out = h.settle(panels::transform::show);
        h.click(at(&out, "100 pt"), panels::transform::show);
        h.frame(
            vec![Event::Key { key: egui::Key::A, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::COMMAND }],
            panels::transform::show,
        );
        h.frame(vec![Event::Text("200 pt".into())], panels::transform::show);
        h.frame(
            vec![Event::Key { key: egui::Key::Enter, physical_key: None, pressed: true, repeat: false, modifiers: Default::default() }],
            panels::transform::show,
        );
        h.settle(panels::transform::show);
        assert_eq!(h.app.session.active().unwrap().doc.artboards[0].rect, DRect::new(-50.0, 0.0, 150.0, 100.0));
        assert_eq!(h.bounds(), DRect::new(-30.0, 20.0, -10.0, 30.0));
        assert_eq!(h.app.session.active().unwrap().history.undo.len(), undo + 1);
    }
}
