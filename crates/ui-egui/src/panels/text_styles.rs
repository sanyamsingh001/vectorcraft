//! Character Styles and Paragraph Styles panels (one implementation, two kinds).
//!
//! Click a style to apply it to the selected text (or the Type tool's selected characters);
//! Alt-click applies it and clears overrides. The style of the selection is highlighted, with a
//! "+" when the text overrides some of its attributes.

use egui::{Sense, Ui, vec2};
use serde_json::{Map, Value, json};

use super::character::{text_editing, text_style};
use super::{pstate, selection_len, set_pstate};
use crate::VectorcraftApp;
use crate::theme::Tokens;
use crate::widgets::{self, menu_item};

#[derive(Clone, Copy, PartialEq)]
pub enum Kind {
    Char,
    Para,
}

impl Kind {
    fn prefix(self) -> &'static str {
        if self == Kind::Char { "charStyle" } else { "paraStyle" }
    }
    fn what(self) -> &'static str {
        if self == Kind::Char { "Character" } else { "Paragraph" }
    }
    /// `what` in the UI language, for text shown to the user.
    fn what_label(self) -> &'static str {
        if self == Kind::Char { tl!("Character") } else { tl!("Paragraph") }
    }
    fn key(self) -> &'static str {
        if self == Kind::Char { "char-style-sel" } else { "para-style-sel" }
    }
}

