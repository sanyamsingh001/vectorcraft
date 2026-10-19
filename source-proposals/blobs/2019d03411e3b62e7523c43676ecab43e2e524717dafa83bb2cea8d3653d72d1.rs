mod tests {

    /// Pending canvas typing must settle before the Layers column opens its own preview.
    #[test]
    fn pending_typing_full_app_visibility_drag_preserves_text_and_history() {
        let (mut app, [_, _, group, child, _, _]) = nested();
        for (id, name) in [(group, "Typing Group"), (child, "Typing Child")] {
            app.run("layer.setProps", json!({"ids": [id.0], "name": name})).unwrap();
        }
        let text_id = NodeId(app.run("text.create", json!({"x": 160, "y": 200, "text": "Seed", "size": 24})).unwrap()["id"].as_u64().unwrap());
        app.run("select.set", json!({"ids": [text_id.0]})).unwrap();
        app.run("window.panel", json!({"panel": "layers"})).unwrap();
        crate::dock::set_collapsed(&mut app, false);
        let ctx = egui::Context::default();
        let mut time = 0.0;
        let mut shell = |app: &mut VectorcraftApp, mut events: Vec<egui::Event>, threshold: bool| {
            time += 0.1;
            events.insert(0, egui::Event::ModifiersChanged(egui::Modifiers::NONE));
            let mut raw = egui::RawInput {
                events,
                time: Some(time),
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, vec2(1600.0, 850.0))),
                ..Default::default()
            };
            app.raw_input_hook(&mut raw);
            let mut observed = false;
            let mut out = ctx.run_ui(raw, |ui| {
                app.logic(ui.ctx());
                // Only the first layout pass: a successful first pass legitimately settles typing.
                if threshold && !observed {
                    assert!(ui.input(|i| i.pointer.primary_down()), "SETUP: threshold is held");
                    assert!(ui.ctx().memory(|m| m.focused().is_none()), "SETUP: no egui field");
                    assert!(!ui.ctx().egui_wants_keyboard_input(), "SETUP: Type tool owns text");
                    assert!(app.ui.dialog.is_none() && !app.ui.palette_open && app.ui.flyout.is_none());
                    assert!(!app.ui.tools_drawer && !app.session.tool_busy() && !app.session.tool_composing());
                    assert_eq!(crate::scrub::phase(ui.ctx()), crate::scrub::Phase::Idle);
                    assert_eq!(app.session.tool_id(), "type");
                    assert!(app.session.tool_wants_text());
                    assert_eq!(app.session.tool_options()["typing"], json!(true));
                    let interaction = app.session.doc().unwrap().interaction.as_ref().expect("SETUP: pending Typing before Layers UI");
                    assert_eq!(interaction.label, "Typing", "SETUP: earlier settlement is inconclusive");
                    let preview = interaction.preview.as_ref().expect("SETUP: real typing preview");
                    assert_eq!(preview.0, "text.editRange");
                    assert_eq!(preview.1["id"], json!(text_id.0));
                    eprintln!("LAYERS_PENDING_TYPING_THRESHOLD_BEFORE_UI");
                    observed = true;
                }
                app.ui(ui);
            });
            assert!(!app.ui.status.starts_with("Internal error:"), "SETUP: shell reported {}", app.ui.status);
            assert!(!threshold || observed, "SETUP: threshold frame reached the full shell");
            out.textures_delta.clear();
            out
        };
        for _ in 0..3 {
            shell(&mut app, vec![], false);
        }
        let history = |st: &vectorcraft_engine::DocState| {
            let entries = |items: &[vectorcraft_engine::HistoryEntry]| {
                items.iter().map(|e| (e.label.clone(), e.doc.clone(), e.selection.clone())).collect::<Vec<_>>()
            };
            (entries(&st.history.undo), entries(&st.history.redo))
        };
        app.session.active_mut().unwrap().mark_saved();
        let before = app.session.doc().unwrap().clone();
        let h0 = history(&before);
        let s0 = before.selection.clone();
        assert_eq!(s0.objects, vec![text_id]);
        assert!(h0.1.is_empty() && !before.is_dirty());
        assert!(before.interaction.is_none() && before.undo_group.is_none());
        assert!(!before.doc.ancestry(text_id).unwrap().contains(&group));
        assert!(before.doc.node(group).unwrap().visible && before.doc.node(child).unwrap().visible);
        let text = |doc: &vectorcraft_doc::Document| match &doc.node(text_id).unwrap().kind {
            NodeKind::Text(t) => t.plain_text(),
            _ => panic!("SETUP: public text.create made a text object"),
        };
        assert_eq!(text(&before.doc), "Seed");

        // Enter the existing text through the actual canvas, with no tool action/preview seeding.
        app.select_tool("type");
        shell(&mut app, vec![], false);
        let bounds = app.session.doc().unwrap().doc.node(text_id).unwrap().geometric_bounds().unwrap();
        let canvas = app.canvas_rect.expect("SETUP: full shell laid out the canvas");
        let at = crate::canvas::Xf::new(canvas, app.view().unwrap()).to_screen(bounds.center());
        assert!(canvas.contains(at));
        for events in [vec![egui::Event::PointerMoved(at)], vec![button(at, true)], vec![button(at, false)], vec![]] {
            shell(&mut app, events, false);
        }
        assert_eq!(app.session.tool_id(), "type");
        assert!(app.session.tool_wants_text() && !app.session.tool_busy());
        assert!(!ctx.input(|i| i.pointer.primary_down()));
        let options = app.session.tool_options();
        assert_eq!(options["editing"], json!(text_id.0), "SETUP: canvas click entered the intended text");
        let caret = options["caret"].as_u64().unwrap() as usize;
        assert_eq!(options["start"], json!(caret));
        assert_eq!(options["end"], json!(caret));
        assert!(caret <= 4);
        assert_eq!(app.session.doc().unwrap().doc, before.doc, "SETUP: text entry preserves D0");
        assert_eq!(app.session.doc().unwrap().selection, s0, "SETUP: text entry preserves S0");
        assert_eq!(history(app.session.doc().unwrap()), h0);
        assert!(!app.session.in_interaction());
        let mut expected_text = "Seed".to_string();
        expected_text.insert(caret, 'X');
        shell(&mut app, vec![egui::Event::Text("X".into())], false);
        let out = shell(&mut app, vec![], false);
        let typed = app.session.doc().unwrap().clone();
        assert_eq!(text(&typed.doc), expected_text, "SETUP: raw text reached the Type tool");
        assert_eq!(app.session.tool_options()["caret"], json!(caret + 1));
        assert_eq!(app.session.tool_options()["typing"], json!(true));
        assert_eq!(typed.selection, s0);
        assert_eq!(history(&typed), h0, "SETUP: typing is still uncommitted after a full idle frame");
        assert!(typed.is_dirty());
        let typing = typed.interaction.as_ref().expect("SETUP: real Typing remains pending");
        assert_eq!(typing.label, "Typing");
        assert_eq!(typing.doc, before.doc);
        assert_eq!(*typing.selection, s0);
        assert_eq!(typing.preview.as_ref().unwrap().0, "text.editRange");
        before.doc.walk(|n| {
            // Ancestors contain the edited text; compare unrelated leaf art independently.
            if n.id != text_id && n.children().is_none() {
                assert_eq!(typed.doc.node(n.id), Some(n));
            }
        });
        eprintln!("LAYERS_PENDING_TYPING_TEXT_READY");

        // Reuse the existing widget inventory (toolbar::tests::widget_rects), retaining IDs.
        // Painted custom names bind each row; live widget rects locate its eye and lock columns.
        let canvas = app.canvas_rect.unwrap();
        let eye = |name: &str| {
            let labels: Vec<_> = out
                .shapes
                .iter()
                .filter_map(|c| match &c.shape {
                    egui::Shape::Text(t) if t.galley.job.text == name && t.pos.x > canvas.right() => Some(t.pos.y + t.galley.size().y * 0.5),
                    _ => None,
                })
                .collect();
            assert_eq!(labels.len(), 1, "SETUP: one visible dock row named {name}");
            let y = labels[0];
            let mut columns: Vec<_> = ctx.viewport(|vp| {
                vp.prev_pass
                    .widgets
                    .layers()
                    .flat_map(|(_, widgets)| widgets.iter())
                    .filter(|w| {
                        w.rect.size() == vec2(COLUMN, app.ui.layers_panel.row())
                            && w.rect.left() > canvas.right()
                            && w.rect.top() < y
                            && y < w.rect.bottom()
                    })
                    .map(|w| (w.id, w.rect))
                    .collect()
            });
            columns.sort_by(|a, b| a.1.left().total_cmp(&b.1.left()));
            assert_eq!(columns.len(), 2, "SETUP: actual eye and lock widgets for {name}");
            assert_eq!(columns[0].1.right(), columns[1].1.left());
            let (id, rect) = columns[0];
            let response = ctx.read_response(id).expect("SETUP: eye widget response");
            assert!(response.enabled());
            assert_eq!(response.rect, rect);
            assert_eq!(ctx.layer_id_at(rect.center()), Some(response.layer_id), "SETUP: eye is not covered");
            (id, rect)
        };
        let (eye_id, first) = eye("Typing Group");
        let (_, second) = eye("Typing Child");
        assert_eq!(first.bottom(), second.top(), "SETUP: group and child are adjacent visible rows");
        assert!(!canvas.contains(first.center()) && !canvas.contains(second.center()));
        assert!(first.contains(first.center() + vec2(0.0, 6.0)), "SETUP: threshold move stays in first eye");
        eprintln!("LAYERS_PENDING_TYPING_EYE_WIDGETS_READY");

        shell(&mut app, vec![egui::Event::PointerMoved(first.center())], false);
        shell(&mut app, vec![button(first.center(), true)], false);
        shell(&mut app, vec![egui::Event::PointerMoved(first.center() + vec2(0.0, 6.0))], true);
        assert!(ctx.is_being_dragged(eye_id), "SETUP: actual group eye owns pointer capture");
        let response = ctx.read_response(eye_id).expect("SETUP: captured eye response");
        assert_eq!(response.rect, first);
        assert!(response.dragged(), "SETUP: the eye response is a real drag, not a click");
        eprintln!("LAYERS_PENDING_TYPING_EYE_CAPTURED");
        shell(&mut app, vec![egui::Event::PointerMoved(second.center())], false);
        assert!(ctx.is_being_dragged(eye_id), "SETUP: eye owns the held move into the child row");
        let held = app.session.doc().unwrap().clone();
        shell(&mut app, vec![button(second.center(), false)], false);
        shell(&mut app, vec![], false);
        assert!(!ctx.input(|i| i.pointer.primary_down()));
        eprintln!("LAYERS_PENDING_TYPING_RELEASED");

        // The expected document is independent of the preview/command under test.
        let mut expected = typed.doc.clone();
        for id in [group, child] {
            std::sync::Arc::make_mut(&mut expected).node_mut(id).unwrap().visible = false;
        }
        let after = app.session.doc().unwrap().clone();
        eprintln!("LAYERS_PENDING_TYPING_LATE_ORACLES");
        assert_eq!(after.doc, expected, "LAYERS_PENDING_TYPING_VISIBILITY_DOCUMENT");
        assert_eq!(text(&after.doc), expected_text);
        assert_eq!(after.doc.node(text_id), typed.doc.node(text_id), "typed runs and all text attributes remain exact");
        assert_eq!(after.selection, s0);
        assert_eq!(after.uid, before.uid);
        assert_eq!(after.active_layer, before.active_layer);
        assert!(after.is_dirty() && after.interaction.is_none() && after.undo_group.is_none());
        assert_eq!(app.session.tool_options()["typing"], json!(false));
        let selection = std::sync::Arc::new(s0.clone());
        let typing_entry = ("Typing".to_string(), before.doc.clone(), selection.clone());
        let visibility_entry = ("Visibility".to_string(), typed.doc.clone(), selection.clone());
        let mut h_typed = h0.clone();
        h_typed.0.push(typing_entry);
        let mut h_after = h_typed.clone();
        h_after.0.push(visibility_entry);
        assert_eq!(history(&after), h_after, "typing and visibility are separate logical steps");
        assert_eq!(history(&held), h_typed, "only typing is committed while the column is held");
        let live = held.interaction.as_ref().unwrap();
        assert_eq!(live.label, "Visibility");
        assert_eq!(live.doc, typed.doc);
        assert_eq!(*live.selection, s0);
        assert_eq!(live.preview, Some(("layer.setProps".into(), json!({"ids": [group.0, child.0], "visible": false}))));
        assert_eq!(held.doc, expected);

        let mut h_undo_visibility = h_typed.clone();
        h_undo_visibility.1.push(("Visibility".into(), expected.clone(), selection.clone()));
        let mut h_undo_typing = h0.clone();
        h_undo_typing.1 = h_undo_visibility.1.clone();
        h_undo_typing.1.push(("Typing".into(), typed.doc.clone(), selection));
        for (command, label, doc, hist, dirty) in [
            ("edit.undo", "Visibility", &typed.doc, &h_undo_visibility, true),
            ("edit.undo", "Typing", &before.doc, &h_undo_typing, false),
            ("edit.redo", "Typing", &typed.doc, &h_undo_visibility, true),
            ("edit.redo", "Visibility", &expected, &h_after, true),
        ] {
            let result = app.run(command, json!({})).unwrap();
            assert_eq!(result[if command == "edit.undo" { "undone" } else { "redone" }], json!(label));
            let st = app.session.doc().unwrap();
            assert_eq!(&st.doc, doc, "{command} {label}: complete document");
            assert_eq!(st.selection, s0, "{command} {label}: complete selection");
            assert_eq!(st.uid, before.uid);
            assert_eq!(&history(st), hist, "{command} {label}: both logical history stacks");
            assert_eq!(st.is_dirty(), dirty);
            assert!(st.interaction.is_none() && st.undo_group.is_none());
        }
        eprintln!("LAYERS_PENDING_TYPING_HISTORY_VERIFIED");
    }
}
