Image Trace revalidation against c26c4a2

Ready for the parent's sequential integration. Prefer round2-c26c4a2.patch.
Base: c26c4a20609e8c1019eb98def978f6844082965a.
Patch SHA256: 40d85537d3241e4da14ce21015ec3f7d26aa60a5954927f0e0cfe1e17f822c3d.
Changed paths: ROADMAP.md, crates/trace/src/contour.rs, crates/trace/src/quantize.rs.
177 insertions, 5 deletions; no check artifacts included.

Scope and portability

- Read canonical and assigned-worktree AGENTS.md and the ROADMAP assessment. Required plan files are absent in both locations. Applied the owner's clean-room rule; no Adobe application, code, asset, or reference output was used.
- Assigned worktree remains on 843f461 with exactly the recovered three-file source diff. Original patches, reports, binaries, and logs remain preserved. Only round2-prefixed review artifacts were added in the assigned worktree.
- The earlier round1-latest-main.patch applies cleanly to c26c4a2. The refreshed patch is based explicitly on c26c4a2 and passes git apply --check against both an immutable c26 snapshot and a read-only snapshot of the canonical staged ROADMAP. Both patched snapshots have exactly the assigned contour/quantize source.
- Reversing the four trace ROADMAP edits restores the staged ROADMAP byte-for-byte: the parent's #526 AI text and all unrelated text are preserved. The parent's root sources and index were only read. No root Cargo invocation, staging, commit, push, remote action, PR, or other-worktree edit occurred.
- All trace production source, tests, Cargo manifests, and Cargo.lock at c26c4a2 match the previous main revision 2b321d2. The upstream lib.rs, fitter, mosaic code, and tests remain untouched by the patch. Upstream #552 preset behavior, #557 parallel layer order, 64-Mi-pixel decode guard, and engine anchor budgets are preserved. This patch does not reimplement #525 or #543.
- The old cached rlibs' production source is byte-identical to the current baseline and patched snapshots. Their early tests.rs snapshot predates upstream's three tests, but it was compiled without cfg(test), so that file does not enter the rlibs. The recovered composed test binary's sources match the patched c26 source, with the same upstream test suite plus two local budget-boundary tests. All these source mappings are checked by round2-controller.py.

Adversarial findings

- Uniform contour output has the original start vertex, clockwise winding, signed area, vertex order, pixel count, and empty holes. Nonuniform masks still enter the unchanged walk. Existing exact-oracle tests cover empty masks, single-pixel and skinny dimensions, and a 1024-square mask.
- A valid uniform denoise image has no different opaque neighbour to merge into, even at Noise above image area; returning early preserves its labels, including transparency. Once a nonuniform component reaches Noise, its label will remain unchanged. The patch still visits and marks the rest of that component; it skips only irrelevant neighbour counts, preserving subsequent traversal and tie behavior.
- The recovered denoise oracle covers every 4^6 label arrangement in both orientations, threshold boundaries, transparency, ties, and larger regions. New output checks add alpha 127/128, near-uniform masks whose final pixel is transparent, transparent holes, white filtering, skinny rasters, maximum 256-color requests, and Noise 0, 2, area+1, and u32::MAX. All 432 complete TraceResult debug representations exactly match the c26 baseline (coordinates, handles, subpaths, palette/layer order, colors, and pixel counts).
- Another 96 paired trace_within calls preserve zero, exact, one-under, and one-over anchor-budget outcomes for empty, uniform, skinny, and holed output under both layer methods and both white-filter settings. Errors retain the requested budget. No output, noise, or budget regression was found in these checks.
- New tests use small original procedural inputs, not an external or Adobe corpus. They are local review probes, excluded from the three-file patch. No claim is made about untested photographic inputs, another OS, or other hardware.

Resource assessment and unique performance evidence

