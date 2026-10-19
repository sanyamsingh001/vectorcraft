# Vector source recovery — 2026-10-19

UNTESTED recovery snapshots created after development was stopped. These separate variants are not merged candidates or validated releases. No recovery CI was run.

## Current source

| Source | Exact recovery commit | Tree |
|---|---|---|
| Guide + Type current320b | [0f5ff98afd9ab3e02700e36e23995047f06b2493](https://github.com/sanyamsingh001/vectorcraft/tree/0f5ff98afd9ab3e02700e36e23995047f06b2493) | e2ccdaf3c6a13a59c87b78498ef63d2a0c5c4e36 |
| Picker currenta466 | [e63d46ea178404e7d84eafa79eee4891293cc105](https://github.com/sanyamsingh001/vectorcraft/tree/e63d46ea178404e7d84eafa79eee4891293cc105) | 1423762bff64b75c38760099e91839d2609e53fb |
| Average/Join currenta466 | [6949d9da082f3dfa2149e0d0199aec9db5d3770b](https://github.com/sanyamsingh001/vectorcraft/tree/6949d9da082f3dfa2149e0d0199aec9db5d3770b) | 52f15e2b278cbd06a710cbf10905cbdcdb59c328 |
| Canonical320b source | [6382250c092c554101cfee3a35f1025387596895](https://github.com/sanyamsingh001/vectorcraft/tree/6382250c092c554101cfee3a35f1025387596895) | 15653caa6007b91c8cc8fbe08aed5135802b5f4c |
| Published Type tree | [f0e3686e20534a0e80f5b10e3fbffba1d1ccc19d](https://github.com/sanyamsingh001/vectorcraft/tree/f0e3686e20534a0e80f5b10e3fbffba1d1ccc19d) | 06dff1aef861a800cee2b07da30146c974b03d80 |

## Coverage and limits

- 403 source/proposal recovery refs were read back at their exact OIDs before this index was published. branch-map.json maps every branch to commit/tree/origin.
- 347 distinct orphan source snapshots plus priority sources and current Picker are preserved. Additional immediately available original-tip source trees are included where needed.
- 1356 distinct source patch/Rust blobs (1777 original path references) are retained on [the immutable proposal snapshot](https://github.com/sanyamsingh001/vectorcraft/tree/d11705b4d130780ce636bdcf23a7919d67270a6d).
- 107 original tip identities and exact raw commit bytes are recorded in historical-tips.json, with exact upstream/fork ref matches where observed. Orphan snapshots do not preserve complete ancestry. No absent history was fetched.
- Physical capture succeeded for105 of283 registered worktrees;173 were absent and5 errored during concurrent external removal/change.281 of282 surviving administrative indexes were recovered separately. omissions.json and source inventories name exact paths and exceptions.
- Missing recorded tree IDs do not by themselves prove unique source loss: many variants remain in indexed snapshots or source patch/fixture blobs. Unique unstaged/untracked bytes removed before capture cannot be claimed recovered.
- Credentials, logs, caches, build artifacts and private-only rendered captures are excluded. No original checkout was reset or deleted by this recovery.

Current Guide product is validation source tree e2ccdaf3c6a13a59c87b78498ef63d2a0c5c4e36. The similarly named priority00 branch is canonical/base320b, not the dirty Guide product.
