Bounded Image Trace scratch-memory sidecar — c26c4a2

Actionable defect verified and guarded. Preferred cumulative candidate: round3-cumulative-c26.patch.
Resource-only follow-up after the accepted round2 patch: round3-resource-only.patch.
Frozen review snapshot: round3-frozen; hashes and summary: round3-freeze.json.
The frozen snapshot contains c26 trace sources plus the accepted uniform/denoise optimizations and this guard. Parent owns integration and full CI. No commit or remote action occurred.

Finding and bounded reproduction

The 64-Mi-pixel guard applies to raster decoding. The engine's 2,000,000-anchor guard is checked only after a colour layer has constructed all its contours and fitted a path. Neither counts concurrent pixel-indexed contour scratch. Native parallel_map previously admitted min(available cores, active colours) workers regardless of image dimensions.

For a nonuniform layer at width w, height h, P=w*h:

  bool mask                       P bytes
  u32 component labels           4P bytes
  u8 edge grid                   (w+1)*(h+1) bytes
  fixed scratch                  6P+w+h+1 bytes

These coexist after label() returns and the edge grid is allocated. This calculation excludes flood stacks, component metadata, contour/fit vectors, returned paths, allocator overhead, quantization, and the RGBA/label input. It is allocated capacity, not a giant-image RSS measurement or a statement about committed pages.

Original procedural pattern: 16 equal vertical bands, each a distinct colour; Color mode, 16 colours, Abutting, Noise=1, Paths/Corners=100. Every layer is nonuniform, but every band's contour is just a rectangle: 16 paths, 64 anchors. Small output complexity therefore cannot protect against the full-image scratch allocations.

A 64x64 input verified all sixteen nonuniform-layer scratch allocations, each 4096-byte mask + 16384-byte component table + 4225-byte edge grid = 24705 bytes. A local instrumentation-only barrier held the first worker wave at this allocation point. The host reported twelve available workers, and they simultaneously held 296460 bytes of fixed scratch. The trace succeeds at an exact 64-anchor budget. With budget zero, the old implementation still allocated scratch for all sixteen layers before returning TooComplex.

Only the original 64x64 image was allocated in these resource probes. Each probe process had RLIMIT_AS=256 MiB (the existing maximum decoded RGBA footprint), CPU limit 30 seconds, bounded wall timeout, and MALLOC_ARENA_MAX=1. Instrumentation uses safe Rust in an untracked source copy; the shipped source has no probes, barriers, reduced limits, or new assets.

Dimension-only largest accepted square: 8192x8192 = 67108864 pixels, exactly MAX_PIXELS. Each band's four-vertex rectangle and the 64-anchor output are unchanged by scaling this pattern. A nonuniform layer's fixed scratch is 402669569 bytes. At the twelve workers observed by the tiny probe, concurrent fixed scratch is 4832034828 bytes (4.50018 GiB), in addition to 402653184 bytes (384 MiB) of RGBA and u16 labels. These are exact size calculations; no 8192-square raster, decoder payload, benchmark, or OOM experiment was run. The pattern demonstrates an allocation-accounting gap and OOM risk, not a claim that every host would fail.

Guard and why its bound is justified

For positive dimensions, (w-1)*(h-1)>=0 implies w+h<=P+1. Thus the most fixed scratch any one raster at the already accepted pixel limit needs is:

  P + 4P + (P+w+h+1) <= 7P+2
  allowance = 7*MAX_PIXELS+2 = 469762050 bytes (448 MiB + 2 bytes).

This bound comes from the existing pixel limit and actual buffer element sizes; it is not a newly guessed input cap or a machine-memory threshold. layer_worker_limit divides that shared allowance by the image's nonuniform-layer fixed-scratch size, then parallel_map also limits by available workers and layer count. For every decoded image within the existing limit, at least one worker fits and admitted fixed scratch is no greater than one maximum accepted raster's worst footprint. At 8192-square admission is one worker (about 384 MiB of fixed scratch); at 8192x4096 it is two; at 4096-square it is four. Small inputs retain parallelism up to available cores and colours.

The decode limit remains 64 Mi pixels. The engine's anchor budget, trace_within signature/error text, output geometry/order, presets, and the single-threaded dispatcher path remain unchanged. The parallel dispatcher accepts a worker cap. A layer also checks whether another layer has already exceeded the shared anchor budget before allocating a fresh mask. This prevents queued work from allocating buffers for an already failed trace. Concurrent jobs already in flight may still finish; this is cooperative cancellation.

