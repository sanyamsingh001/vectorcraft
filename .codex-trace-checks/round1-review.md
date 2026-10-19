Adversarial sweep round 1 — Image Trace (#543 / #525)

Final source changes: ROADMAP.md, crates/trace/src/contour.rs, crates/trace/src/quantize.rs.
No commit, staging, push, PR publication, or subagents. No Adobe software, source or assets.

Upstream and patch scope

- Worktree HEAD is 843f461; local origin/main was af7a523 when compared. Remote main at final inspection is 2b321d240f368f19e82c93ed927dbaafb6636a05.
- #543 was resolved by upstream #552: Make and Make and Expand immediately use Default; preset choices remain in the Control bar, Properties and Image Trace panel. Removed the worker's superseded modal dropdown changes and its failing MouseWheel test (missing phase).
- #557 merged during review as d7b4d85d3ccb5a1e3d70deb34dc26bd9c7badd93. Tested head 06242cd3ab7f4fddaa449a98cd9097fc23700001 is unchanged; latest main's trace lib equals that tested source.
- Removed overlapping decode/output guards and arbitrary 16-MP / 50k-path / 250k-anchor limits; kept the original API so it composes with upstream's trace_within. Latest main's contour.rs and quantize.rs still exactly equal the original worktree base, so these changes are unique.
- Original worker patch is preserved in round1-original.patch. round1-unique.patch is against the worktree HEAD. Prefer round1-latest-main.patch when integrating onto fresh main: it preserves upstream's #543/#557 ROADMAP updates and passed git apply --check against the cached latest-main snapshot. Only the three source files are in either patch; check artifacts are excluded.

Unique verified improvements

- Uniform masks return the exact original rectangle or empty contour, avoiding the component table, flood stack and edge grid. Loop order, vertices, winding, holes and pixel counts match the original walk exactly, including empty and skinny masks.
- Uniform denoise labels return unchanged before scratch allocation, including Noise greater than image area and transparent labels.
- Denoise stops counting neighbouring colours after a component meets Noise; later counts cannot affect its unchanged label. Differential tests cover all 4^6 small label arrangements in both orientations, threshold boundaries, ties, transparency and larger regions.
- ROADMAP keeps photographic quality and performance at scale open and qualifies the historical 1k² B&W timing. No parity score increase.

Verification

- cargo test -p vectorcraft-trace --all-targets: 26 passed, one timing test ignored.
- cargo clippy -p vectorcraft-trace --all-targets -- -D warnings: passed.
- cargo build -p vectorcraft-trace and cargo fmt --all -- --check: passed.
- 84 complete fixture/preset combinations: byte-for-byte equal outputs across original, candidate, #557, and #557 plus optimizations. Fixtures include uniform, transparent, quadrants, transparent gaps, a gradient, and fragmented colours; all 12 presets plus Noise beyond image area and no-denoise settings.
- Isolated #557 source composed with the unique changes: 31 tests passed, one timing test ignored. Includes upstream oversized-header refusal, parallel layer ordering and anchor-budget refusal, plus zero/exact/one-over boundary tests and empty trace with zero budget.
- Source guard stayed bb590f999b3dad14dae7438e866a93931bdfee9055ff3d084edb8f866dbd7d65 during every completed check. Initial inherited controller/child were absent from /proc, and no assigned-worktree checker was active before editing. All new controllers finished before handoff.
- Single-job Cargo, dev/test debug=0, incremental=0, assigned worktree's separate existing target. Reused its artifacts; no duplicate full build. Full cargo xtask ci remains the parent's gate.

Measured resource outcomes against #557 on this Linux host (12 available CPUs)

| Synthetic input | #557 peak RSS | With unique changes | Geometry |
|---|---:|---:|---|
| 4096² uniform, Noise=4 | 258.6 MiB | 115.0 MiB | Same one path / four anchors |
| 4096² uniform, Noise=u32::MAX | 306.6 MiB | 114.6 MiB | Same one path / four anchors |
| 2048² synthetic gradient, High Fidelity Photo | 448.6 MiB | 427.7 MiB | Same 176 paths / 1844 anchors |

Uniform trace time in this probe fell from 4.98 s to 0.30 s at Noise=4. These are synthetic observations on the current host, not general photographic performance budgets. wait4 measured each probe process separately. At 4096², RGBA + labels + mask alone account for 112 MiB, matching the optimized result's scale. The photo probe's text mistakenly printed its side as Noise; the actual preset Noise was 4, as recorded in round1-measurements-annotated.json.

What still warrants work

- Tracing still runs on the UI thread; photographic edge quality is unchanged. Neither issue is claimed fixed here.
- Upstream's 64-MP input and emitted-anchor budgets do not cap aggregate contour scratch allocation. Each nonuniform worker allocates approximately 6 bytes/pixel for mask, component labels and edge grid; 12 concurrent workers at 64 MP can account for about 4.5 GiB before flood stacks, contour vectors, input and output. This follows from the inspected allocations; a dense 64-MP runtime peak was not exercised. Aggregate worker memory admission remains a separate concern; no speculative API cap was introduced.
- Full integrated cargo xtask ci and broader platform/hardware verification remain for the parent. No other-OS fix was attempted.

Evidence: round1-finished.json, round1-verify-finished.json, round1-errors-finished.json, round1-measurements-annotated.json; corresponding logs and parity dumps are in this untracked directory.
