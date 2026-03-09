# Bug 15: RadixTree underreports overlap scores vs PositionalIndexer

> **Status: FIXED** — The `find_matches` logic was substantially rewritten in upstream PRs #5973 and #6122. The scoring now properly tracks `matched_depth` along tree paths.

## Summary

The `RadixTree` indexer's `find_matches` returns lower overlap scores than `PositionalIndexer` for the same stored blocks and query sequence. Specifically, when a worker has stored blocks that fully cover a query, RadixTree reports a score of 1 instead of the correct value (e.g., 3 for a 3-block query).

## Severity

**High** — Overlap scores drive the KV-cache-aware routing decision in `DefaultWorkerSelector`. Underreporting scores means the router will not prefer workers that already have cached KV blocks, leading to unnecessary recomputation of KV cache and degraded inference latency.

## Steps to Reproduce

```rust
use dynamo_kv_router::RadixTree;
use dynamo_kv_router::PositionalIndexer;
use dynamo_kv_router::indexer::SyncIndexer;
use dynamo_kv_router::protocols::LocalBlockHash;

// Store blocks covering hashes [6, 0, 5] on worker 0
let mut radix = RadixTree::new();
let positional = PositionalIndexer::new(32);
// ... apply same store events to both ...

let query = vec![LocalBlockHash(6), LocalBlockHash(0), LocalBlockHash(5)];

let r_radix = radix.find_matches(query.clone(), false);
let r_positional = positional.find_matches(&query, false);

// MISMATCH:
// Radix:      {worker_id: 0, dp_rank: 0} => 1   ← WRONG
// Positional: {worker_id: 0, dp_rank: 0} => 3   ← CORRECT
```

### Reproduction with fuzz_triple_differential:

```bash
cd lib/kv-router/fuzz
cargo +nightly fuzz run fuzz_triple_differential \
    artifacts/fuzz_triple_differential/crash-f79bf57988d52dd105b554a1f070a6c593284171
```

## Root Cause

The RadixTree traversal in `find_matches` appears to only count a single block hash match per worker when it should be counting all matching blocks along the tree path. The PositionalIndexer correctly counts all matching positions.

Three independent crash inputs all show the same pattern:
- RadixTree reports score = **1** where PositionalIndexer reports score = **3** (the full query length)
- The ConcurrentRadixTree agrees with RadixTree (same underlying tree logic)
- Only PositionalIndexer gives the correct score

The RadixTree's `find_matches` walks children from root, tracking `matched_depth` as it descends. It only increments depth for nodes it visits. If the tree structure doesn't form a contiguous path matching the query (e.g., due to how events are applied), some workers may appear only at the first node and drop out, getting score 1 instead of the correct depth.

The PositionalIndexer uses a position-based lookup (`jump_search_matches`) where each block is indexed by its position in the sequence, so it correctly counts all positions where a worker has matching blocks.

The root cause is likely in `RadixTree::apply_event` — the tree structure after Store events may not correctly represent the full sequence, causing `find_matches` to see only partial matches.

Code: `lib/kv-router/src/indexer/radix_tree.rs:156-314`.

## Crash Artifacts

- `fuzz/artifacts/fuzz_triple_differential/crash-f79bf57988d52dd105b554a1f070a6c593284171` — Query `[6, 0, 5]`, Radix=1 vs Positional=3 for worker 0
- `fuzz/artifacts/fuzz_triple_differential/crash-33160f5734e5d06e6af9cabf3030ca138df58123` — Query `[6, 2, 5]`, Radix=1 vs Positional=3 for worker 1
- `fuzz/artifacts/fuzz_triple_differential/crash-9878cebe1a3d641646d8d57279b70f8f3e3f0a4d` — Query `[2, 0, 1]`, Radix=1 vs Positional=3 for worker 0

## Suggested Fix

Investigate `RadixTree::find_matches` in `lib/kv-router/src/indexer/radix_tree.rs`. The tree traversal likely needs to accumulate the number of matching blocks along the path from root to leaf, not just count the number of matching tree nodes (which may each represent multiple blocks).

Found by: `fuzz_triple_differential` fuzzer.
