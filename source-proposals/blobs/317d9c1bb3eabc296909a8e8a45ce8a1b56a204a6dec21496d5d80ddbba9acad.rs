//! The real menu dialog, dropdown, Cancel and OK controls, driven headlessly.
use super::*;
use egui::{Event, Pos2, Rect, vec2};
use serde_json::json;
use vectorcraft_engine::Session;

fn frame(ctx: &egui::Context, app: &mut VectorcraftApp, events: Vec<Event>) -> Vec<(String, Rect)> {
    let mut out = ctx
        .run_ui(egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1000.0, 700.0))), events, ..Default::default() }, |ui| {
            show(app, ui.ctx())
        });
    out.textures_delta.clear();
    fn collect(s: &egui::Shape, out: &mut Vec<(String, Rect)>) {
        match s {
            egui::Shape::Text(t) => out.push((t.galley.text().to_string(), t.visual_bounding_rect())),
            egui::Shape::Vec(v) => v.iter().for_each(|s| collect(s, out)),
            _ => {}
        }
    }
    let mut text = vec![];
    for s in out.shapes {
        collect(&s.shape, &mut text);
    }
    text
}
fn at(text: &[(String, Rect)], label: &str) -> Pos2 {
    text.iter().find(|(t, _)| t == label).unwrap_or_else(|| panic!("missing {label}: {text:?}")).1.center()
}
fn click(ctx: &egui::Context, app: &mut VectorcraftApp, p: Pos2) -> Vec<(String, Rect)> {
    for pressed in [true, false] {
        frame(
            ctx,
            app,
            vec![
                Event::PointerMoved(p),
                Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() },
            ],
        );
    }
    frame(ctx, app, vec![])
}
#[test]
fn average_join_dialog_cancel_and_smooth_confirm_use_real_controls() {
    let ctx = egui::Context::default();
    crate::theme::install_fonts(&ctx);
    crate::theme::apply(&ctx, Default::default());
    let mut app = VectorcraftApp::new(Session::new(), Default::default());
    app.run("file.new", json!({"width":400,"height":300})).unwrap();
    let a = app.run("shape.line", json!({"x1":20,"y1":100,"x2":100,"y2":100})).unwrap()["id"].clone();
    let b = app.run("shape.line", json!({"x1":140,"y1":140,"x2":240,"y2":100})).unwrap()["id"].clone();
    app.run("select.anchorsMany", json!({"items":[{"id":a,"anchors":[[0,1]]},{"id":b,"anchors":[[0,0]]}]})).unwrap();
    app.session.doc_mut().unwrap().mark_saved();
    let before = app.session.doc().unwrap().doc.clone();
    let undo = app.session.doc().unwrap().history.undo.len();
    crate::menus::invoke(&mut app, "path.averageJoin", json!({}));
    assert_eq!(app.ui.dialog.as_ref().unwrap().kind, "averageJoin");
    for _ in 0..5 {
        frame(&ctx, &mut app, vec![]);
    }
    let text = frame(&ctx, &mut app, vec![]);
    let corner = at(&text, "Corner");
    let text = click(&ctx, &mut app, corner);
    let smooth = at(&text, "Smooth");
    let text = click(&ctx, &mut app, smooth);
    assert_eq!(app.ui.dialog.as_ref().unwrap().fields["kind"], json!("smooth"));
    let cancel = at(&text, "Cancel");
    click(&ctx, &mut app, cancel);
    assert!(app.ui.dialog.is_none());
    assert_eq!(app.session.doc().unwrap().doc, before);
    assert_eq!(app.session.doc().unwrap().history.undo.len(), undo);
    assert!(!app.session.doc().unwrap().is_dirty());
    crate::menus::invoke(&mut app, "path.averageJoin", json!({}));
    for _ in 0..5 {
        frame(&ctx, &mut app, vec![]);
    }
    let text = frame(&ctx, &mut app, vec![]);
    let corner = at(&text, "Corner");
    let text = click(&ctx, &mut app, corner);
    let smooth = at(&text, "Smooth");
    let text = click(&ctx, &mut app, smooth);
    let ok = at(&text, "OK");
    click(&ctx, &mut app, ok);
    assert!(app.ui.dialog.is_none());
    let id = vectorcraft_doc::NodeId(a.as_u64().unwrap());
    let p = app.session.doc().unwrap().doc.node(id).unwrap().path_data().unwrap();
    assert_eq!(p.subpaths[0].anchors.len(), 3);
    assert_eq!(p.subpaths[0].anchors[1].p, vectorcraft_geom::Point::new(120.0, 120.0));
    assert_eq!(p.subpaths[0].anchors[1].kind, vectorcraft_geom::AnchorKind::Smooth);
    assert_eq!(app.session.doc().unwrap().history.undo.len(), undo + 1);
    app.run("edit.undo", json!({})).unwrap();
    assert_eq!(app.session.doc().unwrap().doc, before);
}