/// (name, attrs, uses) for every style, Normal first.
fn styles(app: &mut VectorcraftApp, kind: Kind) -> Vec<(String, Map<String, Value>, u64)> {
    let v = app.session.execute(&format!("{}.list", kind.prefix()), &json!({})).unwrap_or_default();
    v["styles"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|s| {
                    (
                        s["name"].as_str().unwrap_or("").to_string(),
                        s["attrs"].as_object().cloned().unwrap_or_default(),
                        s["uses"].as_u64().unwrap_or(0),
                    )
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The selection's style name (Normal when none) and its attributes, if text is selected.
fn current(app: &VectorcraftApp, kind: Kind) -> Option<(Option<String>, Map<String, Value>)> {
    let (c, p) = text_style(app)?;
    let v = match kind {
        Kind::Char => serde_json::to_value(&c),
        Kind::Para => serde_json::to_value(&p),
    }
    .ok()?;
    let mut m = v.as_object()?.clone();
    let name = m.remove("style_name").and_then(|v| v.as_str().map(str::to_string));
    Some((name, m))
}

/// Parameters targeting the selection: the Type tool's selected characters (paragraph styles: the
/// paragraphs its selection or caret touches), or the selected objects.
fn target(app: &VectorcraftApp, kind: Kind, mut p: Value) -> Value {
    if let Some((id, a, b)) = text_editing(app) {
        p["id"] = json!(id.0);
        if kind == Kind::Para || b > a {
            p["start"] = json!(a);
            p["end"] = json!(b);
        }
    }
    p
}

pub fn show(app: &mut VectorcraftApp, ui: &mut Ui, kind: Kind) {
    let t = Tokens::get(ui.ctx());
    let all = styles(app, kind);
    let cur = current(app, kind);
    let normal = all.first().map(|s| s.0.clone()).unwrap_or_default();
    let cur_name = cur.as_ref().map(|(n, _)| n.clone().unwrap_or_else(|| normal.clone()));
    let mut clicked: Option<(String, bool)> = None;
    let mut sel: Option<String> = pstate(ui.ctx(), kind.key());
    widgets::list_box(ui, |ui| {
        for (name, attrs, _) in &all {
            let is_cur = cur_name.as_deref() == Some(name.as_str());
            // Overrides: an attribute the style sets that the selection doesn't match.
            let overridden = is_cur && cur.as_ref().is_some_and(|(_, m)| attrs.iter().any(|(k, v)| m.get(k) != Some(v)));
            let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 22.0), Sense::click());
            if is_cur || sel.as_deref() == Some(name.as_str()) {
                ui.painter().rect_filled(r, 0.0, if is_cur { t.row_selected } else { t.hover });
            } else if resp.hovered() {
                ui.painter().rect_filled(r, 0.0, t.hover);
            }
            // The built-in "[Normal … Style]" (listed first) is translated where painted; other names
            // are user data, brackets or not.
            let shown = super::label_or_name(name, *name == normal);
            let label = if overridden { format!("{shown}+") } else { shown.to_string() };
            ui.painter().text(r.left_center() + vec2(8.0, 0.0), egui::Align2::LEFT_CENTER, label, egui::FontId::proportional(12.5), t.text);
            if resp.clicked() {
                clicked = Some((name.clone(), ui.input(|i| i.modifiers.alt)));
            }
        }
    });
    let has_text = cur.is_some();
    if let Some((name, clear)) = clicked {
        sel = Some(name.clone());
        if has_text {
            let p = target(app, kind, json!({ "name": name, "clearOverrides": clear }));
            app.run(&format!("{}.apply", kind.prefix()), p).ok();
        }
    }
    set_pstate(ui.ctx(), kind.key(), sel.clone());
    let deletable = sel.as_ref().is_some_and(|s| *s != normal);
    widgets::bottom_bar(ui, |ui| {
        ui.add_space((ui.available_width() - 2.0 * 28.0).max(0.0));
        if widgets::icon_button_enabled(
            ui,
            "dc-new-item",
            &crate::i18n::fmt(tl!("Create New {kind} Style"), &[("kind", kind.what_label())]),
            false,
            true,
            24.0,
        )
        .clicked()
            && let Ok(r) = app.run(&format!("{}.new", kind.prefix()), target(app, kind, json!({})))
        {
            set_pstate(ui.ctx(), kind.key(), r["name"].as_str().map(str::to_string));
        }
        if widgets::icon_button_enabled(
            ui,
            "trash-2",
            &crate::i18n::fmt(tl!("Delete {kind} Style"), &[("kind", kind.what_label())]),
            false,
            deletable,
            24.0,
        )
        .clicked()
            && let Some(n) = &sel
            && app.run(&format!("{}.delete", kind.prefix()), json!({ "name": n })).is_ok()
        {
            set_pstate::<Option<String>>(ui.ctx(), kind.key(), None);
        }
    });
}

pub fn menu(app: &mut VectorcraftApp, ui: &mut Ui, kind: Kind) {
    let sel: Option<String> = pstate(ui.ctx(), kind.key());
    let normal = if kind == Kind::Char { "[Normal Character Style]" } else { "[Normal Paragraph Style]" };
    let user = sel.as_ref().is_some_and(|s| s != normal);
    let has_text = current(app, kind).is_some() || selection_len(app) > 0;
    let p = kind.prefix();
    if menu_item(ui, &crate::i18n::fmt(tl!("New {kind} Style…"), &[("kind", kind.what_label())]), true, false) {
        app.run(&format!("{p}.new"), target(app, kind, json!({}))).ok();
    }
    if menu_item(ui, &crate::i18n::fmt(tl!("Duplicate {kind} Style"), &[("kind", kind.what_label())]), sel.is_some(), false)
        && let Some(n) = &sel
    {
        app.run(&format!("{p}.duplicate"), json!({ "name": n })).ok();
    }
    if menu_item(ui, &crate::i18n::fmt(tl!("Delete {kind} Style"), &[("kind", kind.what_label())]), user, false)
        && let Some(n) = &sel
    {
        app.run(&format!("{p}.delete"), json!({ "name": n })).ok();
    }
    ui.separator();
    if menu_item(ui, tl!("Clear Overrides"), sel.is_some() && has_text, false)
        && let Some(n) = &sel
    {
        app.run(&format!("{p}.apply"), target(app, kind, json!({ "name": n, "clearOverrides": true }))).ok();
    }
    if menu_item(ui, &crate::i18n::fmt(tl!("Redefine {kind} Style"), &[("kind", kind.what_label())]), sel.is_some() && has_text, false)
        && let Some(n) = &sel
    {
        app.run(&format!("{p}.redefine"), target(app, kind, json!({ "name": n }))).ok();
    }
    if menu_item(ui, &crate::i18n::fmt(tl!("{kind} Style Options…"), &[("kind", kind.what_label())]), user, false)
        && let Some(n) = &sel
    {
        let label = format!("{} Style Options", kind.what());
        app.run("ui.paramDialog", json!({ "command": format!("{p}.rename"), "label": label, "params": { "name": n, "to": n } })).ok();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vectorcraft_doc::{NodeId, NodeKind};

    use super::super::tests_appearance::{click, frame_events, text_rect};

    #[test]
    fn panels_draw_and_apply_headless() {
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        app.session.execute("file.new", &json!({"width": 100, "height": 100})).unwrap();
        let id = app.session.execute("text.create", &json!({"x": 10, "y": 40, "text": "Hi"})).unwrap()["id"].clone();
        app.session.execute("select.set", &json!({"ids": [id]})).unwrap();
        app.session.execute("charStyle.new", &json!({"name": "Big", "attrs": {"size": 30.0}})).unwrap();
        for kind in [Kind::Char, Kind::Para] {
            let ctx = egui::Context::default();
            let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
                show(&mut app, ui, kind);
                menu(&mut app, ui, kind);
            });
            out.textures_delta.clear();
        }
        app.run("charStyle.apply", target(&app, Kind::Char, json!({"name": "Big"}))).unwrap();
        let (name, attrs) = current(&app, Kind::Char).unwrap();
        assert_eq!((name.as_deref(), attrs["size"].as_f64()), (Some("Big"), Some(30.0)));
    }

    fn capture_app() -> (VectorcraftApp, NodeId, egui::Context) {
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        app.run("file.new", json!({"width": 300, "height": 200})).unwrap();
        app.run("charStyle.new", json!({"name": "Untouched", "attrs": {"size": 22}})).unwrap();
        let other = app.run("text.create", json!({"x": 20, "y": 100, "text": "Other", "size": 22})).unwrap()["id"].clone();
        app.run("charStyle.apply", json!({"name": "Untouched", "id": other})).unwrap();
        let id = NodeId(app.run("text.create", json!({"x": 20, "y": 50, "text": "ABcdef", "size": 18})).unwrap()["id"].as_u64().unwrap());
        app.run("text.setRangeStyle", json!({"id": id.0, "start": 2, "end": 6, "size": 34})).unwrap();
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        assert_eq!(text_editing(&app), None);
        (app, id, ctx)
    }

    fn select_type_range(app: &mut VectorcraftApp, id: NodeId, start: usize, end: usize) {
        // The same editing handshake and selection API as character::tests::scaled_range_fields_follow_the_type_tool_selection.
        app.session.select_tool("type", Default::default()).unwrap();
        app.session.apply_actions(vec![vectorcraft_tools::Action::Notify("text.editNew".into())]).unwrap();
        app.session.set_tool_option("select", &json!({"start": start, "end": end}));
        super::super::character::end_typing(app);
        assert_eq!(app.session.tool_options()["typing"], json!(false));
        assert!(!app.session.in_interaction(), "settle typing before taking the history baseline");
        assert_eq!(text_editing(app), Some((id, start, end)));
    }

    /// Click production panel/menu content with the existing hover/press/release helper.
    /// A live range guard and command counter distinguish a dispatch failure from a missed click or lost Type selection.
    fn click_style_ui(ctx: &egui::Context, app: &mut VectorcraftApp, id: NodeId, range: Option<(usize, usize)>, is_menu: bool, label: &str) {
        let expected = range.map(|(start, end)| (id, start, end));
        let mut expected_target = json!({});
        if let Some((start, end)) = range {
            expected_target["id"] = json!(id.0);
            if end > start {
                expected_target["start"] = json!(start);
                expected_target["end"] = json!(end);
            }
        }
        let commands = app.run_count;
        let mut dispatched = false;
        let mut draw = |app: &mut VectorcraftApp, ui: &mut egui::Ui| {
            // On the release frame this is immediately before the production menu/row handles the click and builds its target.
            assert_eq!(text_editing(app), expected, "Type selection before {label}");
            assert_eq!(target(app, Kind::Char, json!({})), expected_target, "actual panel target before {label}");
            let before = app.run_count;
            if is_menu {
                menu(app, ui, Kind::Char);
            } else {
                show(app, ui, Kind::Char);
            }
            if app.run_count != before {
                assert!(!dispatched, "{label} dispatched more than once");
                assert_eq!(app.run_count, before.wrapping_add(1), "one command from {label}");
                dispatched = true;
            }
        };
        frame_events(ctx, app, vec![], &mut draw);
        let texts = frame_events(ctx, app, vec![], &mut draw);
        // widgets::menu_item prefixes an unchecked label with three spaces; the panel row does not.
        let painted = if is_menu { format!("   {label}") } else { label.to_string() };
        let pos = text_rect(&texts, &painted).center();
        click(ctx, app, pos, &mut draw);
        assert!(dispatched, "pointer click did not dispatch {label}; do not interpret this as parameter rejection");
        assert_eq!(app.run_count, commands.wrapping_add(1), "exactly one real panel command");
        assert_eq!(text_editing(app), expected, "Type selection after {label}");
    }

    fn text_runs(app: &VectorcraftApp, id: NodeId) -> Vec<(String, f64, Option<String>)> {
        let st = app.session.doc().unwrap();
        let NodeKind::Text(text) = &st.doc.node(id).unwrap().kind else { panic!("expected text") };
        text.runs.iter().map(|r| (r.text.clone(), r.style.size, r.style.style_name.clone())).collect()
    }

    fn capture_from_menu(range: Option<(usize, usize)>, redefine: bool) {
        let (mut app, id, ctx) = capture_app();
        let mut follower = None;
        let mut local_override = None;
        if redefine {
            app.run("charStyle.new", json!({"name": "Body", "attrs": {"size": 24}})).unwrap();
            // Select the definition through its real panel row; no test-only panel selection is installed.
            click_style_ui(&ctx, &mut app, id, None, false, "Body");
            let selected: Option<String> = pstate(&ctx, Kind::Char.key());
            assert_eq!(selected.as_deref(), Some("Body"));
            for (slot, text, y, size) in [(&mut follower, "Follower", 130, 24), (&mut local_override, "Override", 160, 48)] {
                let user = NodeId(app.run("text.create", json!({"x": 20, "y": y, "text": text, "size": 24})).unwrap()["id"].as_u64().unwrap());
                app.run("charStyle.apply", json!({"name": "Body", "id": user.0})).unwrap();
                app.run("text.setRangeStyle", json!({"id": user.0, "start": 0, "end": text.len(), "size": size})).unwrap();
                *slot = Some(user);
            }
            app.run("select.set", json!({"ids": [id.0]})).unwrap();
            app.run("text.setRangeStyle", json!({"id": id.0, "start": 0, "end": 6, "size": 18})).unwrap();
            app.run("text.setRangeStyle", json!({"id": id.0, "start": 2, "end": 6, "size": 34})).unwrap();
        }
        let link = redefine.then(|| "Body".to_string());
        assert_eq!(text_runs(&app, id), vec![("AB".into(), 18.0, link.clone()), ("cdef".into(), 34.0, link)]);
        if let Some((start, end)) = range {
            select_type_range(&mut app, id, start, end);
        }
        assert!(!app.session.in_interaction());
        app.session.doc_mut().unwrap().mark_saved();
        let before = app.session.doc().unwrap().doc.clone();
        let selection = app.session.doc().unwrap().selection.clone();
        let undo = app.session.doc().unwrap().history.undo.len();
        assert!(!app.session.doc().unwrap().is_dirty());
        let label = if redefine { "Redefine Character Style" } else { "New Character Style…" };
        click_style_ui(&ctx, &mut app, id, range, true, label);
        let after = app.session.doc().unwrap().doc.clone();
        let name = if redefine { "Body" } else { "Character Style 1" };
        let definition = after.char_styles.iter().find(|s| s.name == name);
        let size = definition.and_then(|s| s.attrs.get("size")).and_then(Value::as_f64);
        let expected_size = if range == Some((3, 5)) { 34.0 } else { 18.0 };
        let count = before.char_styles.len() + usize::from(!redefine);
        let history = &app.session.doc().unwrap().history;
        assert_eq!(
            (after.char_styles.len(), size, history.undo.len(), history.redo.len()),
            (count, Some(expected_size), undo + 1, 0),
            "{label} dispatched with live range {range:?}; capture must use the selected start and make one style edit"
        );
        assert!(!definition.unwrap().attrs.contains_key("style_name"));
        for old in &before.char_styles {
            if old.name != name {
                assert_eq!(after.char_styles.iter().find(|s| s.name == old.name), Some(old));
            }
        }
        // New captures without applying. Redefine changes its unmodified follower, preserving overrides and unrelated text.
        before.walk(|node| {
            if matches!(&node.kind, NodeKind::Text(_)) && Some(node.id) != follower {
                assert_eq!(after.node(node.id), Some(node));
            }
        });
        if let Some(user) = follower {
            assert_eq!(text_runs(&app, user), vec![("Follower".into(), expected_size, Some("Body".into()))]);
        }
        if let Some(user) = local_override {
            assert_eq!(text_runs(&app, user), vec![("Override".into(), 48.0, Some("Body".into()))]);
        }
        assert_eq!(app.session.doc().unwrap().selection, selection);
        assert!(app.session.doc().unwrap().is_dirty());
        app.run("edit.undo", json!({})).unwrap();
        assert_eq!(app.session.doc().unwrap().doc, before);
        assert!(!app.session.doc().unwrap().is_dirty());
        app.run("edit.redo", json!({})).unwrap();
        assert_eq!(app.session.doc().unwrap().doc, after);
        assert!(app.session.doc().unwrap().is_dirty());
    }

    // Desired ordinary behavior: debug main is source-predicted to fail these two assertions after a real menu dispatch.
    // Start 3 is inside the second run; style_at chooses the preceding run at an exact boundary.
    #[test]
    fn selected_type_range_new_character_style_from_menu() {
        capture_from_menu(Some((3, 5)), false);
    }

    #[test]
    fn selected_type_range_redefine_character_style_from_menu() {
        capture_from_menu(Some((3, 5)), true);
    }

    #[test]
    fn new_character_style_object_and_caret_controls() {
        // The current collapsed target omits start even at caret 3, so capture still uses object-start size 18.
        for range in [None, Some((3, 3))] {
            capture_from_menu(range, false);
        }
    }

    #[test]
    fn redefine_character_style_object_and_caret_controls() {
        for range in [None, Some((3, 3))] {
            capture_from_menu(range, true);
        }
    }

    #[test]
    fn character_style_range_apply_from_panel_control() {
        let (mut app, id, ctx) = capture_app();
        app.run("charStyle.new", json!({"name": "Body", "attrs": {"size": 26}})).unwrap();
        select_type_range(&mut app, id, 3, 5);
        let before = app.session.doc().unwrap().doc.clone();
        let undo = app.session.doc().unwrap().history.undo.len();
        click_style_ui(&ctx, &mut app, id, Some((3, 5)), false, "Body");
        let after = app.session.doc().unwrap().doc.clone();
        assert_eq!(
            text_runs(&app, id),
            vec![("AB".into(), 18.0, None), ("c".into(), 34.0, None), ("de".into(), 26.0, Some("Body".into())), ("f".into(), 34.0, None)]
        );
        assert_eq!(after.char_styles, before.char_styles, "Apply does not redefine Body");
        before.walk(|node| {
            if node.id != id && matches!(&node.kind, NodeKind::Text(_)) {
                assert_eq!(after.node(node.id), Some(node));
            }
        });
        assert_eq!((app.session.doc().unwrap().history.undo.len(), app.session.doc().unwrap().history.redo.len()), (undo + 1, 0));
        app.run("edit.undo", json!({})).unwrap();
        assert_eq!(app.session.doc().unwrap().doc, before);
        app.run("edit.redo", json!({})).unwrap();
        assert_eq!(app.session.doc().unwrap().doc, after);
    }

    // Pinned-main diagnosis, separate from the desired pointer regressions above. This is not a captured menu return value.
    #[test]
    #[cfg(debug_assertions)]
    fn selected_type_range_capture_debug_rejection_diagnostic() {
        for command in ["charStyle.new", "charStyle.redefine"] {
            let (mut app, id, _) = capture_app();
            app.run("charStyle.new", json!({"name": "Body", "attrs": {"size": 24}})).unwrap();
            app.run("charStyle.new", json!({"name": "Redo probe", "attrs": {"size": 42}})).unwrap();
            app.run("edit.undo", json!({})).unwrap();
            select_type_range(&mut app, id, 3, 5);
            app.session.doc_mut().unwrap().mark_saved();
            let before = app.session.doc().unwrap().clone();
            assert_eq!(before.history.redo.len(), 1);
            let params = target(&app, Kind::Char, if command == "charStyle.redefine" { json!({"name": "Body"}) } else { json!({}) });
            assert_eq!((params["id"].as_u64(), params["start"].as_u64(), params["end"].as_u64()), (Some(id.0), Some(3), Some(5)));
            let checker = crate::menus::check_params(command, &params).unwrap_err().to_string();
            let commands = app.run_count;
            assert_eq!(text_editing(&app), Some((id, 3, 5)));
            let rejected = app.run(command, params).unwrap_err().to_string();
            assert!(checker.contains("unknown parameter `end`"), "{command}: {checker}");
            assert_eq!(rejected, checker);
            assert_eq!(app.run_count, commands.wrapping_add(1));
            let after = app.session.doc().unwrap();
            assert_eq!((after.doc.clone(), &after.selection, after.is_dirty()), (before.doc.clone(), &before.selection, false));
            for (old, new) in [(&before.history.undo, &after.history.undo), (&before.history.redo, &after.history.redo)] {
                assert_eq!(old.len(), new.len());
                for (a, b) in old.iter().zip(new.iter()) {
                    assert_eq!((&a.label, &a.doc, &a.selection), (&b.label, &b.doc, &b.selection));
                }
            }
            assert_eq!(text_editing(&app), Some((id, 3, 5)));
        }
    }
}
