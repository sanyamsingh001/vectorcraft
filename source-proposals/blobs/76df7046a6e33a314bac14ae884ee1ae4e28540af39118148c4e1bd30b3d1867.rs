#[test]
fn type_arrows_keep_devanagari_clusters_with_an_rtl_suffix() {
    // Keep the Hindi input and face from the ordinary-text diagnostic. The Hebrew
    // suffix only selects the Type tool's existing visual-arrow route.
    let db = FontDb::global();
    let face = db.face("Noto Sans Devanagari", "Regular").expect("the diagnosed Devanagari face is installed");
    assert_eq!(face.family, "Noto Sans Devanagari");
    assert_eq!(face.style, "Regular");
    eprintln!(
        "TYPE_ARROW_FONT: {} {}; version={}; bytes={}; face_index={}",
        face.family, face.style, face.version, face.file_data().len(), face.face_index()
    );
    let hindi = "हिन्दी";
    let second = "हि".len();
    assert_eq!((second, hindi.len()), (6, 18));
    assert!(hindi.chars().all(|ch| face.covers(ch)), "the selected face covers the unchanged Hindi prefix");
    let style = CharStyle { font_family: face.family.clone(), font_style: face.style.clone(), size: 20.0, ..Default::default() };

    // The plain-LTR control runs first. Each case uses a fresh document and tool.
    for (case, text, visual_route) in [("plain_ltr", "हिन्दी", false), ("mixed_rtl", "हिन्दी שלום", true)] {
        let (mut d, id, mut tool) = editing(text);
        let NodeKind::Text(t) = &mut d.node_mut(id).unwrap().kind else { panic!("text fixture") };
        for run in &mut t.runs {
            run.style = style.clone();
        }
        assert_eq!(t.plain_text(), text);
        assert_eq!(&text[..hindi.len()], hindi);
        let lay = vectorcraft_text::layout(db, t);
        assert_eq!(lay.lines.len(), 1, "{case}: one ordinary horizontal line");
        assert!(!lay.vertical, "{case}: horizontal Type");
        let mut hindi_spans: Vec<_> = lay.glyphs.iter().filter(|g| g.byte < hindi.len()).map(|g| (g.byte, g.len)).collect();
        hindi_spans.sort_unstable();
        hindi_spans.dedup();
        // This is the diagnosed shaping precondition, not the desired caret oracle.
        assert_eq!(hindi_spans, vec![(0, 6), (6, 6), (12, 6)], "{case}: retain the diagnosed raw Hindi spans");
        for glyph in lay.glyphs.iter().filter(|g| g.byte < hindi.len()) {
            let used = db.face_by_id(glyph.font_id).expect("the Hindi glyph's loaded face");
            assert!(used.file_data() == face.file_data(), "{case}: Hindi uses the selected face bytes");
            assert_eq!(used.face_index(), face.face_index(), "{case}: Hindi uses the selected face index");
            assert_ne!(glyph.gid, 0, "{case}: Hindi glyph is present");
            assert!(!glyph.rtl, "{case}: Hindi prefix remains LTR");
        }
        assert_eq!(lay.glyphs.iter().any(|g| g.rtl), visual_route, "{case}: actual Type arrow-route predicate");
        if visual_route {
            assert!(lay.glyphs.iter().any(|g| g.rtl && g.byte >= hindi.len() + 1), "{case}: the ordinary suffix supplies RTL glyphs");
        }
        assert_eq!(edit::next_cluster(text, second), hindi.len(), "{case}: logical cluster end");
        assert_eq!(edit::prev_cluster(text, hindi.len()), second, "{case}: logical cluster start");
        tool.caret = second;
        tool.anchor = second;
        assert_eq!(tool.editing, Some(id));
        assert_eq!(tool.sel(), (second, second), "{case}: collapsed caret before Right");
        assert!(tool.typing.is_none(), "{case}: no pending typing session");
        eprintln!(
            "TYPE_ARROW_PRE_ASSERT_READY: case={case}; hindi_spans={hindi_spans:?}; visual_route={visual_route}; caret={}; anchor={}",
            tool.caret, tool.anchor
        );
        key(&mut tool, &d, ToolKey::Right, Mods::default());
        assert_eq!((tool.caret, tool.anchor), (hindi.len(), hindi.len()), "{case}: Right must move 6 -> 18 across the Hindi cluster");
        key(&mut tool, &d, ToolKey::Left, Mods::default());
        assert_eq!((tool.caret, tool.anchor), (second, second), "{case}: Left must move 18 -> 6 across the Hindi cluster");
        eprintln!("TYPE_ARROW_CASE_COMPLETE: {case}");
    }
}

