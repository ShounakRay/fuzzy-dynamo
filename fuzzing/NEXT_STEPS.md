# Next Steps: Fuzzing Infrastructure

## Priority 1: Validate and Reproduce High-Confidence Bugs

**Why first**: We identified crash paths in the KV router that are very likely real bugs but haven't been confirmed at runtime. Running the fuzzers against these will either produce crash artifacts (proof) or reveal that the code has hidden guards we missed.

### Actions

1. **Run `fuzz_block_hash_computation` with `kv_block_size=0` seed**
   - Expected: `chunks_exact(0)` panic in `compute_block_hash_for_seq`
   - If confirmed: File upstream issue with reproducer
   - Duration: 10 seconds is enough — the bug should trigger immediately from seeds

2. **Run `fuzz_request_extra_info` with `block_size=0` seed**
   - Expected: Division by zero in `to_block_level`
   - Same upstream correlation as [#3112](https://github.com/ai-dynamo/dynamo/issues/3112)

3. **Run `fuzz_two_part_roundtrip` with `FUZZ_OVERFLOW_CHECKS=1`**
   - Expected: Integer overflow in `total_len = 24 + header_len + body_len`
   - If confirmed: This is a wire protocol bug affecting all inter-service communication

### Why this matters
Crash bugs in network-facing code (KV router, runtime codecs) are higher severity than parser bugs because they can be triggered by untrusted input from external services, not just malformed LLM output.

---

## Priority 2: Extended Fuzzing Runs on New Targets

**Why second**: The new fuzz targets have only been smoke-tested (compilation check). Running them for real will likely find bugs in the first few minutes, as the differential fuzzer did for parsers.

### Actions

1. **KV Router targets** — 15-30 minutes each
   ```bash
   FUZZ_CRATE=kv-router FUZZ_TIMEOUT=900 ./fuzzing/run.sh
   ```
   Focus on: `fuzz_radix_tree_events` (state machine — most complex), `fuzz_positional_indexer`

2. **Runtime codec targets** — 10-15 minutes each
   ```bash
   FUZZ_CRATE=runtime FUZZ_TIMEOUT=600 ./fuzzing/run.sh
   ```
   Focus on: `fuzz_two_part_roundtrip` (round-trip oracle finds logic bugs, not just crashes)

3. **Tokens hash targets** — 5-10 minutes each
   ```bash
   FUZZ_CRATE=tokens FUZZ_TIMEOUT=300 ./fuzzing/run.sh
   ```
   Focus on: `fuzz_token_block_sequence` (stateful operations — most likely to find bugs)

4. **Parser new oracles** — 10 minutes each
   ```bash
   FUZZ_CRATE=parsers FUZZ_TARGET=fuzz_content_preservation FUZZ_TIMEOUT=600 ./fuzzing/run.sh
   FUZZ_CRATE=parsers FUZZ_TARGET=fuzz_streaming_monotonicity FUZZ_TIMEOUT=600 ./fuzzing/run.sh
   ```

### Why this matters
Every new bug class we added (round-trip, state machine, streaming monotonicity) is a new opportunity the original crash-only harnesses missed. The differential parser fuzzer found a real bug in ~10 seconds — the new oracles may do the same.

---

## Priority 3: Creative Bug-Finding Strategies

**Why third**: Standard fuzzing covers the "known unknowns." Creative approaches find the "unknown unknowns."

### 3a: Adversarial Seed Generation

Instead of random mutations, craft seeds that specifically target known weak patterns:
- **Integer boundary seeds**: `u32::MAX`, `u32::MAX - 23` (just under header size), `u32::MAX / 2` for overflow in `a + b`
- **UTF-8 torture seeds**: 4-byte sequences, overlong encodings, truncated multibyte chars at chunk boundaries
- **State machine reset sequences**: Store → Clear → Query (was the query state cleaned up?)

### 3b: Coverage-Guided Seed Prioritization

After initial runs, use `fuzzing/coverage.sh` to identify which source lines are NOT covered. Write targeted seeds that force execution through uncovered branches. This is more efficient than blind fuzzing.

### 3c: Property-Based Testing (proptest)

For the KV router's `RadixTree`, write proptest properties:
- **Commutativity**: Order of independent store events shouldn't affect query results
- **Idempotency**: Storing the same blocks twice shouldn't change the tree
- **Monotonicity**: Adding blocks should never decrease overlap scores for existing queries

These catch subtle logic bugs that crash-only fuzzing misses entirely.

### 3d: Concurrency Stress Testing

The `PositionalIndexer` uses `DashMap` (concurrent hash map). Race conditions are possible:
- Concurrent store + query
- Concurrent store + remove for same worker
- Concurrent clear + find_matches

This requires `loom` or `shuttle` rather than `cargo-fuzz`.

---

## Priority 4: Upstream Engagement

**Why fourth**: After validating bugs, share findings with the upstream project.

### Actions

1. **File upstream issues** for each confirmed bug with:
   - Minimal reproducer (seed file + fuzz target)
   - Root cause analysis
   - Suggested fix

2. **Propose fuzzing CI integration** — offer the fuzzing infrastructure as a PR:
   - The repo has zero fuzzing (confirmed by GitHub search)
   - 260k LOC Rust with 311 `unwrap()` calls in the parser alone
   - CI integration would prevent regression of fixed bugs

3. **Cross-reference existing issues**:
   - [#3393](https://github.com/ai-dynamo/dynamo/issues/3393): Our Bug 1 (streaming content loss) is a specific instance
   - [#3399](https://github.com/ai-dynamo/dynamo/issues/3399): Our regression tests directly address "unrealistic tests"
   - [#6147](https://github.com/ai-dynamo/dynamo/issues/6147): Our TCP fuzzing validates the fix in [PR #6393](https://github.com/ai-dynamo/dynamo/pull/6393)

---

## Priority 5: Code Quality and Maintenance

### 5a: Modularize Fuzz Utilities

~560 lines of duplication exist across fuzz targets (event construction, streaming chunk logic, parser type arrays). Extract into shared utility modules:
- KV router event helpers → `lib/kv-router/fuzz/src/helpers.rs`
- Parser streaming chunker → `lib/parsers/fuzz/src/streaming.rs`
- Parser type constants → `lib/parsers/fuzz/src/types.rs`

### 5b: Coverage Analysis

Run `fuzzing/coverage.sh` on all crates to identify:
- Dead code that fuzzing can't reach
- Complex branches that need targeted seeds
- Code paths with no test coverage at all

### 5c: Continuous Fuzzing Setup

Set up OSS-Fuzz or ClusterFuzzLite for continuous fuzzing:
- Auto-runs all fuzz targets on every commit
- Maintains growing corpus across runs
- Alerts on regressions

---

## Priority 6: Expand to More Crates

Lower priority because these have heavier dependency chains, but still valuable:

### 6a: SSE Codec Fuzz Target (dynamo-llm)

If `dynamo-llm` can be compiled as a fuzz dependency (heavy: kube, axum, nats, zmq), add:
- `fuzz_sse_decode`: Feed arbitrary bytes to `SseLineCodec::decode`
- `fuzz_sse_streaming`: Test state machine with interleaved field types

Fallback: The regression tests we already added cover the most critical edge cases.

### 6b: Distributed Protocol Fuzzing

The `dynamo-runtime` transports include NATS and ZMQ protocols. These are network-facing and high-value but require complex test harnesses (mock NATS server, etc.).

---

## Decision Framework

When deciding what to work on next, prioritize by:

1. **Bug confidence**: Confirmed > likely > possible > latent
2. **Severity**: Network-facing crash > data loss > cosmetic
3. **Effort-to-value ratio**: Running existing targets > writing new ones > building infrastructure
4. **Upstream impact**: Bugs that affect production users > latent bugs in unused code paths