- The uniform mask scan precedes component labels, flood stack, and edge-grid allocation. It keeps the existing mask and returns only a rectangle or empty contours. Uniform denoise returns before its seen bitmap, component vector, or flood stack. Accepted nonuniform components stop neighbour counting but still traverse every pixel. These are the unique changes being credited.
- Recovered same-host Linux measurements against #557: 4096-square uniform, Noise=4, peak RSS 258.6 to 115.0 MiB; Noise=u32::MAX, 306.6 to 114.6 MiB. Both runs produced the same one path and four anchors. At this size the input RGBA, u16 labels, and bool mask alone account for 112 MiB, consistent with the shortcut's remaining footprint.
- The recovered 4.98 s to 0.30 s uniform trace timing is one observation, not a repeated or idle-machine budget. No expensive benchmark was repeated in this review. The saved synthetic-gradient measurement is preserved but is not offered as a general photographic speed or memory claim. Its log printed the side length as Noise; round1-measurements-annotated.json records the actual Noise=4.
- Remaining resource concern, pre-existing upstream: each nonuniform layer allocates a bool mask (P bytes), u32 component labels (4P), and an edge grid ((w+1)(h+1)), approximately 6P bytes before flood stacks, component metadata, loop vectors, fitting, and output. parallel_map admits one layer per worker up to available_parallelism; neither the decode-pixel guard nor emitted-anchor budget provides aggregate scratch-memory admission. No large dense-image run or hardware extrapolation is claimed here, and this focused patch introduces no speculative cap.
- Photographic edge quality and tracing on the UI thread are unchanged. ROADMAP keeps photographic quality/performance at scale open, qualifies the old B&W timing, and leaves parity scores unchanged.

Verification and evidence

- Recovered Cargo checks: 26 focused tests passed, one timing test ignored; trace Clippy with -D warnings, trace build, and fmt all passed. The sources remain exactly at their saved guard, so these checks remain evidence for the assigned patch.
- Recovered four parity dumps have the same SHA256 c9a44bf43a62b1d772bcc6e739c99f1a1040b59b6a282118815cdeb4f5b2a08e and exactly 84 fixture/preset lines each.
- First detached controller: probe compilation, two adversarial tests (432 full outputs and 96 budget comparisons), and the recovered composed suite (31 passed, one timing test ignored) all exited 0, using the assigned target's cached artifacts.
- Final dependency audit found that geom gained Live Corners between the assigned worktree base and current main, though trace and pathops do not call the changed shape functions. Closed that portability gap by copying all 17 current geom/pathops source files from c26c4a2 into the assigned review directory and compiling them, both current baseline/patched trace libraries, the exact current patched suite, and the adversarial probe. Every copied source was checked byte-for-byte against the immutable c26 objects. External locked dependencies are unchanged; only workspace package versions changed from 0.6.0 to 0.7.0.
- Final current-source verification: eight build/test steps exited 0; the exact current patched suite passed 29 tests with one timing test ignored, and both adversarial tests passed again with the current geom/pathops libraries (432 complete outputs, 96 paired budget checks). Artifacts and exits are in round2-current2-finished.json and round2-current2-*.log / *.exit.json. This was a sequential rustc build using cached external rlibs; no external dependency rebuild or full workspace CI. CARGO_BUILD_JOBS=1, dev/test debug=0, incremental=0, assigned target were set.
- The initial dependency-build runner selected a cached kurbo 0.11 artifact alongside linesweeper's 0.13, failing before either current trace library was built. Its failed exits/logs remain under round2-current-*. The final round2-current2-controller.py pins the matching kurbo 0.13 artifact, and all steps pass. No shipped code was changed for this verification-runner issue.
- Source diff guard stayed bb590f999b3dad14dae7438e866a93931bdfee9055ff3d084edb8f866dbd7d65 throughout. Snapshot, patch, controller, probe, rlib, and reused test-binary hashes stayed unchanged during the job. All inherited controllers were absent, and the new controller finished before handoff.
- round2-portability.json: current-source hashes, patch checks, staged text preservation.
- round2-finished.json and round2-*.exit.json: initial guarded exit evidence and input hashes.
- round2-current2-finished.json and round2-current2-*.exit.json: final c26 source/dependency build and test evidence, source/input hashes, and external/built rlib hashes.
- round2-adversarial.rs / round2-controller.py / round2-current2-controller.py: reproducible focused probes and final current-dependency builds.
- round2-adversarial.log / round2-composed-recovered.log: test outcomes.
- round2-summary.json: integration summary and exact changed paths.
- round1-review.md, round1-finished.json, round1-verify-finished.json, round1-errors-finished.json, round1-measurements-annotated.json, and their logs: recovered original evidence.

Parent still owns sequential integration, required commit identity/trailer, and full integrated CI. This review establishes Linux/shared-code behavior only.