#[test]
fn average_join_menu_disables_single_line_and_enables_two_lines() {
    let mut app = VectorcraftApp::new(Session::new(), Default::default());
    app.run("file.new", json!({"width":400,"height":300})).unwrap();
    let a = app.run("shape.line", json!({"x1":20,"y1":100,"x2":100,"y2":100})).unwrap()["id"].clone();
    app.run("select.anchors", json!({"id":a,"anchors":[[0,0],[0,1]],"mode":"set"})).unwrap();
    let id = vectorcraft_doc::NodeId(a.as_u64().unwrap());
    assert_eq!(app.session.doc().unwrap().selection.anchors[&id].len(), 2, "both endpoints explicitly selected through the command");
    let command = vectorcraft_engine::find_command("path.averageJoin").unwrap();
    assert!((command.enabled)(&app.session).is_err(), "averaging this line would leave only one anchor");
    assert!(!crate::menus::enabled(&app, "path.averageJoin"));
    app.session.doc_mut().unwrap().mark_saved();
    let before = app.session.doc().unwrap().doc.clone();
    let selection = app.session.doc().unwrap().selection.clone();
    let undo = app.session.doc().unwrap().history.undo.len();
    assert!(app.run("path.averageJoin", json!({})).is_err());
    assert_eq!(app.session.doc().unwrap().doc, before);
    assert_eq!(app.session.doc().unwrap().selection, selection);
    assert_eq!(app.session.doc().unwrap().history.undo.len(), undo);
    assert!(!app.session.doc().unwrap().is_dirty());
    let b = app.run("shape.line", json!({"x1":140,"y1":140,"x2":240,"y2":100})).unwrap()["id"].clone();
    app.run("select.anchorsMany", json!({"items":[{"id":a,"anchors":[[0,1]]},{"id":b,"anchors":[[0,0]]}]})).unwrap();
    assert!((command.enabled)(&app.session).is_ok(), "one endpoint from each separate two-point line is eligible");
    assert!(crate::menus::enabled(&app, "path.averageJoin"));
    app.run("path.averageJoin", json!({})).unwrap();
    assert_eq!(app.session.doc().unwrap().doc.node(id).unwrap().path_data().unwrap().subpaths[0].anchors.len(), 3);
}

