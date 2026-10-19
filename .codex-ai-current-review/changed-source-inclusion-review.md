# Changed-source native/AI inclusion review

No additional concrete defect found in this bounded case. The suspected old-key/new-key hydration does not occur in the exact current functions.

Reviewed frozen root: e037064b2d763f77b6927de3ffb70ef5b6c53e3a, gate-files-e037-r3 staged guard 02d306cd6779d2e1f58e7f93d83b2c6b3cdef584f112d44d0cf9296064eb61d8. Existing CLI SHA-256 45f9a68caf6f40192365c32754af1b985b698b8c81c87fb8c4b5254a0492fb37 and its Cargo fingerprint were emitted during the frozen root gate build. The guard and executable hash matched before and after all probes; full-gate success is not inferred.

Exact key flow:

- `crates/engine/src/cmd/place/document.rs:95`: `read_file_again` returns no bytes when the source hash differs from the placed link's stored hash.
- `crates/doc/src/placed_document.rs:259`: a full blob or an already-read byte cache supplies the original content by `p.key`. `full_bytes` at line 268 caches a successful read under that same key.
- `crates/engine/src/cmd/place/document.rs:215`: `full_documents` writes hydrated bytes with `out.images.insert(p.key.clone(), ...)` at line 231. It does not derive a replacement content key.
- `crates/engine/src/cmd/links.rs:505`: reopening with the default Ask When Modified reports a changed source and preserves the old placed snapshot. Explicit Update Links reads a Source keyed from the new content; `install_document` at line 355 changes key, dimensions, transform and link together and stores the new blob.
- Native v4 and AI serializers restore the original tree after preparing resources; since preparation uses the existing key, restored nodes and prepared resources still agree.

48 runtime cases passed using only small original shapes: a 40×30 source and an 80×60 proxy parent. The source was changed externally after the CLI session reopened its proxy. Source file sizes were 23,881 and 26,135 bytes. Each scenario checked native and AI × Serialize/Export/Save-with-no-path × includeLinked false/true. AI used pdfCompatible:false; this directly checked its original editing attachment.

| Scenario | includeLinked:true result |
| --- | --- |
| Unchanged source after proxy reopen | Exact original source bytes under the original key |
| Source changed after proxy reopen, no prior byte hydration | Original placed key/metadata/preview retained; changed bytes rejected |
| Original source bytes cached before external change | Exact cached original bytes under the original key |
| Source changed, then explicit Update Links | Updated key/metadata retained; exact new source bytes included |

Every output retained the complete current PlacedDocument metadata, referred to an existing image entry under its own key, and left document.json unchanged. IncludeLinked false retained previews; included full bytes were compared byte-for-byte with the synthetic source file, not merely checked for a non-proxy flag. The changed-file positive control reported one modified link before Update Links.

Artifacts: `check-changed-source-inclusion.py`, `changed-source-inclusion-results.json`, and original fixtures in `changed-source-inclusion-fixtures/`. The script drives the existing CLI through explicit local `mcp --headless` stdio, allowing the fixture file to change between commands without timing races or background threads. No server connection, network call, Cargo command, production edit, refactor, commit or token read was performed.

Limits: single-level synthetic native links with stored hashes, default Ask When Modified plus explicit Update Links; path retargeting, optional preview/PDF artwork and hashless legacy input were not exercised by this probe. This is a scope result, not a global clean claim and not one of the required final frozen fully staffed clean rounds. The earlier background-save/Pen snapshot finding remains a separate preliminary handoff.
