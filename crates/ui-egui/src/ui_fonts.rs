//! Fallback fonts for the app's own text: layer and file names (and anything else the UI shows) in
//! Chinese, Japanese, Korean or other scripts the UI fonts lack are drawn in an installed font that
//! covers them. Builds made with craft-fonts (`CRAFT_FONTS_DIR`, all releases) carry Japanese fonts,
//! which `theme::install_fonts` adds after the UI fonts, so Japanese never gets here; nothing else
//! is bundled for them: the first time a frame paints a character the UI fonts
//! don't have, an installed font covering it is looked for (in the background on native) and added
//! to egui's families as their last fallback, from the next frame on. Text the UI fonts cover costs
//! no font loading or memory. On the web, which has no system fonts, such characters stay
//! missing-glyph boxes (Japanese too, unless built with craft-fonts).

use std::collections::HashSet;
use std::sync::Arc;

use egui::epaint::Shape;
use egui::epaint::text::{FontInsert, FontPriority, InsertFontFamily};
use egui::{FontData, FontFamily, FontId, LayerId};
use vectorcraft_text::{FontDb, FontFace};

#[derive(Default)]
pub(crate) struct UiFonts {
    /// Characters looked at already: the UI fonts have them, a fallback was added (or is being
    /// looked for), or no font has them.
    seen: HashSet<char>,
    /// Font availability when the painted characters were last checked.
    generation: Option<u64>,
    /// The fonts added to egui, by name.
    added: HashSet<String>,
    /// The faces covering the characters being looked for (native: looked for on a thread).
    #[cfg(not(target_arch = "wasm32"))]
    pending: Option<std::sync::mpsc::Receiver<Vec<Arc<FontFace>>>>,
}

impl UiFonts {
    /// After a frame is laid out: add the fallback fonts found since the last frame, and look for
    /// fonts covering the characters this frame painted that the UI fonts lack.
    pub(crate) fn frame(&mut self, ctx: &egui::Context) {
        self.frame_with_db(ctx, FontDb::global());
    }

    /// The production lookup path, with a database that can be private to a headless test.
    fn frame_with_db(&mut self, ctx: &egui::Context, db: &'static FontDb) {
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(faces) = self.pending.as_ref().and_then(|rx| rx.try_recv().ok()) {
            self.pending = None;
            self.install(ctx, &faces);
        }
        #[cfg(not(target_arch = "wasm32"))]
        if self.pending.is_some() {
            return;
        }
        let generation = db.generation();
        if self.generation.replace(generation).is_some_and(|previous| previous != generation) {
            // Keep installed fonts and any current search; only reconsider painted characters.
            self.seen.clear();
        }
        let missing = self.missing(ctx);
        if missing.is_empty() {
            return;
        }
        let find = move || {
            let mut faces: Vec<Arc<FontFace>> = vec![];
            for c in missing {
                if let Some(f) = db.face_covering(c)
                    && !faces.iter().any(|x| x.id() == f.id())
                {
                    faces.push(f);
                }
            }
            faces
        };
        // Native: the installed fonts are read off the UI thread. Characters met meanwhile wait for
        // the next search (they aren't marked seen until then).
        #[cfg(not(target_arch = "wasm32"))]
        {
            let (tx, rx) = std::sync::mpsc::channel();
            let repaint = ctx.clone();
            let spawned = std::thread::Builder::new().name("ui-font-fallback".into()).spawn(move || {
                // The receiver is gone only when the app is.
                let _ = tx.send(find());
                repaint.request_repaint();
            });
            if spawned.is_ok() {
                self.pending = Some(rx);
            }
        }
        // The web has only the loaded fonts to look through: quick, on this thread.
        #[cfg(target_arch = "wasm32")]
        self.install(ctx, &find());
    }

