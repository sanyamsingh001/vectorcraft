#[test]
fn syllables_map_to_their_source_bytes_and_caret_stops() {
    let Some(face) = devanagari_test_face("syllables_map_to_their_source_bytes_and_caret_stops") else { return };
    // हि + न्दी: the second syllable's matra stays with the conjunct it follows.
    let s = "हिन्दी";
    let l = devanagari_layout(&face, s);
    let second = "हि".len();
    assert_eq!(source_clusters(&l), vec![(0, second), (second, s.len() - second)]);
    // The first syllable's i matra is drawn before its consonant.
    assert_eq!(l.glyphs[1].gid, face.glyph_for('ह'), "{:?}", l.glyphs);
    assert!(l.glyphs[0].origin.x < l.glyphs[1].origin.x, "{:?}", l.glyphs);
    // Left and Right arrows pass over the same syllables the shaper made.
    assert_eq!(edit::next_cluster(s, 0), second);
    assert_eq!(edit::next_cluster(s, second), s.len());
    assert_eq!(edit::prev_cluster(s, s.len()), second);
    assert_eq!(caret_horizontal(&l, 0, true), second);
    assert_eq!(caret_horizontal(&l, second, true), s.len());
    assert_eq!(caret_horizontal(&l, s.len(), false), second);
    assert!((caret_position(&l, second).0.x - cluster_extent(&l, 0).1).abs() < 1e-9);
}
