# Preliminary review-e037 cross-boundary review

Candidate: `/mnt/downloads/dev/artcraft/vectorcraft/.git/codex/worktrees/review-e037` on `e037064b2d763f77b6927de3ffb70ef5b6c53e3a`. Observed staged-diff guard `fd071263a6e44ff6e5faabf9537afeed66b69802b7b5225f04a6fa7338886a70`; the manifest still records `27c988d9935f6d80597829a81aa3ad74b5cf1f74012b0223718dc609e25a539d`. The relevant file hashes are unchanged from the preceding manifest-matched snapshot. The composition is evolving and this is not a final clean round. Candidate/root/gate sources were not edited; no Cargo or remote actions were run.

## P2 BACKGROUND-SAVE-LINK-PEN-SNAPSHOT

A native source's Background Save completion refreshes its placed object in another open document. If that placing document is midway through a Pen drag, the next Pen preview restores its older interaction snapshot and discards the refreshed source key and link metadata. The document can return to the previous linked artwork after briefly receiving the new artwork.

Normal workflow: place an original native file, open and recolour the source, start Background Save, switch to the parent and drag a second Pen anchor, let save completion arrive, then continue dragging. The ready test orders the same job creation/write/completion operations without sleeps or threads.

Source proof:
- `crates/ui-egui/src/io.rs:344` snapshots the job; the background callback at line 364 calls `done.complete` against the current session.
- `crates/engine/src/cmd/fileio/save.rs:482` completion invokes `refresh_placed` at line 493.
- `crates/engine/src/cmd/links.rs:589` switches the active index directly, updates the placing document, and restores the index without finishing its tool or interaction.
- `crates/engine/src/cmd/links.rs:690` updates through `Session::edit`; `install_document` at line 355 changes the placed key/link fields on the live document.
- `crates/engine/src/lib.rs:1097` edits inside an interaction without replacing its base snapshot. `Session::preview` at line 1191 restores that older base.
- `crates/tools/src/pen.rs:221` continues the second-anchor drag with a replacement preview.

Evidence level: source-confirmed with a precise regression ready. Current-candidate runtime reproduction and an executed failing regression are pending. This is residual composed behavior, not an attribution to the native IncludeLinked correction.

`review-e037-background-link-pen-test.patch` changes only `crates/engine/src/tests_placed_document.rs`. It uses the existing original-fixture helpers and the real SaveJob encoder/writer/completion path. The positive control requires completion to change the placed key; the final assertion requires the following drag/up to retain the complete refreshed PlacedDocument. Expected negative: final equality fails on the reviewed source. This expected failure has not been executed. Run the filtered engine test after applying the test-only patch once builds are permitted.

A production correction needs to preserve the refresh across the interaction or finish the interaction before updating, with deliberate tool and Undo semantics. No unverified production patch is proposed.

## Bounded checks

The native IncludeLinked correction remains integrated with the native encoder SHA previously rechecked (`a2bed7a64b3f152319994cf23df7398335f29e8a05e9c617182ad231c1fb058e`). Optional PNG/PDF extras use the prepared document; native version 4 restores the original node tree and retains prepared resources; older explicit versions retain the legacy branch. These are source checks, not new optional-output runtime results.

The public aliases, eager AI boolean validation, original placed resources, useArtboards:false path, native version-4 defaults, and upstream standard-absent/null createLayers default were checked in the cumulative paths. Save/export snapshots, range validation and modal cancel routing yielded no additional concrete finding in this bounded review. This is not a global gap-free claim. Trace numerical behavior and pending feature/navigation composition remain with their corresponding scopes.

Test artifact parsing/formatting and read-only patch application/whitespace checks passed. No Rust compilation or regression execution was run. Parent test command once builds are allowed:

```sh
cargo test -p vectorcraft-engine --lib tests_placed_document::background_save_link_refresh_survives_next_pen_preview -- --exact
```
