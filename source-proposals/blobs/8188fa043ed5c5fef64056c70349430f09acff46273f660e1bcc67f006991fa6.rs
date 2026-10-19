    /// A persistent context drives the real dialog, including its opening-click deferral and
    /// keyboard handling. No test requests a particular field's focus.
    fn focus_frame(ctx: &egui::Context, app: &mut VectorcraftApp, events: Vec<egui::Event>) {
        let screen_rect = Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(1280.0, 900.0)));
        let mut out = ctx.run_ui(egui::RawInput { events, screen_rect, focused: true, ..Default::default() }, |ui| {
            crate::shortcuts::handle(app, ui.ctx());
            super::super::show(app, ui.ctx());
        });
        out.textures_delta.clear();
    }

    fn focus_key(key: egui::Key) -> Vec<egui::Event> {
        [true, false]
            .into_iter()
            .map(|pressed| egui::Event::Key { key, physical_key: None, pressed, repeat: false, modifiers: egui::Modifiers::NONE })
            .collect()
    }

    fn focus_colors(mode: ColorMode) -> (Color, Color) {
        match mode {
            ColorMode::Rgb => (Color::rgb8(51, 102, 153), Color::rgb8(40, 102, 153)),
            ColorMode::Cmyk => (Color::cmyk(0.25, 0.2, 0.1, 0.1), Color::cmyk(0.4, 0.2, 0.1, 0.1)),
        }
    }

    /// #1692's opening-focus request is separate from Tab order and stale hex buffering.
    #[test]
    fn initial_focus_uses_document_color_mode() {
        let rgb = Color::rgb8(51, 102, 153);
        let cmyk = Color::cmyk(0.25, 0.2, 0.1, 0.1);
        // The document mode chooses the first field even when the supplied paint uses the other model.
        for (mode, original) in [(ColorMode::Rgb, rgb), (ColorMode::Cmyk, cmyk), (ColorMode::Rgb, cmyk), (ColorMode::Cmyk, rgb)] {
            let mut app = app();
            app.run("file.documentColorMode", json!({"mode": mode})).unwrap();
            app.run("paint.setFill", json!({"color": color_json(&original)})).unwrap();
            let before = app.session.active().unwrap().doc.clone();
            let undo = app.session.active().unwrap().history.undo.len();
            let ctx = egui::Context::default();
            crate::theme::install_fonts(&ctx);
            app.run("ui.colorPicker", json!({"stroke": false})).unwrap();
            // paint.setFill may convert the supplied colour to the document model before opening.
            // Preserve the components displayed on opening, not a new conversion of the input.
            let opened = picked(app.ui.dialog.as_ref().unwrap()).color;
            let expected = match mode {
                ColorMode::Rgb => {
                    let [_, g, b] = opened.to_rgb();
                    Color::rgb(40.0 / 255.0, g, b)
                }
                ColorMode::Cmyk => {
                    let [_, m, y, k] = opened.to_cmyk_managed(cms::active().settings().intent);
                    Color::cmyk(0.4, m, y, k)
                }
            };
            for _ in 0..3 {
                focus_frame(&ctx, &mut app, vec![]);
            }
            assert_eq!(picked(app.ui.dialog.as_ref().unwrap()).color, opened, "opening focus leaves the colour unchanged");
            let initial = ctx.memory(|m| m.focused()).unwrap();
            // No Select All: opening focus must select the complete value for replacement.
            focus_frame(&ctx, &mut app, vec![egui::Event::Text("40".into())]);
            focus_frame(&ctx, &mut app, focus_key(egui::Key::Tab));
            focus_frame(&ctx, &mut app, vec![]);
            assert_eq!(picked(app.ui.dialog.as_ref().unwrap()).color, expected, "{mode:?}: typing must edit R or C, not H");
            assert_eq!(crate::panels::current_paints(&app).0.color(), Some(expected), "live preview");
            assert_eq!(app.ui.dialog.as_ref().unwrap().str("channel"), "hue", "focus does not select a channel radio");
            let after_tab = ctx.memory(|m| m.focused());
            assert!(after_tab.is_some() && after_tab != Some(initial), "Tab still leaves the initial field");
            for _ in 0..2 {
                focus_frame(&ctx, &mut app, vec![]);
                assert_eq!(ctx.memory(|m| m.focused()), after_tab, "initial focus is not reclaimed");
            }
            assert_eq!(app.session.active().unwrap().history.undo.len(), undo, "preview adds no history step");
            focus_frame(&ctx, &mut app, focus_key(egui::Key::Escape));
            assert!(app.ui.dialog.is_none());
            assert_eq!(app.session.active().unwrap().doc, before, "Cancel restores the document");
            assert_eq!(app.session.active().unwrap().history.undo.len(), undo);
            assert!(!app.session.in_interaction());
        }
    }

    #[test]
    fn initial_focus_after_click_open_tracks_document_mode() {
        let mut app = app();
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        // Reopen the same dialog/context, including a change of document mode between openings.
        for mode in [ColorMode::Cmyk, ColorMode::Rgb, ColorMode::Cmyk] {
            app.run("file.documentColorMode", json!({"mode": mode})).unwrap();
            let (original, expected) = focus_colors(mode);
            app.run("paint.setFill", json!({"color": color_json(&original)})).unwrap();
            let undo = app.session.active().unwrap().history.undo.len();
            app.run("ui.colorPicker", json!({"stroke": false})).unwrap();
            let pos = egui::pos2(5.0, 5.0);
            let button =
                |pressed| egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE };
            focus_frame(&ctx, &mut app, vec![egui::Event::PointerMoved(pos), button(true), button(false)]);
            for _ in 0..3 {
                focus_frame(&ctx, &mut app, vec![]);
            }
            focus_frame(&ctx, &mut app, vec![egui::Event::Text("40".into())]);
            focus_frame(&ctx, &mut app, focus_key(egui::Key::Enter));
            assert!(app.ui.dialog.is_none());
            assert_eq!(crate::panels::current_paints(&app).0.color(), Some(expected), "{mode:?}: reopened first field");
            assert_eq!(app.session.active().unwrap().history.undo.len(), undo + 1, "Enter commits once");
            focus_frame(&ctx, &mut app, vec![]);
        }
    }

    #[test]
    fn initial_focus_without_document_keeps_hue() {
        let mut app = VectorcraftApp::new(Session::new(), Default::default());
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        let original = Picked::of(Color::rgb8(51, 102, 153));
        app.run("ui.colorPicker", json!({"color": color_json(&original.color)})).unwrap();
        for _ in 0..3 {
            focus_frame(&ctx, &mut app, vec![]);
        }
        focus_frame(&ctx, &mut app, vec![egui::Event::Text("120".into())]);
        focus_frame(&ctx, &mut app, focus_key(egui::Key::Tab));
        let expected = Picked::from_hsb([120.0, original.hsb[1], original.hsb[2]]);
        assert_eq!(picked(app.ui.dialog.as_ref().unwrap()).color.to_hex(), expected.color.to_hex());
        focus_frame(&ctx, &mut app, focus_key(egui::Key::Escape));
        assert!(app.ui.dialog.is_none());
        assert!(app.session.active().is_none());
    }