    /// Characters painted this frame that the UI fonts lack and haven't been looked for, now
    /// marked seen. None while a search is running.
    fn missing(&mut self, ctx: &egui::Context) -> Vec<char> {
        #[cfg(not(target_arch = "wasm32"))]
        if self.pending.is_some() {
            return vec![];
        }
        let mut layers: Vec<LayerId> = ctx.memory(|m| m.layer_ids().collect());
        if !layers.contains(&LayerId::background()) {
            layers.push(LayerId::background());
        }
        let mut chars: Vec<char> = vec![];
        let seen = &self.seen;
        let mut note = |text: &str| {
            // Most UI text is ASCII, which the UI fonts cover.
            if !text.is_ascii() {
                chars.extend(text.chars().filter(|c| !c.is_ascii() && !c.is_whitespace() && !c.is_control() && !seen.contains(c)));
            }
        };
        ctx.graphics(|g| {
            for l in &layers {
                for s in g.get(*l).into_iter().flat_map(|list| list.all_entries()) {
                    painted_text(&s.shape, &mut note);
                }
            }
        });
        if chars.is_empty() {
            return chars;
        }
        chars.sort_unstable();
        chars.dedup();
        let font = FontId::new(13.0, FontFamily::Proportional);
        let missing: Vec<char> = ctx.fonts_mut(|f| chars.iter().copied().filter(|c| !f.has_glyph(&font, *c)).collect());
        self.seen.extend(chars);
        missing
    }

    /// Add `faces` to every egui family, after the fonts it has.
    fn install(&mut self, ctx: &egui::Context, faces: &[Arc<FontFace>]) {
        let families: Vec<FontFamily> = ctx.fonts(|f| f.definitions().families.keys().cloned().collect());
        for face in faces {
            let name = format!("{} {} #{}", face.family, face.style, face.id());
            if !self.added.insert(name.clone()) {
                continue;
            }
            // The face's file data, shared with the process-wide font database (which keeps every
            // face it loads for the session) instead of copied: one handle is leaked per font added.
            let face: &'static FontFace = Box::leak(Box::new(face.clone()));
            let data = FontData { index: face.face_index(), ..FontData::from_static(face.file_data()) };
            let families = families.iter().map(|family| InsertFontFamily { family: family.clone(), priority: FontPriority::Lowest }).collect();
            ctx.add_font(FontInsert { name, data, families });
        }
        if !faces.is_empty() {
            ctx.request_repaint();
        }
    }

    /// Wait for the font search running, if any, and add what it found (tests).
    #[cfg(test)]
    pub(crate) fn finish(&mut self, ctx: &egui::Context) {
        if let Some(faces) = self.pending.take().and_then(|rx| rx.recv().ok()) {
            self.install(ctx, &faces);
        }
    }
}