/// Exercise the ordinary full-app shortcut route with an existing Redo branch.
fn average_join_shortcut_case(selection_kind: &str, eligible: bool) {
    let ctx = egui::Context::default();
    crate::theme::install_fonts(&ctx);
    crate::theme::apply(&ctx, Default::default());
    let mut app = VectorcraftApp::new(Session::new(), Default::default());
    app.run("file.new", json!({"width":400,"height":300})).unwrap();
    let a = app.run("shape.line", json!({"x1":20,"y1":100,"x2":100,"y2":100})).unwrap()["id"].clone();
    let b = app.run("shape.line", json!({"x1":140,"y1":140,"x2":240,"y2":100})).unwrap()["id"].clone();
    app.select_tool("directSelection");
    app.run("object.move", json!({"dx":5,"dy":0})).unwrap();
    app.run("edit.undo", json!({})).unwrap();
    match selection_kind {
        "empty" => app.run("select.none", json!({})).unwrap(),
        "single_line" => app.run("select.anchors", json!({"id":a,"anchors":[[0,0],[0,1]],"mode":"set"})).unwrap(),
        "two_lines" => app.run("select.anchorsMany", json!({"items":[{"id":a,"anchors":[[0,1]]},{"id":b,"anchors":[[0,0]]}]})).unwrap(),
        _ => panic!("unknown fixture"),
    };
    app.session.doc_mut().unwrap().mark_saved();
    let run = |app: &mut VectorcraftApp, mut events: Vec<Event>, modifiers: egui::Modifiers| {
        events.insert(0, Event::ModifiersChanged(modifiers));
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1600.0, 900.0))),
            time: Some(ctx.input(|i| i.time) + 0.1),
            events,
            ..Default::default()
        };
        let mut out = ctx.run_ui(input, |ui| {
            app.logic(ui.ctx());
            app.ui(ui);
        });
        out.textures_delta.clear();
    };
    for _ in 0..3 {
        run(&mut app, vec![], egui::Modifiers::NONE);
    }
    assert!(app.ui.dialog.is_none());
    assert_eq!(crate::menus::enabled(&app, "path.averageJoin"), eligible);
    let before = app.session.doc().unwrap().doc.clone();
    let selection = app.session.doc().unwrap().selection.clone();
    let history = |app: &VectorcraftApp| {
        let entries = |items: &[vectorcraft_engine::HistoryEntry]| {
            items.iter().map(|e| (e.label.clone(), e.doc.clone(), e.selection.clone())).collect::<Vec<_>>()
        };
        let h = &app.session.doc().unwrap().history;
        (entries(&h.undo), entries(&h.redo))
    };
    let before_history = history(&app);
    assert!(!before_history.1.is_empty(), "fixture retains a usable Redo branch");
    let modifiers = egui::Modifiers::COMMAND | egui::Modifiers::ALT | egui::Modifiers::SHIFT;
    for pressed in [true, false] {
        run(&mut app, vec![Event::Key { key: egui::Key::J, physical_key: None, pressed, repeat: false, modifiers }], modifiers);
    }
    assert_eq!(app.session.doc().unwrap().doc, before);
    assert_eq!(app.session.doc().unwrap().selection, selection);
    assert_eq!(history(&app), before_history);
    assert!(!app.session.doc().unwrap().is_dirty());
    assert!(!app.session.in_interaction());
    if eligible {
        let dialog = app.ui.dialog.as_ref().expect("eligible Average and Join shortcut must open the dialog");
        assert_eq!(dialog.kind, "averageJoin");
        assert_eq!(dialog.fields["kind"], json!("corner"));
        for pressed in [true, false] {
            run(
                &mut app,
                vec![Event::Key { key: egui::Key::Escape, physical_key: None, pressed, repeat: false, modifiers: egui::Modifiers::NONE }],
                egui::Modifiers::NONE,
            );
        }
    }
    assert!(app.ui.dialog.is_none(), "disabled Average and Join shortcut must not open a dialog: {selection_kind}");
    assert_eq!(app.session.doc().unwrap().doc, before);
    assert_eq!(app.session.doc().unwrap().selection, selection);
    assert_eq!(history(&app), before_history);
    assert!(!app.session.doc().unwrap().is_dirty());
}

#[test]
fn average_join_shortcut_rejects_empty_selection_without_editing() {
    average_join_shortcut_case("empty", false);
}

#[test]
fn average_join_shortcut_rejects_single_line_without_editing() {
    average_join_shortcut_case("single_line", false);
}

#[test]
fn average_join_shortcut_opens_for_two_lines_and_escape_keeps_state() {
    average_join_shortcut_case("two_lines", true);
}

