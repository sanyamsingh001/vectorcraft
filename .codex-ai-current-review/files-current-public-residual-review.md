# files-current residual public-command review

Reviewed files-current on e037064b2d763f77b6927de3ffb70ef5b6c53e3a at parent r5 guard `22785ca7bb8e2d5bbd729957d576f815dfc038a8525670496f21c66690ce618d`. Root and candidate guards were unchanged throughout the completed native/AI probes. No candidate/root source edits, Cargo, remote actions, token reads, stress tests or security testing were performed.

## Concrete remaining test-only gap

`crates/engine/src/tests_placed_document.rs:716` in `svg_editing_keeps_placed_links_separate_from_drawn_previews` includes `image/svg+xml` as an `encode_all` format and unwraps the result. The public resolver accepts format ids/extensions, including case and dot aliases. It does not match MIME fields; SVG's extension is `svg`, while `image/svg+xml` is metadata.

Small cached CLI negative proof: `document.serialize` with `format:image/svg+xml` returns exit 1 and `unknown format image/svg+xml`. This error occurs in the unchanged resolver before SVG dispatch, so the new SVG preparation boundary does not alter it. The current test reaches the same resolver and will fail at that unwrap. Minimal test correction: use supported `.SVG` in place of the MIME string. Parent owns this correction; no patch or API expansion was applied by this reviewer.

The earlier Save-warning assertion issue is already corrected in r5. A small Save SVG probe returns its existing fidelity warning on an ordinary available document; checking specifically for the placed-preview warning is appropriate. These two checks are saved in `svg-new-test-assertions-review.json`.

## Native/AI result

No production gap was found in the bounded public-command native/AI review. 64 positive cases and 13 expected-error controls passed on existing CLI SHA-256 `45f9a68caf6f40192365c32754af1b985b698b8c81c87fb8c4b5254a0492fb37`. The executable was emitted during the earlier frozen gate-r3 build; the relevant Save/native/format/placed/export sources still match. Reversing only the current SVG/SVGZ exemption and its comment reproduces the recorded r3 encode.rs SHA. Native/template/AI use the identical exempt branch in both. This is scoped executable provenance, not a claim that the new SVG implementation ran on the old executable.

Original fixtures use a 40×30 source and an 80×60 parent with two artboards. The placed source uses bounding crop and affine `[1.15, 0.25, -0.35, 0.8, 7, 3]`.

Passed checks:
- Serialize, Export and Save without a destination, native/template/AI, includeLinked false/true, source available/missing: complete PlacedDocument metadata and transform retained, every referenced key exists, full inclusion bytes equal the original source exactly, source document.json unchanged.
- Public Save aliases: vectorcraft, .vectorcraft, VECTORCRAFT, drawcraft, .DRAWCRAFT, template, vctemplate, .VCTEMPLATE, ai, .ai, AI, .AI.
- Implicit native Serialize/Save and file.formatOptions report/write native version 4.
- Invalid AI boolean options reject for full and subset exports and nested Save options; invalid native version 5 rejects; all error controls leave document.json unchanged.

Raw case evidence: `public-command-residual-results.json`; rerunnable bounded fixture script: `check-public-command-residuals.py`.

## SVG source boundary

The parent correction exempts SVG/SVGZ from generic placed preparation, creates the native editing attachment from the original document, and prepares a temporary drawn document inside the SVG encoder. That source order preserves missing-link metadata while allowing available vector output and missing-source preview warnings. Alias normalization occurs before the exemption. No additional production gap was identified by this source review. Runtime execution of this new SVG correction remains with the parent's matched gate.

This is preliminary scoped feedback. It does not count toward the two final fully staffed frozen clean rounds, and it is not a global gap-free claim.