/// Call `f` with the text of `shape` and the shapes it holds.
fn painted_text(shape: &Shape, f: &mut impl FnMut(&str)) {
    match shape {
        Shape::Text(t) => f(t.galley.text()),
        Shape::Vec(v) => v.iter().for_each(|s| painted_text(s, f)),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Run a frame painting `text`, then the fallback search after it.
    fn frame(ctx: &egui::Context, fonts: &mut UiFonts, text: &str) {
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            ui.label(text);
            fonts.frame(ui.ctx());
        });
        out.textures_delta.clear();
    }

    fn has_glyph(ctx: &egui::Context, c: char) -> bool {
        ctx.fonts_mut(|f| f.has_glyph(&FontId::new(13.0, FontFamily::Proportional), c))
    }

    #[test]
    fn latin_text_loads_no_fonts() {
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        let mut fonts = UiFonts::default();
        frame(&ctx, &mut fonts, "Layer 1 — Café");
        assert!(fonts.pending.is_none() && fonts.added.is_empty());
    }

    #[test]
    fn japanese_renders_with_the_craft_fonts_without_installed_fonts() {
        if !vectorcraft_text::CRAFT_FONTS.iter().any(|f| f.is_japanese()) {
            eprintln!("skipped: built without craft-fonts (set CRAFT_FONTS_DIR to a craft-fonts checkout to run it)");
            return;
        }
        let text = "日本語の文字";
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        let mut fonts = UiFonts::default();
        frame(&ctx, &mut fonts, text);
        // Real glyphs (no tofu) in every family from the first frame, with no installed font looked for.
        assert!(fonts.pending.is_none() && fonts.added.is_empty(), "no system-font search");
        let families: Vec<FontFamily> = ctx.fonts(|f| f.definitions().families.keys().cloned().collect());
        for family in families {
            let stack = ctx.fonts(|f| f.definitions().families.get(&family).cloned().unwrap_or_default());
            assert!(stack.last().is_some_and(|n| n.starts_with("craft-fonts ")), "{family:?}: craft-fonts last: {stack:?}");
            for c in text.chars() {
                assert!(ctx.fonts_mut(|f| f.has_glyph(&FontId::new(13.0, family.clone()), c)), "{family:?} draws {c}");
            }
        }
        // The UI prefers BIZ UDPGothic.
        let stack = ctx.fonts(|f| f.definitions().families.get(&FontFamily::Proportional).cloned().unwrap_or_default());
        let first = stack.iter().find(|n| n.starts_with("craft-fonts ")).cloned().unwrap_or_default();
        assert_eq!(first, "craft-fonts BIZ UDPGothic Regular", "{stack:?}");
    }

    #[test]
    fn the_ui_works_without_the_craft_fonts() {
        // Built either way, the fonts install and Japanese text lays out (as installed-font
        // fallbacks or missing-glyph boxes when no font has it).
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        let mut fonts = UiFonts::default();
        frame(&ctx, &mut fonts, "日本語の文字 Layer 1");
        fonts.finish(&ctx);
        frame(&ctx, &mut fonts, "日本語の文字 Layer 1");
        if vectorcraft_text::CRAFT_FONTS.is_empty() {
            let names: Vec<String> = ctx.fonts(|f| f.definitions().font_data.keys().cloned().collect());
            assert!(names.iter().all(|n| !n.starts_with("craft-fonts")), "{names:?}");
        }
        assert!(has_glyph(&ctx, 'L'));
    }

    #[test]
    fn cjk_names_get_an_installed_fallback_font() {
        let text = "Logo 标志 ロゴ 로고";
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        let mut fonts = UiFonts::default();
        frame(&ctx, &mut fonts, text);
        assert!(!has_glyph(&ctx, '标'), "the UI fonts have no CJK");
        fonts.finish(&ctx);
        // The fonts arrive with the next frame.
        frame(&ctx, &mut fonts, text);
        let installed: Vec<char> = text.chars().filter(|c| !c.is_ascii() && FontDb::global().face_covering(*c).is_some()).collect();
        if installed.is_empty() {
            eprintln!("no installed font covers {text}: nothing to check");
            return;
        }
        for c in installed {
            assert!(has_glyph(&ctx, c), "{c} has a glyph");
        }
        // Looked for once: a later frame starts no search.
        frame(&ctx, &mut fonts, text);
        assert!(fonts.pending.is_none());
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod rescan_retry_review_tests {
    use super::*;

    // Original two-glyph TrueType fixture: .notdef plus one 100x100 square. The scalar is
    // private use, so the test needs no installed language font, craft-fonts or compositor.
    fn new_ui_font(c: char) -> Vec<u8> {
        new_ui_font_with_version(c, None)
    }

    fn new_ui_font_with_version(c: char, version: Option<&str>) -> Vec<u8> {
        let family = "VC Review UI Rescan";
        let mut head: Vec<u8> = [0x0001_0000_u32, 0x0001_0000, 0, 0x5f0f_3cf5].into_iter().flat_map(u32::to_be_bytes).collect();
        head.extend([0_u16, 1000].into_iter().flat_map(u16::to_be_bytes));
        head.extend([0; 16]);
        head.extend([0_i16, 0, 100, 100, 0, 8, 2, 0, 0].into_iter().flat_map(i16::to_be_bytes));
        let mut hhea = 0x0001_0000_u32.to_be_bytes().to_vec();
        hhea.extend([800_i16, -200, 0, 600, 0, 500, 100, 1, 0, 0, 0, 0, 0, 0, 0, 2].into_iter().flat_map(i16::to_be_bytes));
        let mut maxp = 0x0001_0000_u32.to_be_bytes().to_vec();
        maxp.extend([2_u16, 4, 1, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0].into_iter().flat_map(u16::to_be_bytes));
        let mut glyf: Vec<u8> = [1_i16, 0, 0, 100, 100, 3, 0].into_iter().flat_map(i16::to_be_bytes).collect();
        glyf.extend([1; 4]);
        glyf.extend([0_i16, 100, 0, -100, 0, 0, 100, 0].into_iter().flat_map(i16::to_be_bytes));
        let loca: Vec<u8> = [0_u16, 0, (glyf.len() / 2) as u16].into_iter().flat_map(u16::to_be_bytes).collect();
        let hmtx: Vec<u8> = [600_u16, 0, 600, 0].into_iter().flat_map(u16::to_be_bytes).collect();
        let mut post = vec![0; 32];
        post[..4].copy_from_slice(&0x0003_0000_u32.to_be_bytes());
        let mut names = vec![];
        let mut strings = vec![];
        let records = [(1_u16, family), (2, "Regular"), (4, family)]
            .into_iter()
            .chain(version.map(|value| (5, value)))
            .chain([(6, "VCReviewUIRescan-Regular")]);
        for (id, value) in records {
            let bytes: Vec<u8> = value.encode_utf16().flat_map(u16::to_be_bytes).collect();
            names.extend([3_u16, 1, 0x409, id, bytes.len() as u16, strings.len() as u16].into_iter().flat_map(u16::to_be_bytes));
            strings.extend(bytes);
        }
        let count = if version.is_some() { 5_u16 } else { 4 };
        let mut name: Vec<u8> = [0_u16, count, 6 + 12 * count].into_iter().flat_map(u16::to_be_bytes).collect();
        name.extend(names);
        name.extend(strings);
        let mut cmap: Vec<u8> = [0_u16, 1, 3, 10].into_iter().flat_map(u16::to_be_bytes).collect();
        cmap.extend(12_u32.to_be_bytes());
        cmap.extend([12_u16, 0].into_iter().flat_map(u16::to_be_bytes));
        cmap.extend([28_u32, 0, 1, u32::from(c), u32::from(c), 1].into_iter().flat_map(u32::to_be_bytes));
        let mut tables = vec![
            (b"head", head),
            (b"hhea", hhea),
            (b"maxp", maxp),
            (b"glyf", glyf),
            (b"loca", loca),
            (b"hmtx", hmtx),
            (b"post", post),
            (b"name", name),
            (b"cmap", cmap),
        ];
        tables.sort_by_key(|(tag, _)| *tag);
        let n = tables.len() as u16;
        let power = 1_u16 << n.ilog2();
        let mut font = 0x0001_0000_u32.to_be_bytes().to_vec();
        font.extend([n, power * 16, power.ilog2() as u16, n * 16 - power * 16].into_iter().flat_map(u16::to_be_bytes));
        let mut offset = 12 + 16 * tables.len();
        for (tag, data) in &tables {
            font.extend_from_slice(*tag);
            let checksum = data.chunks(4).fold(0_u32, |sum, chunk| {
                let mut word = [0; 4];
                word[..chunk.len()].copy_from_slice(chunk);
                sum.wrapping_add(u32::from_be_bytes(word))
            });
            font.extend([checksum, offset as u32, data.len() as u32].into_iter().flat_map(u32::to_be_bytes));
            offset += data.len().next_multiple_of(4);
        }
        let mut head_offset = 0;
        for (tag, data) in &tables {
            if *tag == b"head" {
                head_offset = font.len();
            }
            font.extend_from_slice(data);
            font.resize(font.len().next_multiple_of(4), 0);
        }
        let checksum = font.as_chunks::<4>().0.iter().fold(0_u32, |sum, word| sum.wrapping_add(u32::from_be_bytes(*word)));
        font[head_offset + 8..head_offset + 12].copy_from_slice(&0xb1b0_afba_u32.wrapping_sub(checksum).to_be_bytes());
        font
    }

    fn frame(db: &'static FontDb, ctx: &egui::Context, fonts: &mut UiFonts, text: &str) {
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            ui.label(text);
            fonts.frame_with_db(ui.ctx(), db);
        });
        output.textures_delta.clear();
    }

    fn has_glyph(ctx: &egui::Context, c: char) -> bool {
        ctx.fonts_mut(|f| f.has_glyph(&FontId::new(13.0, FontFamily::Proportional), c))
    }

    #[test]
    fn completed_ui_miss_retries_after_font_generation_changes() {
        // One local database, with no host-font scan directories. It outlives the real workers;
        // finish() receives each completed lookup. The process-wide database and other UI tests are untouched.
        let db: &'static FontDb = Box::leak(Box::new(FontDb::with_font_dirs(vec![])));
        assert_eq!(db.load_system_fonts(), 0);
        let c = '\u{10fffd}';
        let text = c.to_string();
        assert!(db.face_covering(c).is_none(), "fixture scalar must be absent initially");
        let ctx = egui::Context::default();
        let mut fonts = UiFonts::default();
        frame(db, &ctx, &mut fonts, &text);
        assert!(fonts.pending.is_some(), "the ordinary worker must search for this glyph");
        fonts.finish(&ctx);
        frame(db, &ctx, &mut fonts, &text);
        assert!(fonts.seen.contains(&c) && fonts.added.is_empty());
        assert!(fonts.pending.is_none(), "a completed miss is cached at the same generation");
        assert!(!has_glyph(&ctx, c));
        let generation = db.generation();
        frame(db, &ctx, &mut fonts, &text);
        assert!(fonts.pending.is_none());
        assert_eq!(db.generation(), generation);

        assert_eq!(db.add_font(new_ui_font(c)), 1);
        // The backend API called by text.rescanFonts, against this test's empty scan paths.
        assert_eq!(db.load_system_fonts(), 0);
        assert!(db.generation() > generation);
        let face = db.face_covering(c).unwrap();
        assert_eq!(face.family, "VC Review UI Rescan");

        // Fresh-context control: both the original bytes and the unchanged fallback installer
        // must draw the exact scalar. No installed font, global mutation or sleep is needed.
        let fresh = egui::Context::default();
        let mut fresh_fonts = UiFonts::default();
        frame(db, &fresh, &mut fresh_fonts, &text);
        fresh_fonts.finish(&fresh);
        frame(db, &fresh, &mut fresh_fonts, &text);
        assert!(has_glyph(&fresh, c), "fresh-context control must have the new glyph");
        assert_eq!(fresh_fonts.added.len(), 1);

        frame(db, &ctx, &mut fonts, &text);
        fonts.finish(&ctx);
        frame(db, &ctx, &mut fonts, &text);
        assert!(has_glyph(&ctx, c), "a completed earlier UI miss must retry after availability changes");
        assert_eq!(fonts.added.len(), 1);
        assert!(fonts.pending.is_none());
        let added = fonts.added.clone();
        let families = ctx.fonts(|f| f.definitions().families.clone());
        let names = ctx.fonts(|f| f.definitions().font_data.keys().cloned().collect::<Vec<_>>());
        frame(db, &ctx, &mut fonts, &text);
        assert_eq!(fonts.added, added);
        assert!(fonts.pending.is_none(), "same-generation hits need no further search");

        // A later rescan retains installed UI fonts. Rechecking painted characters that now
        // have glyphs must not start another worker or add a duplicate family/style name.
        let generation = db.generation();
        assert_eq!(db.load_system_fonts(), 0);
        assert!(db.generation() > generation);
        assert_eq!(db.face_covering(c).unwrap().id(), face.id());
        frame(db, &ctx, &mut fonts, &text);
        assert!(has_glyph(&ctx, c) && fonts.seen.contains(&c));
        assert!(fonts.pending.is_none());
        assert_eq!(fonts.added, added);
        assert_eq!(ctx.fonts(|f| f.definitions().families.clone()), families);
        assert_eq!(ctx.fonts(|f| f.definitions().font_data.keys().cloned().collect::<Vec<_>>()), names);
    }

    #[test]
    fn completed_ui_miss_retries_with_another_version_of_an_installed_family() {
        // Private database and real lookup workers; both original fixtures have the same family,
        // style and face index, but name-ID 5 and the sole mapped private-use scalar differ.
        let db: &'static FontDb = Box::leak(Box::new(FontDb::with_font_dirs(vec![])));
        assert_eq!(db.load_system_fonts(), 0);
        let x = '\u{10fffc}';
        let y = '\u{10fffd}';
        let text = format!("{x}{y}");
        assert!(db.face_covering(x).is_none() && db.face_covering(y).is_none());
        let generation = db.generation();
        assert_eq!(db.add_font(new_ui_font_with_version(x, Some("Version 1.000"))), 1, "the first version loads");
        assert!(db.generation() > generation);
        let old_face = db.face_covering(x).unwrap();
        assert_eq!(old_face.family, "VC Review UI Rescan");
        assert_eq!(old_face.style, "Regular");
        assert_eq!(old_face.face_index(), 0);
        assert_eq!(old_face.version, "Version 1.000");
        assert!(old_face.covers(x) && !old_face.covers(y));

        let ctx = egui::Context::default();
        let mut fonts = UiFonts::default();
        frame(db, &ctx, &mut fonts, &x.to_string());
        assert!(fonts.pending.is_some(), "the first version uses the actual worker");
        fonts.finish(&ctx);
        frame(db, &ctx, &mut fonts, &x.to_string());
        assert!(has_glyph(&ctx, x), "old-context positive: the first version draws X");
        assert!(!has_glyph(&ctx, y));
        assert_eq!(fonts.added.len(), 1);
        assert!(fonts.pending.is_none());
        let installed = fonts.added.clone();

        frame(db, &ctx, &mut fonts, &text);
        assert!(fonts.pending.is_some(), "Y must first go through an actual unsuccessful lookup");
        fonts.finish(&ctx);
        frame(db, &ctx, &mut fonts, &text);
        assert!(fonts.seen.contains(&y) && fonts.pending.is_none(), "the Y miss is completed and cached");
        assert!(has_glyph(&ctx, x) && !has_glyph(&ctx, y));
        assert_eq!(fonts.added, installed);
        let generation = db.generation();
        frame(db, &ctx, &mut fonts, &text);
        assert!(fonts.pending.is_none(), "an unchanged generation does not retry the completed miss");
        assert_eq!(db.generation(), generation);

        assert_eq!(db.add_font(new_ui_font_with_version(y, Some("Version 2.000"))), 1, "another version is retained by the backend");
        assert!(db.generation() > generation, "adding the second version changes availability");
        let new_face = db.face_covering(y).unwrap();
        assert_eq!(new_face.version, "Version 2.000");
        assert_eq!(new_face.family, old_face.family);
        assert_eq!(new_face.style, old_face.style);
        assert_eq!(new_face.face_index(), old_face.face_index());
        assert_ne!(new_face.id(), old_face.id());
        assert!(new_face.covers(y) && !new_face.covers(x), "the two faces have disjoint glyph coverage");
        assert_eq!(db.face_covering(x).unwrap().id(), old_face.id(), "the first version stays available");

        // Establish that the second fixture and the unchanged installer actually draw Y before
        // asking the old context to add this version beside its already installed first version.
        let fresh = egui::Context::default();
        let mut fresh_fonts = UiFonts::default();
        frame(db, &fresh, &mut fresh_fonts, &y.to_string());
        assert!(fresh_fonts.pending.is_some());
        fresh_fonts.finish(&fresh);
        frame(db, &fresh, &mut fresh_fonts, &y.to_string());
        assert!(has_glyph(&fresh, y), "fresh-context positive: the second version draws Y");
        assert!(!has_glyph(&fresh, x), "the fresh control installed only the second version");
        assert_eq!(fresh_fonts.added.len(), 1);
        assert!(fresh_fonts.pending.is_none());

        frame(db, &ctx, &mut fonts, &text);
        assert_eq!(fonts.generation, Some(db.generation()));
        assert!(fonts.pending.is_some(), "the changed generation retries Y through the actual worker");
        fonts.finish(&ctx);
        assert!(fonts.pending.is_none());
        frame(db, &ctx, &mut fonts, &text);
        assert!(has_glyph(&ctx, x), "retrying Y must retain the first version's X");
        assert!(
            has_glyph(&ctx, y),
            "completed miss must install the newer same-family version: old={:?}, new={:?}, installed={:?}",
            old_face.version,
            new_face.version,
            fonts.added
        );
        assert!(installed.is_subset(&fonts.added), "the existing fallback must remain installed");
        assert_eq!(fonts.added.len(), 2, "the two distinct faces each supply their own glyph");
        let installed = fonts.added.clone();
        frame(db, &ctx, &mut fonts, &text);
        assert!(has_glyph(&ctx, x) && has_glyph(&ctx, y));
        assert!(fonts.pending.is_none());
        assert_eq!(fonts.added, installed, "another frame must not duplicate either version");
    }
}