#[test]
fn average_join_kind_keyboard_enter_opens_and_selects_without_accepting() {
    fn key(ctx: &egui::Context, app: &mut VectorcraftApp, key: egui::Key) {
        for pressed in [true, false] {
            frame(
                ctx,
                app,
                vec![
                    Event::ModifiersChanged(egui::Modifiers::NONE),
                    Event::Key { key, physical_key: None, pressed, repeat: false, modifiers: egui::Modifiers::NONE },
                ],
            );
        }
    }

    fn tab_to(ctx: &egui::Context, app: &mut VectorcraftApp, label: &str, typ: egui::WidgetType) -> egui::Id {
        fn collect(shape: &egui::Shape, clip: Rect, label: &str, out: &mut Vec<Rect>) {
            match shape {
                egui::Shape::Text(text) if text.galley.text() == label => {
                    let rect = text.visual_bounding_rect();
                    if clip.contains(rect.center()) {
                        out.push(rect);
                    }
                }
                egui::Shape::Vec(shapes) => shapes.iter().for_each(|shape| collect(shape, clip, label, out)),
                _ => {}
            }
        }
        let mut observed = vec![];
        for _ in 0..24 {
            let mut target = None;
            for pressed in [true, false] {
                let events = vec![
                    Event::ModifiersChanged(egui::Modifiers::NONE),
                    Event::Key { key: egui::Key::Tab, physical_key: None, pressed, repeat: false, modifiers: egui::Modifiers::NONE },
                ];
                let mut out = ctx.run_ui(
                    egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1000.0, 700.0))), events, ..Default::default() },
                    |ui| show(app, ui.ctx()),
                );
                out.textures_delta.clear();
                let gained: Vec<_> = out
                    .platform_output
                    .events
                    .iter()
                    .filter_map(|event| match event {
                        egui::output::OutputEvent::FocusGained(info) => Some(info),
                        _ => None,
                    })
                    .collect();
                observed.extend(gained.iter().map(|info| format!("{info:?}")));
                // An Area can contain the label on the same layer: geometry alone is not identity.
                let [info] = gained.as_slice() else { continue };
                let named = match typ {
                    egui::WidgetType::ComboBox => info.current_text_value.as_deref() == Some(label),
                    egui::WidgetType::Button => info.label.as_deref() == Some(label),
                    _ => false,
                };
                if !info.enabled || info.typ != typ || !named {
                    continue;
                }
                let id = ctx.memory(|memory| memory.focused()).expect("AVG_JOIN_KEYBOARD_FOCUS: semantic focus must remain live");
                let response = ctx.read_response(id).expect("AVG_JOIN_KEYBOARD_FOCUS: focused widget has a current response");
                let mut matches = vec![];
                for shape in &out.shapes {
                    collect(&shape.shape, shape.clip_rect, label, &mut matches);
                }
                assert_eq!(matches.len(), 1, "AVG_JOIN_KEYBOARD_FOCUS: one visible {label} value");
                let point = matches[0].center();
                assert!(
                    response.has_focus() && response.rect.contains(point) && response.interact_rect.contains(point),
                    "AVG_JOIN_KEYBOARD_FOCUS: semantic {label} target must own the focused response"
                );
                assert_eq!(
                    ctx.layer_id_at(point),
                    Some(response.layer_id),
                    "AVG_JOIN_KEYBOARD_FOCUS: semantic {label} target must be on the top visible layer"
                );
                target = Some((id, point, response.layer_id));
            }
            if let Some((id, point, layer)) = target {
                assert_eq!(ctx.memory(|memory| memory.focused()), Some(id), "AVG_JOIN_KEYBOARD_FOCUS: focus survives Tab release");
                let response = ctx.read_response(id).unwrap();
                assert!(response.has_focus() && response.rect.contains(point) && response.interact_rect.contains(point));
                assert_eq!(response.layer_id, layer);
                assert_eq!(ctx.layer_id_at(point), Some(layer));
                return id;
            }
        }
        panic!("AVG_JOIN_KEYBOARD_FOCUS: Tab did not reach {typ:?} {label}; observed {observed:?}");
    }

    let snapshot = |app: &VectorcraftApp| {
        let state = app.session.doc().unwrap();
        let entries = |items: &[vectorcraft_engine::HistoryEntry]| {
            items.iter().map(|entry| (entry.label.clone(), entry.doc.clone(), entry.selection.clone())).collect::<Vec<_>>()
        };
        (
            state.doc.clone(),
            state.selection.clone(),
            entries(&state.history.undo),
            entries(&state.history.redo),
            state.is_dirty(),
            app.session.in_interaction(),
        )
    };
    for accept in [false, true] {
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        crate::theme::apply(&ctx, Default::default());
        let mut app = VectorcraftApp::new(Session::new(), Default::default());
        app.run("file.new", json!({"width":400,"height":300})).unwrap();
        let a = app.run("shape.line", json!({"x1":20,"y1":100,"x2":100,"y2":100})).unwrap()["id"].clone();
        let b = app.run("shape.line", json!({"x1":140,"y1":140,"x2":240,"y2":100})).unwrap()["id"].clone();
        app.run("object.move", json!({"dx":5,"dy":0})).unwrap();
        let moved = app.session.doc().unwrap().doc.clone();
        app.run("edit.undo", json!({})).unwrap();
        app.run("select.anchorsMany", json!({"items":[{"id":a,"anchors":[[0,1]]},{"id":b,"anchors":[[0,0]]}]})).unwrap();
        app.session.doc_mut().unwrap().mark_saved();
        let before = snapshot(&app);
        assert_eq!(before.3.len(), 1, "fixture keeps an ordinary Redo branch");
        assert!(!before.4 && !before.5);
        assert!(crate::menus::enabled(&app, "path.averageJoin"));
        crate::menus::invoke(&mut app, "path.averageJoin", json!({}));
        for _ in 0..5 {
            frame(&ctx, &mut app, vec![]);
        }
        let combo = tab_to(&ctx, &mut app, "Corner", egui::WidgetType::ComboBox);
        assert!(!egui::ComboBox::is_open(&ctx, combo));
        assert_eq!(app.ui.dialog.as_ref().unwrap().str("kind"), "corner");
        assert_eq!(snapshot(&app), before, "focus traversal must not edit the selection");
        key(&ctx, &mut app, egui::Key::Enter);
        assert!(app.ui.dialog.is_some(), "AVG_JOIN_KIND_ENTER_OPEN: opening the choices must not accept Average and Join");
        assert!(egui::ComboBox::is_open(&ctx, combo), "AVG_JOIN_KIND_LIST_OPEN: Enter must open the focused combo");
        assert_eq!(app.ui.dialog.as_ref().unwrap().str("kind"), "corner");
        assert_eq!(snapshot(&app), before, "opening choices preserves document, selection, both history stacks and saved state");

        let option = tab_to(&ctx, &mut app, "Smooth", egui::WidgetType::Button);
        assert_ne!(option, combo, "focus must reach the option, not remain on the combo button");
        assert!(egui::ComboBox::is_open(&ctx, combo));
        assert_eq!(app.ui.dialog.as_ref().unwrap().str("kind"), "corner", "Tab alone must not choose an option");
        assert_eq!(snapshot(&app), before);
        key(&ctx, &mut app, egui::Key::Enter);
        assert!(app.ui.dialog.is_some(), "AVG_JOIN_KIND_ENTER_CHOICE: choosing Smooth must not accept Average and Join");
        // First prove that the focused option was activated; popup closure alone is insufficient.
        assert_eq!(app.ui.dialog.as_ref().unwrap().str("kind"), "smooth", "AVG_JOIN_KIND_CHOICE_APPLIED: Enter must choose Smooth");
        assert!(!egui::ComboBox::is_open(&ctx, combo), "AVG_JOIN_KIND_POPUP_CLOSED: choosing Smooth closes only the list");
        assert_eq!(snapshot(&app), before, "choosing a kind is not a document edit or preview");

        let text = frame(&ctx, &mut app, vec![]);
        let target = at(&text, if accept { "OK" } else { "Cancel" });
        click(&ctx, &mut app, target);
        assert!(app.ui.dialog.is_none() && !app.session.in_interaction());
        if accept {
            let joined = snapshot(&app);
            let id = vectorcraft_doc::NodeId(a.as_u64().unwrap());
            let path = joined.0.node(id).unwrap().path_data().unwrap();
            assert_eq!(path.subpaths[0].anchors.len(), 3);
            assert_eq!(path.subpaths[0].anchors[1].p, vectorcraft_geom::Point::new(120.0, 120.0));
            assert_eq!(path.subpaths[0].anchors[1].kind, vectorcraft_geom::AnchorKind::Smooth);
            assert_eq!(joined.2.len(), before.2.len() + 1);
            assert_eq!(&joined.2[..before.2.len()], before.2.as_slice());
            assert!(joined.3.is_empty() && joined.4);
            app.run("edit.undo", json!({})).unwrap();
            let undone = snapshot(&app);
            assert_eq!(undone.0, before.0);
            assert_eq!(undone.1, before.1);
            assert_eq!(undone.2, before.2);
            assert!(!undone.4 && !undone.5);
            app.run("edit.redo", json!({})).unwrap();
            assert_eq!(snapshot(&app), joined);
        } else {
            assert_eq!(snapshot(&app), before, "Cancel retains the untouched selection and Redo branch");
            app.run("edit.redo", json!({})).unwrap();
            assert_eq!(app.session.doc().unwrap().doc, moved, "the pre-dialog Redo remains usable");
        }
    }
}

