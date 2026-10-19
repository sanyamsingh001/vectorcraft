#[test]
fn type_arrows_keep_devanagari_clusters_with_an_rtl_suffix() {
    // Use the text tests' installed-family order, requiring coverage of the unchanged Hindi input.
    // The Hebrew suffix selects the Type tool's existing visual-arrow route.
    let hindi = "हिन्दी";
    let second = "हि".len();
    assert_eq!((second, hindi.len()), (6, 18));
    let db = FontDb::global();
    let face = ["Kohinoor Devanagari", "ITF Devanagari", "Devanagari Sangam MN", "Noto Sans Devanagari", "Nirmala UI", "Mangal"]
        .into_iter()
        .filter_map(|name| db.face(name, "Regular").filter(|face| face.family == name))
        .find(|face| hindi.chars().all(|ch| face.covers(ch)));
    let Some(face) = face else {
        eprintln!("SKIPPED: type_arrows_keep_devanagari_clusters_with_an_rtl_suffix; no installed candidate font covers the Hindi input");
        return;
    };
    eprintln!(
        "TYPE_ARROW_FONT: {} {}; version={}; bytes={}; face_index={}",
        face.family,
        face.style,
        face.version,
        face.file_data().len(),
        face.face_index()
    );
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
        // Raw shaping spans are diagnostic; editing boundaries do not depend on their count.
        assert!(!hindi_spans.is_empty(), "{case}: Hindi glyph source spans are present");
        for glyph in lay.glyphs.iter().filter(|g| g.byte < hindi.len()) {
            let used = db.face_by_id(glyph.font_id).expect("the Hindi glyph's loaded face");
            assert!(used.file_data() == face.file_data(), "{case}: Hindi uses the selected face bytes");
            assert_eq!(used.face_index(), face.face_index(), "{case}: Hindi uses the selected face index");
            assert_ne!(glyph.gid, 0, "{case}: Hindi glyph is present");
            assert!(!glyph.rtl, "{case}: Hindi prefix remains LTR");
        }
        assert_eq!(lay.glyphs.iter().any(|g| g.rtl), visual_route, "{case}: actual Type arrow-route predicate");
        if visual_route {
            assert!(lay.glyphs.iter().any(|g| g.rtl && g.byte > hindi.len()), "{case}: the ordinary suffix supplies RTL glyphs");
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

#[test]
fn type_arrows_keep_devanagari_clusters_across_wrapped_lines() {
    use vectorcraft_doc::{Composer, Justify};
    use vectorcraft_geom::{Affine, PathData};

    // Use the installed-family order and unchanged Hindi input from the single-line test.
    // Shaping and wrapping capabilities vary by face; an eligibility skip is not arrow coverage.
    let hindi = "हिन्दी";
    let text = "हिन्दी שלום";
    let second = "हि".len();
    let split = "हिन्".len();
    assert_eq!((second, split, hindi.len()), (6, 12, 18));
    let db = FontDb::global();
    let face = ["Kohinoor Devanagari", "ITF Devanagari", "Devanagari Sangam MN", "Noto Sans Devanagari", "Nirmala UI", "Mangal"]
        .into_iter()
        .filter_map(|name| db.face(name, "Regular").filter(|face| face.family == name))
        .find(|face| hindi.chars().all(|ch| face.covers(ch)));
    let Some(face) = face else {
        eprintln!("SKIPPED: type_arrows_keep_devanagari_clusters_across_wrapped_lines; no installed candidate font covers the Hindi input");
        return;
    };
    eprintln!(
        "TYPE_WRAP_FONT: {} {}; version={}; bytes={}; face_index={}",
        face.family,
        face.style,
        face.version,
        face.file_data().len(),
        face.face_index()
    );
    let style = CharStyle { font_family: face.family.clone(), font_style: face.style.clone(), size: 20.0, ..Default::default() };
    let (mut d, id, mut tool) = editing(text);
    let NodeKind::Text(t) = &mut d.node_mut(id).unwrap().kind else { panic!("text fixture") };
    for run in &mut t.runs {
        run.style = style.clone();
    }
    t.para.justify = Justify::Left;
    t.para.composer = Composer::SingleLine;
    t.para.hyphenate = false;
    assert_eq!(t.plain_text(), text);
    assert_eq!(&text[..hindi.len()], hindi);

    // Measure once on the selected face. No width sweep or invented line endpoints.
    let natural = vectorcraft_text::layout(db, t);
    assert_eq!(natural.lines.len(), 1, "setup: natural point text is a single line");
    assert!(!natural.vertical, "setup: horizontal text");
    let hindi_glyphs: Vec<_> = natural.glyphs.iter().filter(|g| g.byte < hindi.len()).collect();
    assert!(!hindi_glyphs.is_empty(), "setup: actual Hindi glyphs exist");
    for glyph in &hindi_glyphs {
        let used = db.face_by_id(glyph.font_id).expect("the Hindi glyph's loaded face");
        assert!(used.file_data() == face.file_data(), "setup: Hindi uses the selected face bytes");
        assert_eq!(used.face_index(), face.face_index(), "setup: Hindi uses the selected face index");
        assert_ne!(glyph.gid, 0, "setup: Hindi glyph is present");
        assert!(!glyph.rtl, "setup: Hindi prefix is LTR");
        assert!(glyph.len > 0 && glyph.byte + glyph.len <= hindi.len(), "setup: Hindi glyph source range");
        assert!(
            text.is_char_boundary(glyph.byte) && text.is_char_boundary(glyph.byte + glyph.len),
            "setup: glyph source ranges are UTF-8 boundaries"
        );
    }
    let prefix_width: f64 = hindi_glyphs.iter().filter(|g| g.byte < split).map(|g| g.advance).sum();
    let split_width: f64 = hindi_glyphs.iter().filter(|g| g.byte == split).map(|g| g.advance).sum();
    assert!(prefix_width.is_finite() && prefix_width > 0.0, "setup: prefix has positive finite advance");
    assert!(split_width.is_finite(), "setup: the internal raw-group advance is finite");
    if split_width <= 0.0 {
        eprintln!(
            "SKIPPED: type_arrows_keep_devanagari_clusters_across_wrapped_lines; layout capability: the selected face has no positive-advance raw group at internal byte {split}"
        );
        return;
    }
    let width = prefix_width + split_width * 0.5;
    assert!(width.is_finite() && width > prefix_width && width < prefix_width + split_width, "setup: one measured width separates the raw groups");

    t.kind = TextKind::Area { frame: PathData::from_bezpath(&Rect::new(0.0, 0.0, width, 500.0).to_path(0.1)) };
    t.xf = Affine::IDENTITY;
    assert_eq!((t.area.rows, t.area.columns, t.area.inset), (1, 1, 0.0));
    assert_eq!((t.para.left_indent, t.para.first_line_indent, t.para.right_indent), (0.0, 0.0, 0.0));
    let lay = vectorcraft_text::layout(db, t);
    assert!(!lay.overflow, "setup: the full RTL suffix fits the tall frame");
    assert!(!lay.vertical, "setup: horizontal area text");
    assert!(lay.glyphs.iter().any(|g| g.rtl && g.byte > hindi.len()), "setup: actual RTL suffix selects Type's visual-arrow route");
    for glyph in lay.glyphs.iter().filter(|g| g.byte < hindi.len()) {
        let used = db.face_by_id(glyph.font_id).expect("the wrapped Hindi glyph's loaded face");
        assert!(used.file_data() == face.file_data(), "setup: wrapped Hindi uses the selected face bytes");
        assert_eq!(used.face_index(), face.face_index(), "setup: wrapped Hindi uses the selected face index");
        assert_ne!(glyph.gid, 0, "setup: wrapped Hindi glyph is present");
        assert!(!glyph.rtl, "setup: wrapped Hindi prefix stays LTR");
    }
    assert_eq!(edit::next_cluster(text, second), hindi.len(), "setup: logical cluster end is 18");
    assert_eq!(edit::prev_cluster(text, hindi.len()), second, "setup: logical cluster start is 6");
    assert_eq!(edit::prev_cluster(text, split), second, "setup: raw line endpoint 12 is inside the grapheme");
    assert_eq!(edit::next_cluster(text, split), hindi.len(), "setup: raw line endpoint 12 is not its end");
    if lay.lines.len() < 2 || (lay.lines[0].start, lay.lines[0].end, lay.lines[1].start) != (0, split, split) {
        eprintln!(
            "SKIPPED: type_arrows_keep_devanagari_clusters_across_wrapped_lines; layout capability: the measured frame does not expose adjacent wrapped endpoints at internal byte {split}"
        );
        return;
    }
    assert!(lay.lines.len() >= 2, "setup: actual wrapping occurred");
    assert_eq!(
        (lay.lines[0].start, lay.lines[0].end, lay.lines[1].start),
        (0, split, split),
        "setup: actual adjacent line endpoints reach internal byte 12"
    );
    assert_eq!(lay.line_of(second), 0, "setup: caret 6 is on the preceding line");
    assert!(!lay.lines[0].rtl, "setup: the preceding paragraph line is LTR");
    tool.caret = second;
    tool.anchor = second;
    assert_eq!(tool.editing, Some(id));
    assert_eq!(tool.sel(), (second, second), "setup: collapsed caret before Right");
    assert!(tool.typing.is_none(), "setup: no pending typing session");
    eprintln!(
        "TYPE_WRAP_PRE_ASSERT_READY: line_end={}; next_start={}; caret={}; anchor={}",
        lay.lines[0].end, lay.lines[1].start, tool.caret, tool.anchor
    );
    key(&mut tool, &d, ToolKey::Right, Mods::default());
    assert_eq!((tool.caret, tool.anchor), (hindi.len(), hindi.len()), "wrapped Right must move 6 -> 18 across the Hindi grapheme");
    key(&mut tool, &d, ToolKey::Left, Mods::default());
    assert_eq!((tool.caret, tool.anchor), (second, second), "wrapped Left must move 18 -> 6 across the Hindi grapheme");
    eprintln!("TYPE_WRAP_CASE_COMPLETE");
}

