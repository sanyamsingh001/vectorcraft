Reviewed `files-current` on `e037064b2d763f77b6927de3ffb70ef5b6c53e3a`, now the exact frozen root candidate for `gate-files-e037` (controller 2320022). Gate source diff is `5a7d3496534311f42d68270b78235eded4090f6ebbbf96abc1f58e65b2a8b792`. The overall candidate changed while the parent staged it; the reviewed AI/native source hashes stayed unchanged and now match frozen root. This is a narrow source and bounded synthetic review, with one concrete remaining defect. It is not a clean final round or a global gap-free conclusion.

P2 NATIVE-INCLUDE-MISSING-PLACED-LINK: `crates/engine/src/cmd/fileio/native.rs:136` saves the output-prepared document whenever Include Linked Files is true. For a reopened parent holding a preview of a source that is now missing, `full_documents` substitutes an unlinked Image. Version 4 supports PlacedDocument, but native serialization receives that substituted tree and silently saves the Image. This affects native/template Save, Serialize and Export. It is a pre-existing residual that remains in this candidate; the native-default version fix alone does not cover it.

Negative proof used the existing CLI (SHA-256 `96ddec3800447387d468a518e1993f67eeeeac313ec154a1c81a48fda89570fe`) and an original 40x30 source/80x60 parent. The full native encoder, placed preparation, model, links and format sources used for this path match files-current exactly. No latest AI/PDF-layer binary behavior is inferred. `native-negative.log` and `native-negative.exit` record the expected regression failure (exit 1):

```text
includeLinked:false -> version 4, placeddocument, link present, warnings [], live source unchanged
includeLinked:true  -> version 4, image, link absent, warnings [], live source unchanged
AssertionError: includeLinked:true discarded a missing PlacedDocument link in current native v4
```

`native-include-negative.json` additionally records 36 bounded cases across native/template/AI, Save/Serialize/Export, source available/missing and inclusion false/true. All six native/template missing-source inclusion cases lose the link. Available inclusion carries the exact original source bytes. AI retains placed metadata and missing proxies with either inclusion flag on the prior matching core; AI's new layer default was reviewed separately in current source.

Portable patch: `native-missing-linked-ready.patch` (SHA-256 `0638f8effa2b649e6d965d4e06511ae4d638812c25914a46e736db480578c2ef`). Exact changed paths are `crates/engine/src/cmd/fileio/native.rs` and `crates/engine/src/tests_placed_document.rs`. It keeps the output-prepared document for optional native preview/PDF rendering, then uses hydrated images on the original tree for current-version inclusion. The older-version branch retains its existing flattening behavior. Existing AI helpers and current createLayers handmerge are unchanged. Patch applies cleanly to files-current and the earlier review-current (`git apply --check`, exit 0); scoped rustfmt and diff checks pass. Only the two corresponding source files in the recovered ai-archived-c26 worktree were edited; candidate/root/index were not edited.

Meaningful regression `native_included_links_keep_missing_placed_documents_editable` reopens preview-only original fixtures, checks native/template Save/Serialize/Export with inclusion true/false and source missing/available, inspects raw v4 files for exact placed metadata/proxy status/source bytes, verifies the live document is unchanged, and reopens missing-source outputs to check successful relinking. It is added to the portable patch but has not been compiled or run. After parent integrates it and authorizes checks, run:

```sh
cd /mnt/downloads/dev/artcraft/vectorcraft
cargo test -p vectorcraft-engine --lib native_included_links_keep_missing_placed_documents_editable
```

The requested AI contracts are preserved in current source: encode_all normalizes aliases before preparation; ai_native copies hydrated images onto the original tree; ai_options parses includeLinked/embedProfiles/compress/pdfCompatible before subsets can skip the callback; AI participates in useArtboards:false visible-bounds recursion; native catalog and command specification now default to actual version 4. Current tests include the direct 12-alias public-encoder case, eager validation, raw inclusion payload, visible-bounds and independent native-default regressions. Their current-base execution remains parent-owned.

The layer handmerge matches upstream e037064: in shared encode_ai, absent/null `standard` supplies createLayers:true only when that key is absent. Explicit createLayers:false is retained; an existing createLayers:null retains upstream PDF null/preset semantics. Save, Export and Serialize all reach this helper. The existing upstream test `cmd::fileio::tests_pdflayers::ai_files_write_their_layers_and_sublayers_as_pdf_layers` remains intact and inspects imported PDF pages/layers independently of native editing data, including hidden/non-printing layers, hidden sublayers and explicit false. Parent can extend its call table to Save/Serialize and standard:null if needed; no production layer defect is established by this review.

No new Cargo build or test process, token read, commit or remote action. No Adobe software, code or assets and no GPL/AGPL copying were used. Current fullCI is pending. The earlier c26 gate was canceled because upstream core merged, not because of a failure. Final fully staffed clean rounds remain parent-owned after composition and must not credit this finding-bearing review as clean.
