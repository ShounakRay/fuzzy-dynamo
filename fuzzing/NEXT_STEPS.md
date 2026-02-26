# Next Steps: Fuzzing Infrastructure

## Status

All 27 fuzz targets have been run across 5 crates. 6 crash bugs confirmed with artifacts. 12 additional bugs found by code audit with regression tests. The infrastructure is complete and working.

---

## Priority 1: Fix Confirmed Bugs

These are confirmed crashes with minimal reproducers — ready for upstream fixes.

### Critical
- **Bug 18**: TwoPartCodec integer overflow (`two_part.rs:58`) — network-facing, wraps silently in release builds. Fix: `checked_add()` on `24 + header_len + body_len`.

### High
- **Bug 16**: RadixTree reentrant borrow (`radix_tree.rs:371`) — move the self-reference check BEFORE `block.borrow()`, not after. Alternatively, use `try_borrow()` at line 371.
- **Bug 17**: `to_block_level` division by zero (`protocols.rs:365`) — add `if block_size == 0 { return vec![]; }`.
- **Bug 15**: `compute_block_hash_for_seq` panic on `kv_block_size=0` — add zero-guard.

### Medium
- **Bug 1**: Streaming prefix-matching content loss — rework `force_reasoning` prefix buffering.
- **Bugs 3-5, 7**: Parser data corruption bugs — case-insensitive matching, boundary-aware replacement.

---

## Priority 2: Upstream Engagement

### File Issues
For each confirmed crash bug, file with:
- Minimal reproducer (6-24 bytes)
- Root cause analysis
- Suggested one-line fix

### Propose Fuzzing CI
- This repo has zero existing fuzzing infrastructure (confirmed)
- Our infrastructure covers 260k LOC across 5 crates
- Offer as PR: fuzz crates + unified runner + CI integration

### Cross-reference Existing Issues
- [#3112](https://github.com/ai-dynamo/dynamo/issues/3112): Our Bug 17 is the same class
- [#3393](https://github.com/ai-dynamo/dynamo/issues/3393): Our Bug 1 is a specific instance
- [#6147](https://github.com/ai-dynamo/dynamo/issues/6147): Our TCP fuzzing validates the fix

---

## Priority 3: Extended Fuzzing Campaigns

### Longer Runs on High-Value Targets
The current runs were 2-5 minutes each. Longer runs (30+ min) on stateful targets may find deeper bugs:
- `fuzz_radix_tree_events` — already found a crash in 2 min, more variants likely
- `fuzz_two_part_decode` — overflow found fast, but other codec paths need deeper exploration
- `fuzz_differential` — found bugs in 2 different runs, more parser-specific differentials possible

### Coverage-Guided Seed Refinement
Use `fuzzing/coverage.sh` to identify uncovered branches, then craft targeted seeds to force execution through them. More efficient than blind long runs.

---

## Priority 4: Creative Testing Approaches

### Property-Based Testing (proptest)
For RadixTree:
- **Commutativity**: Independent store events shouldn't affect query results regardless of order
- **Idempotency**: Storing the same blocks twice shouldn't change the tree
- **Monotonicity**: Adding blocks should never decrease overlap scores

### Concurrency Stress Testing
`PositionalIndexer` uses `DashMap` (concurrent hash map). Race conditions possible:
- Concurrent store + query
- Concurrent store + remove for same worker
Requires `loom` or `shuttle` framework.

### Release-Mode Overflow Testing
Bug 18 (TwoPartCodec) panics in debug but WRAPS in release. The release-mode behavior is potentially worse — it could bypass size checks and cause memory corruption. Test with release-mode fuzzing specifically.

---

## Priority 5: Expand to More Crates

### SSE Codec (dynamo-llm)
If `dynamo-llm` compiles as fuzz dependency: add `fuzz_sse_decode` and `fuzz_sse_streaming`. Our 10 regression tests cover critical paths but a fuzzer would find deeper state machine bugs.

### Distributed Protocol Fuzzing
NATS and ZMQ transports in `dynamo-runtime` are network-facing but require mock server harnesses.

---

## Decision Framework

Prioritize by:
1. **Impact**: Network-facing crash > data loss > cosmetic
2. **Effort**: Running existing targets > writing new ones > building infrastructure
3. **Confidence**: Confirmed + artifact > audit-identified > theoretical
4. **Upstream value**: Bugs in production paths > latent bugs in unused code