fn average_join_keyboard_command_dialog_case(accept: bool) {
    fn frame(ctx: &egui::Context, app: &mut VectorcraftApp, events: Vec<Event>, modifiers: egui::Modifiers) -> egui::FullOutput {
        let mut events = events;
        events.insert(0, Event::ModifiersChanged(modifiers));
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1000.0, 700.0))),
            time: Some(ctx.input(|i| i.time) + 0.1),
            events,
            ..Default::default()
        };
        let mut out = ctx.run_ui(input, |ui| {
            app.logic(ui.ctx());
            app.ui(ui);
        });
        out.textures_delta.clear();
        out
    }

    fn key(ctx: &egui::Context, app: &mut VectorcraftApp, key: egui::Key, modifiers: egui::Modifiers) {
        for pressed in [true, false] {
            frame(ctx, app, vec![Event::Key { key, physical_key: None, pressed, repeat: false, modifiers }], modifiers);
        }
    }

    fn tab_to(ctx: &egui::Context, app: &mut VectorcraftApp, label: &str, typ: egui::WidgetType) -> egui::Id {
        fn collect(shape: &egui::Shape, clip: Rect, label: &str, out: &mut Vec<Rect>) {
            match shape {
                egui::Shape::Text(text) if text.galley.text() == label => {
                    let bounds = text.visual_bounding_rect();
                    if clip.contains_rect(bounds) {
                        out.push(bounds);
                    }
                }
                egui::Shape::Vec(shapes) => shapes.iter().for_each(|shape| collect(shape, clip, label, out)),
                _ => {}
            }
        }
        let mut observed = vec![];
        for _ in 0..24 {
            let mut target = None;
            for pressed in [true, false] {
                let out = frame(
                    ctx,
                    app,
                    vec![Event::Key { key: egui::Key::Tab, physical_key: None, pressed, repeat: false, modifiers: egui::Modifiers::NONE }],
                    egui::Modifiers::NONE,
                );
                let gained: Vec<_> = out.platform_output.events.iter().filter_map(|event| match event {
                    egui::output::OutputEvent::FocusGained(info) => Some(info),
                    _ => None,
                }).collect();
                observed.extend(gained.iter().map(|info| format!("{info:?}")));
                let [info] = gained.as_slice() else { continue };
                let named = match typ {
                    egui::WidgetType::ComboBox => info.current_text_value.as_deref() == Some(label),
                    egui::WidgetType::Button => info.label.as_deref() == Some(label),
                    _ => false,
                };
                if !info.enabled || info.typ != typ || !named {
                    continue;
                }
                let id = ctx.memory(|memory| memory.focused()).expect("AJ_ROUTE_FOCUS: semantic target has focus");
                let response = ctx.read_response(id).expect("AJ_ROUTE_FOCUS: current widget response");
                let mut text = vec![];
                for shape in &out.shapes {
                    collect(&shape.shape, shape.clip_rect, label, &mut text);
                }
                // Other app panels may use the same word: the focused response must own this label.
                let targets: Vec<_> = text.into_iter().filter(|bounds| {
                    ctx.content_rect().contains_rect(*bounds) && response.rect.contains_rect(*bounds)
                        && response.interact_rect.contains(bounds.center()) && ctx.layer_id_at(bounds.center()) == Some(response.layer_id)
                }).collect();
                assert_eq!(targets.len(), 1, "AJ_ROUTE_FOCUS: one fully visible {label} label in its own response");
                assert!(response.has_focus());
                target = Some((id, targets[0].center(), response.layer_id));
            }
            if let Some((id, point, layer)) = target {
                assert_eq!(ctx.memory(|memory| memory.focused()), Some(id), "AJ_ROUTE_FOCUS: focus survives Tab release");
                let response = ctx.read_response(id).unwrap();
                assert!(response.has_focus() && response.rect.contains(point) && response.interact_rect.contains(point));
                assert_eq!(response.layer_id, layer);
                assert_eq!(ctx.layer_id_at(point), Some(layer));
                return id;
            }
        }
        panic!("AJ_ROUTE_FOCUS: Tab did not reach {typ:?} {label}; observed {observed:?}");
    }

    let snapshot = |app: &VectorcraftApp| {
        let st = app.session.doc().unwrap();
        let entries = |items: &[vectorcraft_engine::HistoryEntry]| {
            items.iter().map(|e| (e.label.clone(), e.doc.clone(), e.selection.clone())).collect::<Vec<_>>()
        };
        (
            st.doc.clone(), st.selection.clone(), entries(&st.history.undo), entries(&st.history.redo),
            st.is_dirty(), app.session.in_interaction(),
        )
    };
    let ctx = egui::Context::default();
    crate::theme::install_fonts(&ctx);
    crate::theme::apply(&ctx, Default::default());
    let mut app = VectorcraftApp::new(Session::new(), Default::default());
    app.run("file.new", json!({"width":400,"height":300})).unwrap();
    app.select_tool("directSelection");
    let a = app.run("shape.line", json!({"x1":20,"y1":100,"x2":100,"y2":100})).unwrap()["id"].as_u64().unwrap();
    let b = app.run("shape.line", json!({"x1":140,"y1":140,"x2":240,"y2":100})).unwrap()["id"].as_u64().unwrap();
    app.run("object.move", json!({"dx":5,"dy":0})).unwrap();
    let moved = app.session.doc().unwrap().doc.clone();
    app.run("edit.undo", json!({})).unwrap();
    app.run("select.anchorsMany", json!({"items":[{"id":a,"anchors":[[0,1]]},{"id":b,"anchors":[[0,0]]}]})).unwrap();
    for _ in 0..3 {
        frame(&ctx, &mut app, vec![], egui::Modifiers::NONE);
    }
    app.session.doc_mut().unwrap().mark_saved();
    let before = snapshot(&app);
    assert_eq!(before.3.len(), 1, "the keyboard route starts with a usable Redo branch");
    assert!(!before.4 && !before.5 && app.ui.dialog.is_none());

    // From the command shortcut through the final button, only real key events drive the app.
    key(&ctx, &mut app, egui::Key::J, egui::Modifiers::COMMAND | egui::Modifiers::ALT | egui::Modifiers::SHIFT);
    let dialog = app.ui.dialog.as_ref().expect("AJ_ROUTE_SHORTCUT: eligible command opens its own dialog");
    assert_eq!(dialog.kind, "averageJoin");
    assert_eq!(dialog.str("kind"), "corner");
    assert_eq!(snapshot(&app), before);
    let combo = tab_to(&ctx, &mut app, "Corner", egui::WidgetType::ComboBox);
    key(&ctx, &mut app, egui::Key::Enter, egui::Modifiers::NONE);
    assert!(egui::ComboBox::is_open(&ctx, combo));
    assert_eq!(app.ui.dialog.as_ref().unwrap().str("kind"), "corner");
    let option = tab_to(&ctx, &mut app, "Smooth", egui::WidgetType::Button);
    assert_ne!(option, combo);
    key(&ctx, &mut app, egui::Key::Enter, egui::Modifiers::NONE);
    let dialog = app.ui.dialog.as_ref().expect("AJ_ROUTE_CHOICE: choosing Smooth must not accept the dialog");
    assert_eq!(dialog.kind, "averageJoin");
    assert_eq!(dialog.str("kind"), "smooth");
    assert!(!egui::ComboBox::is_open(&ctx, combo));
    assert_eq!(snapshot(&app), before, "command opening and kind choice preserve document and history");
    let button = tab_to(&ctx, &mut app, if accept { "OK" } else { "Cancel" }, egui::WidgetType::Button);
    assert_ne!(button, combo);
    assert_ne!(button, option);
    assert_eq!(snapshot(&app), before);
    key(&ctx, &mut app, egui::Key::Enter, egui::Modifiers::NONE);
    assert!(app.ui.dialog.is_none() && !app.session.in_interaction(), "AJ_ROUTE_FINISH: focused button closes the dialog");
    if accept {
        let after = snapshot(&app);
        let id = vectorcraft_doc::NodeId(a);
        let path = after.0.node(id).unwrap().path_data().unwrap();
        assert_eq!(path.subpaths[0].anchors.len(), 3);
        assert_eq!(path.subpaths[0].anchors[1].p, vectorcraft_geom::Point::new(120.0, 120.0));
        assert_eq!(path.subpaths[0].anchors[1].kind, vectorcraft_geom::AnchorKind::Smooth);
        assert!(after.0.node(vectorcraft_doc::NodeId(b)).is_none());
        assert_eq!(after.1.objects, vec![id]);
        assert_eq!(after.1.anchors[&id], [(0, 1)].into());
        assert_eq!(after.2.len(), before.2.len() + 1);
        assert_eq!(&after.2[..before.2.len()], before.2.as_slice());
        assert_eq!(after.2.last().unwrap(), &("Average and Join".into(), before.0.clone(), std::sync::Arc::new(before.1.clone())));
        assert!(after.3.is_empty() && after.4 && !after.5);
        app.run("edit.undo", json!({})).unwrap();
        let undone = snapshot(&app);
        assert_eq!(undone.0, before.0);
        assert_eq!(undone.1, before.1);
        assert_eq!(undone.2, before.2);
        assert_eq!(undone.3, vec![("Average and Join".into(), after.0.clone(), std::sync::Arc::new(after.1.clone()))]);
        assert!(!undone.4 && !undone.5);
        app.run("edit.redo", json!({})).unwrap();
        assert_eq!(snapshot(&app), after);
    } else {
        assert_eq!(snapshot(&app), before, "keyboard Cancel retains the full original document, selection, history and saved state");
        app.run("edit.redo", json!({})).unwrap();
        assert_eq!(app.session.doc().unwrap().doc, moved, "the original Redo branch remains usable after keyboard Cancel");
    }
}

#[test]
fn average_join_keyboard_command_smooth_ok_preserves_history() {
    average_join_keyboard_command_dialog_case(true);
}

#[test]
fn average_join_keyboard_command_smooth_cancel_preserves_history() {
    average_join_keyboard_command_dialog_case(false);
}
