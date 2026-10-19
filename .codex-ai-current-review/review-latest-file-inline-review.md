# review-latest file/inline merge review

Latest base `5f92f5eb7fd194826aab2ad0844e474f13990938`. Observed complete tracked HEAD diff SHA `b1a8c4db72af9b9a589349a51cde9477d160f094aab0b6ef382be619a48e1419`. Parent expanded review-latest from the initial unstaged file patch/two new tests to a staged UI/trace/updates composition during this review. All nine recorded file/native/inline source hashes stayed unchanged. This review is scoped to those paths; it is not a review of every newly composed feature. No candidate/root edits, Cargo builds, remote actions, token reads, stress or security tests were run. Other OS/tablet behavior, #620 and #633 were left outside scope.

## P2 LATEST-PLACED-INLINE-RESOURCE-SCOPE

Source-confirmed compatibility risk in the new inline feature plus existing placed-document output preparation. Runtime on the latest model is unexecuted: the available cached CLI was built against e037 and cannot validate the new TextRun.inline field or command.

Normal workflow: create an original native source with a symbol inline in text, place that source linked in another document, then export SVG or AI/PDF artwork. Two contracts fail in the exact current source:

1. `crates/doc/src/placed_document.rs:437` namespaces symbol-instance names and text pattern paints, but does not change TextRun.inline.symbol. `add_resources` at line 531 copies Dot as placed:<key>/Dot. The prepared inline reference remains Dot. Latest SVG text_node (line 1660) resolves it against the parent document and emits a SymbolInstance; SVG's SymbolInstance case (line 1377) returns without drawing when that symbol is absent. A parent Dot resolves to its own art instead. PDF uses the same prepared-document symbol lookup.
2. The same resource importer copies source symbol art without its `unknown.symbolSizes` metadata. `crates/doc/src/inline.rs:24` uses that table to restore an app-created symbol's natural size. A normalized 20x20 definition originally drawn at 40x10 gets 20x20 bounds after import, changing inline aspect ratio even once the name is corrected.

Both omissions are in `crates/doc/src/placed_document.rs`. This producer is unchanged from e037; new reference and natural-size consumers arrived upstream. The local SVG metadata and editing-attachment corrections did not introduce this incompatibility.

Ready test-only patch: `review-latest-inline-placed-test.patch`, targeting the existing model tests module in `crates/doc/src/placed_document.rs` on 5f92. It changes no production code. Two tests use one original blue shape, an inline run, optional red same-name parent art, normalized source dimensions, and the real with_placed_art / PlacedDocument::art pipeline. They avoid fonts, file IO, render buffers and importer stubs; the reader cache is primed with the original synthetic model as existing tests do.

Expected negatives, not executed results: the reference test cannot resolve the inline when the parent has no Dot, or obtains the red parent art when it does; the natural-size test expects 40x10 but obtains 20x20. Regression artifact formatting/parsing passed. Apply/run only on the latest inline-capable base when the parent authorizes builds. No production refactor or fix was made.

## Preserved merge contracts

The latest SVG text_node implementation is byte-identical to upstream 5f92 after applying the file candidate; metadata hunks do not overwrite the new inline rendering implementation. The native, SVG, generic encode and format resolver source files match files-current. SVG creates its native attachment before temporary placed-art preparation; native v4 and AI retain the original node tree while moving prepared resources onto the copy. New TextRun.inline parameters, ICF alignment and source symbols remain serializable in the original tree. Upstream's inline_graphics_in_text_round_trip test remains present. This is source verification, not latest-base runtime proof.

review-latest's save.rs additionally contains the live-preview save-date block absent from files-current. This composition delta only updates timestamps in the interaction snapshot; it does not change encoding or original-tree restoration.

The previously reported unsupported image/svg+xml test alias remains in the inherited SVG regression, independently of the upstream inline merge. No new MIME alias support is implied by upstream.

Current e037 bounded native/AI CLI evidence remains scoped to e037. It is not a passing latest-base gate or one of the two final fully staffed frozen clean rounds. Parent should run the ready model regressions and the existing inline SVG/PDF/native tests on the merged base.

The test-only patch passed read-only apply/whitespace checks against review-latest. The candidate HEAD diff also passed its whitespace check. Neither regression was compiled or executed.