The accepted contour.rs and quantize.rs bytes are exactly unchanged. The uniform rectangle/empty-contour shortcut and uniform/accepted-component denoise optimizations remain intact. No new timing or photographic quality claim is made.

End-to-end scaled-budget check

In a second untracked instrumented copy only, MAX_PIXELS was scaled to 4096 so the same 64x64 input becomes the largest accepted raster for that copy. The production formula admits one worker, sharing a derived 28674-byte allowance. This tests actual tracing admission without allocating a large image or introducing that limit into shipped code.

- Full 64-anchor budget: same sixteen paths and 64 anchors; sixteen scratch allocations occur sequentially. The controlled live fixed scratch drops from 296460 to 24705 bytes.
- Zero-anchor budget: same TooComplex {max:0}; only one layer allocates scratch, compared with sixteen before the guard. Fifteen queued layers stop before new scratch allocation.

Evidence is in round3-scaled-finished.json and round3-scaled-baseline-zero.log, round3-scaled-scaled-full.log, and round3-scaled-scaled-zero.log. Both resource probes completed under their process guards and all source hashes stayed fixed.

Meaningful tests and verification

- Three new shared-code tests allocate no giant input: resource admission at square/skinny/exact dimension boundaries and overflow-sized dimensions; actual dispatch's concurrent-work count and result order under worker caps; no further fitting or anchor accounting after an earlier layer has exhausted the budget.
- The exact c26 trace suite plus all accepted and new changes passes 32 tests, with the existing timing test ignored. Current c26 geometry/pathops and cached external dependencies were used for direct build/test verification.
- Both adversarial tests pass: 432 complete TraceResult comparisons and 96 paired budget comparisons against c26 baseline. These cover alpha boundaries, transparent holes, skinny/near-uniform input, 256-color requests, Noise boundaries, white filtering, both layer methods, and empty/exact/one-over/one-under output budgets.
- cargo clippy --locked -p vectorcraft-trace --all-targets -- -D warnings and cargo fmt -p vectorcraft-trace -- --check pass. These ran only in the assigned worktree, using its existing target and original workspace manifest; that manifest is still version 0.6.0. The direct build above separately validates the c26 dependency source/types (version 0.7.0). No full workspace CI or external dependency rebuild ran.
- Detached controllers were sequential, with CARGO_BUILD_JOBS=1, dev/test debug=0, incremental=0, and the assigned target. Final source guard: 39ade3ec7f45c83afe1708b14226079aea345f69487ddab91f9342d312678ed2. The frozen source, patches, probes, and controller input hashes remain fixed during verification. All controllers and children finished before handoff.
- round3-cumulative-c26.patch applies cleanly to an immutable c26 snapshot and a read-only snapshot of the parent's current staged AI roadmap. Reversing only the trace roadmap changes restores the parent's staged roadmap byte-for-byte. round3-resource-only.patch applies cleanly after the accepted round2 candidate. root source/index were not edited.

Limits and tradeoff

This closes the verified multiplicative fixed-scratch admission gap. It is not a total process-memory cap or an OOM guarantee: flood stacks, component/contour/fitting vectors, denoise/quantization, retained output, document state and other memory are outside this fixed-buffer allowance. A fragmented single layer can still grow contours before the emitted-anchor budget is checked. No speculative replacement threshold or new path/anchor cap was introduced. Large images may use fewer CPU workers; no runtime speed claim was measured. Only Linux/shared-code behavior was verified.

Integration and preserved artifacts

Cumulative changed paths: ROADMAP.md, crates/trace/src/contour.rs, crates/trace/src/quantize.rs, crates/trace/src/lib.rs, crates/trace/src/tests.rs.
Resource-only changed paths: ROADMAP.md, crates/trace/src/lib.rs, crates/trace/src/tests.rs.
Use the artifact patches against c26 or the accepted round2 candidate. The assigned worktree's branch HEAD remains 843f461, so its raw HEAD diff also includes already-merged upstream lib/tests/roadmap changes and is not the portable integration patch.
Original round1/round2 patches, logs, binaries and source snapshots are preserved. round3-assigned-before also preserves the assigned source before this round. The parent controls sequential integration, commit identity/trailer, further UI/CLI review using the frozen cumulative candidate, and full CI. No Adobe apps, source, assets, reference output, external corpus, token access, subagent, staging, commit, push or PR was used.
