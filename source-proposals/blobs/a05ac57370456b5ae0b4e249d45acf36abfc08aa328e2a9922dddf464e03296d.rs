
    /// Replacing a Color Picker within one control drain still starts a fresh dialog lifecycle.
    #[test]
    fn same_frame_control_reopen_refreshes_document_mode_focus() {
        fn shell(
            ctx: &egui::Context,
            app: &mut VectorcraftApp,
            time: &mut f64,
            before_ui: &mut dyn FnMut(&VectorcraftApp),
        ) -> Vec<(String, Rect)> {
            *time += 0.1;
            let mut raw = egui::RawInput {
                time: Some(*time),
                screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(1600.0, 900.0))),
                focused: true,
                ..Default::default()
            };
            // No text, pointer, key or focus request is injected into these frames.
            app.raw_input_hook(&mut raw);
            assert!(raw.events.is_empty(), "SETUP: the control commands must not inject input");
            let mut observed = false;
            let mut out = ctx.run_ui(raw, |ui| {
                app.logic(ui.ctx());
                if !observed {
                    before_ui(app);
                    observed = true;
                }
                app.ui(ui);
            });
            assert!(observed, "SETUP: full app logic reached the before-UI observation");
            assert!(!app.ui.status.starts_with("Internal error:"), "SETUP: full shell reported {}", app.ui.status);
            out.textures_delta.clear();
            fn collect(shape: &egui::Shape, texts: &mut Vec<(String, Rect)>) {
                match shape {
                    egui::Shape::Text(t) => texts.push((t.galley.text().to_string(), t.visual_bounding_rect())),
                    egui::Shape::Vec(shapes) => shapes.iter().for_each(|shape| collect(shape, texts)),
                    _ => {}
                }
            }
            let mut texts = vec![];
            out.shapes.iter().for_each(|shape| collect(&shape.shape, &mut texts));
            texts
        }

        let (tx, rx) = std::sync::mpsc::channel();
        let mut app = app().with_control(rx);
        app.run("file.documentColorMode", json!({"mode": "rgb"})).unwrap();
        app.run("paint.setFill", json!({"color": "#336699"})).unwrap();
        let ctx = egui::Context::default();
        let mut time = 0.0;
        for _ in 0..3 {
            shell(&ctx, &mut app, &mut time, &mut |_| {});
        }
        let before = app.session.doc().unwrap().clone();
        let history_counts = |st: &vectorcraft_engine::DocState| (st.history.undo.len(), st.history.redo.len());
        let enqueue = |method: &str, params: Value| {
            let (request, reply) = crate::control::ControlRequest::new(method, params);
            tx.send(request).expect("SETUP: live in-process control receiver");
            reply
        };
        let selected_range = |id| {
            let range = egui::TextEdit::load_state(&ctx, id)?.cursor.char_range()?.as_sorted_char_range();
            Some((range.start.0, range.end.0))
        };

        let open = enqueue("engine.execute", json!({"command": "ui.colorPicker", "params": {"stroke": false}}));
        shell(&ctx, &mut app, &mut time, &mut |_| {
            assert_eq!(open.try_recv().expect("SETUP: initial open drained")["ok"], true, "SETUP: initial open accepted");
        });
        shell(&ctx, &mut app, &mut time, &mut |_| {});
        let texts = shell(&ctx, &mut app, &mut time, &mut |_| {});
        assert_eq!(focused_field(&ctx, &texts).map(|(label, _)| label), Some("R".into()), "SETUP: RGB opening gives R focus");
        let r_id = ctx.memory(|m| m.focused()).expect("SETUP: R field has focus");
        let picker_layer = ctx.read_response(r_id).expect("SETUP: actual R widget").layer_id;
        let r_text = ctx.data(|d| d.get_temp::<String>(r_id)).expect("SETUP: actual R buffer");
        assert!(!r_text.is_empty(), "SETUP: R has a displayed value");
        assert_eq!(ctx.data(|d| d.get_temp::<String>(r_id.with("shown"))), Some(r_text.clone()), "SETUP: no pending field edit");
        assert_eq!(selected_range(r_id), Some((0, r_text.chars().count())), "SETUP: RGB opening selects all of R");
        let initial = app.ui.dialog.as_ref().expect("SETUP: RGB picker exists");
        assert_eq!(initial.kind, KIND);
        let first_picker = initial.fields[crate::screen_color::PICKER_ID].as_u64().expect("SETUP: initial picker identity");
        assert_eq!(picked(initial).color, Color::rgb8(51, 102, 153), "SETUP: no edited picker color");
        let st = app.session.doc().unwrap();
        assert_eq!(st.doc.color_mode, ColorMode::Rgb);
        assert_eq!(st.doc, before.doc, "SETUP: opening leaves the full document unchanged");
        assert_eq!(st.selection, before.selection);
        assert_eq!(history_counts(st), history_counts(&before));
        assert!(st.interaction.is_none() && st.undo_group.is_none(), "SETUP: no pending preview or undo group");
        eprintln!("CP_REOPEN_RGB_FOCUS_READY");

        // All three real requests are queued before the next full app frame. No None-dialog
        // draw, direct dialog reset, field edit or focus-memory write separates the openings.
        let replies = [
            enqueue("ui.dialog.cancel", json!({})),
            enqueue("engine.execute", json!({"command": "file.documentColorMode", "params": {"mode": "cmyk"}})),
            enqueue("engine.execute", json!({"command": "ui.colorPicker", "params": {"stroke": false}})),
        ];
        let mut replacement = None;
        let mut after_logic = None;
        shell(&ctx, &mut app, &mut time, &mut |app| {
            for (index, reply) in replies.iter().enumerate() {
                let answer = reply.try_recv().expect("SETUP: all three requests drain before UI");
                assert_eq!(answer["ok"], true, "SETUP: queued request {index} accepted: {answer}");
            }
            eprintln!("CP_REOPEN_CONTROL_DRAIN_ACCEPTED");
            let dialog = app.ui.dialog.as_ref().expect("SETUP: replacement exists before UI");
            assert_eq!(dialog.kind, KIND, "SETUP: replacement is a Color Picker");
            let id = dialog.fields[crate::screen_color::PICKER_ID].as_u64().expect("SETUP: replacement identity");
            assert_ne!(id, first_picker, "SETUP: reopen created a distinct picker");
            let st = app.session.doc().unwrap();
            assert_eq!(st.doc.color_mode, ColorMode::Cmyk, "SETUP: mode changed before replacement draws");
            assert_eq!(st.uid, before.uid, "SETUP: same active document");
            assert_eq!(st.selection, before.selection);
            assert!(st.interaction.is_none() && st.undo_group.is_none(), "SETUP: no preview before replacement draws");
            replacement = Some(id);
            after_logic = Some(st.clone());
            eprintln!("CP_REOPEN_REPLACEMENT_BEFORE_UI");
        });
        shell(&ctx, &mut app, &mut time, &mut |_| {});
        let texts = shell(&ctx, &mut app, &mut time, &mut |_| {});
        let dialog = app.ui.dialog.as_ref().expect("SETUP: replacement survived its opening frames");
        assert_eq!(dialog.kind, KIND);
        assert_eq!(dialog.fields[crate::screen_color::PICKER_ID].as_u64(), replacement);
        let expected = after_logic.expect("SETUP: captured post-drain model before UI");
        let st = app.session.doc().unwrap();
        assert_eq!(st.uid, expected.uid);
        assert_eq!(st.doc, expected.doc, "SETUP: idle replacement frames do not edit the full document");
        assert_eq!(st.selection, expected.selection);
        assert_eq!(history_counts(st), history_counts(&expected));
        assert!(st.interaction.is_none() && st.undo_group.is_none(), "SETUP: still no pending preview");

        // Locate C independently of current focus, using the actual dialog layer's TextEdit
        // widgets and their painted row labels, as focused_field does for the retained tests.
        let ids: Vec<_> = ctx.viewport(|vp| vp.prev_pass.widgets.layers().flat_map(|(_, widgets)| widgets.iter()).map(|w| w.id).collect());
        let mut cyan = vec![];
        let labels = ["H", "S", "B", "R", "G", "#", "L", "a", "b", "C", "M", "Y", "K"];
        for id in ids {
            let Some(response) = ctx.read_response(id) else { continue };
            if response.layer_id != picker_layer || egui::TextEdit::load_state(&ctx, id).is_none() {
                continue;
            }
            let label = texts
                .iter()
                .filter(|(text, rect)| {
                    labels.contains(&text.as_str()) && rect.max.x <= response.rect.min.x && response.rect.y_range().contains(rect.center().y)
                })
                .max_by(|a, b| a.1.max.x.total_cmp(&b.1.max.x));
            if label.is_some_and(|(text, _)| text == "C") && !cyan.contains(&id) {
                cyan.push(id);
            }
        }
        assert_eq!(cyan.len(), 1, "SETUP: uniquely locate actual C widget in the picker layer");
        let c_id = cyan[0];
        assert_ne!(c_id, r_id, "SETUP: C and R are distinct widgets");
        let c_text = ctx.data(|d| d.get_temp::<String>(c_id)).expect("SETUP: C displayed buffer");
        assert!(!c_text.is_empty(), "SETUP: C has a displayed value");
        assert_eq!(ctx.data(|d| d.get_temp::<String>(c_id.with("shown"))), Some(c_text.clone()), "SETUP: no pending C edit");
        eprintln!("CP_REOPEN_LATE_READY");
        assert_eq!(ctx.memory(|m| m.focused()), Some(c_id), "CP_REOPEN_FRESH_C_FOCUS: replacement must use the new document mode");
        eprintln!("CP_REOPEN_C_FOCUS_VERIFIED");
        assert_eq!(selected_range(c_id), Some((0, c_text.chars().count())), "CP_REOPEN_FRESH_C_SELECT_ALL: replacement selects the complete value");
        eprintln!("CP_REOPEN_FOCUS_AND_SELECTION_VERIFIED");
    }
