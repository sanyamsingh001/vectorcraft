fn choices(kind: &str, key: &str) -> Vec<(&'static str, &'static str)> {
    // The axes in the "axis" context: some languages read the plain Horizontal and Vertical as
    // type orientations.
    let axis = |value, label| (value, crate::i18n::tr_ctx(crate::i18n::current(), "axis", label));
    match (kind, key) {
        ("offsetPath", "joins") => vec![("miter", tl!("Miter")), ("round", tl!("Round")), ("bevel", tl!("Bevel"))],
        ("average", "axis") => vec![axis("horizontal", "Horizontal"), axis("vertical", "Vertical"), axis("both", "Both")],
        ("reflect", "axis") => vec![axis("horizontal", "Horizontal"), axis("vertical", "Vertical"), axis("angle", "Angle")],
        ("shear", "axis") => vec![axis("horizontal", "Horizontal"), axis("vertical", "Vertical")],
        _ => vec![],
    }
}
