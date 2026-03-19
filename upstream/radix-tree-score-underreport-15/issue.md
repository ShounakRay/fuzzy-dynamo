# [BUG]: RadixTree underreports overlap scores vs PositionalIndexer

> **Status: FIXED** — The `find_matches` logic was substantially rewritten in upstream PRs #5973 and #6122. The scoring now properly tracks `matched_depth` along tree paths.

### Describe the Bug

The `RadixTree` indexer's `find_matches` returns lower overlap scores than `PositionalIndexer` for the same stored blocks and query sequence. Specifically, when a worker has stored blocks that fully cover a query, RadixTree reports a score of 1 instead of the correct value (e.g., 3 for a 3-block query).

Overlap scores drive the KV-cache-aware routing decision in `DefaultWorkerSelector`. Underreporting scores means the router will not prefer workers that already have cached KV blocks, leading to unnecessary recomputation of KV cache and degraded inference latency.

The RadixTree traversal in `find_matches` only counts a single block hash match per worker when it should be counting all matching blocks along the tree path. The tree structure after Store events may not correctly represent the full sequence, causing `find_matches` to see only partial matches. The PositionalIndexer uses a position-based lookup (`jump_search_matches`) where each block is indexed by its position in the sequence, so it correctly counts all positions where a worker has matching blocks.

### Steps to Reproduce

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

Alternatively, reproduce with the fuzz artifact:

```bash
cd lib/kv-router/fuzz
cargo +nightly fuzz run fuzz_triple_differential \
    artifacts/fuzz_triple_differential/crash-f79bf57988d52dd105b554a1f070a6c593284171
```

### Expected Behavior

RadixTree should report the same overlap scores as PositionalIndexer: score of 3 for a 3-block query that is fully covered by cached blocks.

### Actual Behavior

RadixTree reports score = 1 where PositionalIndexer reports score = 3. The ConcurrentRadixTree agrees with RadixTree (same underlying tree logic); only PositionalIndexer gives the correct score.

### Environment

On commit `57e6a79f5` in `dynamo-runtime` @ `lib/kv-router/src/indexer/radix_tree.rs`

### Additional Context

Three independent crash inputs all show the same pattern:
- `crash-f79bf57988d52dd105b554a1f070a6c593284171` — Query `[6, 0, 5]`, Radix=1 vs Positional=3 for worker 0
- `crash-33160f5734e5d06e6af9cabf3030ca138df58123` — Query `[6, 2, 5]`, Radix=1 vs Positional=3 for worker 1
- `crash-9878cebe1a3d641646d8d57279b70f8f3e3f0a4d` — Query `[2, 0, 1]`, Radix=1 vs Positional=3 for worker 0

The tree traversal likely needs to accumulate the number of matching blocks along the path from root to leaf, not just count the number of matching tree nodes (which may each represent multiple blocks).

Found via fuzzing with cargo-fuzz / libfuzzer.

### Screenshots

_No response_
