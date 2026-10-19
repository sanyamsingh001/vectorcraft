
    #[test]
    fn moved_guide_release_refreshes_modifiers_and_position() {
        let plain = Mods::default();
        let shift = Mods { shift: true, ..plain };
        let alt = Mods { alt: true, ..plain };
        for vertical in [true, false] {
            let mut d = guides_doc();
            d.guides = vec![Guide::new(vertical, 100.0)];
            let mut s = Selection::default();
            s.set_guides([0]);
            let p = paint();
            let c = ToolContext { smart_guides: false, snap_to_point: false, ..cx(&d, &s, &p) };
            let event = |kind, at, mods| if vertical { ev(kind, at, 50.0, mods) } else { ev(kind, 50.0, at, mods) };
            for (first, last, release, want) in [
                (plain, shift, 223.0, 225.0),
                (shift, plain, 223.0, 223.0),
                (plain, plain, 237.0, 237.0),
                (plain, alt, 223.0, 223.0),
                (alt, plain, 223.0, 223.0),
                (plain, Mods { shift: true, ..alt }, 237.0, 235.0),
            ] {
                let mut g = GuideEdit::default();
                g.press(&c, &event(PointerKind::Down, 100.0, plain)).unwrap();
                g.pointer(&c, &event(PointerKind::Drag, 223.0, first)).unwrap();
                let actions = g.pointer(&c, &event(PointerKind::Up, release, last)).unwrap();
                let preview = actions
                    .iter()
                    .find_map(|a| match a {
                        Action::Preview(id, p) if id == "guide.move" => Some(p),
                        _ => None,
                    })
                    .unwrap_or_else(|| panic!("changed release must refresh the preview: vertical={vertical} first={first:?} last={last:?} release={release} want={want} actions={actions:?}"));
                let actual = 100.0 + preview[if vertical { "dx" } else { "dy" }].as_f64().unwrap();
                assert_eq!(actual, want, "vertical={vertical} first={first:?} last={last:?} release={release} actual={actual} want={want}");
                assert_eq!(preview["copy"], json!(last.alt));
                assert_eq!(actions.last(), Some(&Action::Commit));
                assert!(!g.busy() && g.overlays(&c).is_empty());
            }
        }
    }

    #[test]
    fn moved_guide_alt_changes_keep_initial_gesture() {
        let d = guides_doc();
        let mut s = Selection::default();
        s.set_guides([0, 1]);
        let p = paint();
        let c = ToolContext { smart_guides: false, snap_to_point: false, ..cx(&d, &s, &p) };
        let plain = Mods::default();
        let alt = Mods { alt: true, ..plain };
        let mut g = GuideEdit::default();
        g.press(&c, &ev(PointerKind::Down, 300.0, 50.0, plain)).unwrap();
        g.pointer(&c, &ev(PointerKind::Drag, 323.0, 65.0, plain)).unwrap();
        for m in [alt, plain, alt] {
            assert_eq!(
                g.pointer(&c, &ev(PointerKind::Drag, 323.0, 65.0, m)),
                Some(vec![Action::Preview("guide.move".into(), json!({"dx":23.0,"dy":15.0,"copy":m.alt}))]),
                "mode change keeps the original gesture delta without restarting the interaction"
            );
        }
        assert_eq!(g.pointer(&c, &ev(PointerKind::Up, 323.0, 65.0, alt)), Some(vec![Action::Commit]));
    }

    #[test]
    fn moved_guide_unchanged_release_keeps_commit_only() {
        let d = guides_doc();
        let mut s = Selection::default();
        s.set_guides([0]);
        let p = paint();
        let c = ToolContext { smart_guides: false, snap_to_point: false, ..cx(&d, &s, &p) };
        for shift in [false, true] {
            for alt in [false, true] {
                let m = Mods { shift, alt, ..Default::default() };
                let mut g = GuideEdit::default();
                g.press(&c, &ev(PointerKind::Down, 300.0, 50.0, Mods::default())).unwrap();
                g.pointer(&c, &ev(PointerKind::Drag, 323.0, 65.0, m)).unwrap();
                assert_eq!(g.pointer(&c, &ev(PointerKind::Up, 323.0, 65.0, m)), Some(vec![Action::Commit]));
            }
        }
    }

    #[test]
    fn moved_guide_click_and_ruler_drop_controls() {
        let plain = Mods::default();
        for vertical in [false, true] {
            let mut d = guides_doc();
            d.guides = vec![Guide::new(vertical, 100.0)];
            let mut s = Selection::default();
            s.set_guides([0]);
            let p = paint();
            let screen = ScreenFrame { origin: Point::ZERO, right: Vec2::new(1.0, 0.0), down: Vec2::new(0.0, 1.0), size: (800.0, 600.0) };
            let c = ToolContext { smart_guides: false, snap_to_point: false, screen: Some(screen), ..cx(&d, &s, &p) };
            let event = |kind, at, mods| if vertical { ev(kind, at, 50.0, mods) } else { ev(kind, 50.0, at, mods) };
            for shift in [false, true] {
                let m = Mods { shift, ..plain };
                let mut g = GuideEdit::default();
                assert_eq!(g.press(&c, &event(PointerKind::Down, 100.0, m)), Some(vec![]));
                assert_eq!(g.pointer(&c, &event(PointerKind::Drag, 102.0, m)), Some(vec![]));
                let expected = if shift { vec![Action::Exec("guide.select".into(), json!({"indexes":[0],"toggle":true}))] } else { vec![] };
                assert_eq!(g.pointer(&c, &event(PointerKind::Up, 102.0, m)), Some(expected));
            }
            for alt in [false, true] {
                let mut g = GuideEdit::default();
                g.press(&c, &event(PointerKind::Down, 100.0, plain)).unwrap();
                g.pointer(&c, &event(PointerKind::Drag, 223.0, plain)).unwrap();
                let mut expected = vec![Action::Cancel];
                if !alt {
                    expected.push(Action::Exec("guide.remove".into(), json!({"index":0})));
                }
                assert_eq!(g.pointer(&c, &event(PointerKind::Up, -5.0, Mods { alt, ..plain })), Some(expected));
            }
        }
    }
