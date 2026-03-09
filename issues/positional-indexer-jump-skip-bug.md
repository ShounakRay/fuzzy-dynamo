# Bug 4: PositionalIndexer: Jump optimization skips removed blocks, producing incorrect scores

## Summary

`PositionalIndexer::jump_search_matches` can report inflated match scores when blocks have been removed from the middle of a stored sequence. The jump optimization skips intermediate positions, missing the gap left by the removal, and incorrectly reports a full prefix match.

## Severity

**High** — This is a correctness bug in the query path. It causes the router to return wrong overlap scores, which directly affects worker selection during KV cache routing.

## Steps to Reproduce

```rust
use dynamo_kv_router::PositionalIndexer;
use dynamo_kv_router::indexer::SyncIndexer;
use dynamo_kv_router::protocols::*;

let indexer = PositionalIndexer::new(32); // jump_size = 32
let mut worker_blocks = FxHashMap::default();

// 1. Store worker 1 with blocks [6, 2, 5] at positions 0, 1, 2
let store_event = make_store_event(/*worker*/ 1, /*event_id*/ 0, &[6, 2, 5], None);
indexer.apply_event(&mut worker_blocks, store_event);

// 2. Remove the block at position 1 (local_hash=2)
let remove_event = make_remove_event(1, 1, &[seq_hash_of_block_at_pos_1]);
indexer.apply_event(&mut worker_blocks, remove_event);

// 3. Query [6, 2, 5]
let query = vec![LocalBlockHash(6), LocalBlockHash(2), LocalBlockHash(5)];
let result = indexer.find_matches(&query, false);

// BUG: PositionalIndexer reports score 3 (full match)
// EXPECTED: score 1 (only position 0 matched; position 1 was removed)
assert_eq!(result.scores[&worker1], 3); // wrong!
```

Crash artifact: `fuzz/artifacts/fuzz_triple_differential/crash-33160f5734e5d06e6af9cabf3030ca138df58123`

Input bytes: `06 2a 3d 3d 3d 24 3d 0e 2a 3d 3d 3d 3d ff 06 2a 3d 3d 3d 24 3d 0e 2a 3d 3d 3d 3d ff ff ff ff ff`

## Root Cause

In `positional.rs`, `jump_search_matches` (line ~619):

```rust
while current_pos < len - 1 && !active.is_empty() {
    let next_pos = (current_pos + self.jump_size).min(len - 1);

    let num_workers_at_next = self.count_workers_at(next_pos, ...);

    if num_workers_at_next == active.len() {
        // BUG: Assumes all workers match at ALL intermediate positions
        current_pos = next_pos;
    } else {
        self.linear_scan_drain(..., current_pos + 1, next_pos + 1, ...);
        current_pos = next_pos;
    }
}
```

The jump optimization assumes: "if N workers are active at position `current_pos` and N workers also match at position `next_pos`, then all N workers match at every position in between." This invariant holds for append-only sequences but **breaks after remove operations**, which can delete blocks from the middle of a sequence without cascade-removing subsequent positions.

When `jump_size=32` and the query has 3 elements, `next_pos = min(0+32, 2) = 2`. The jump goes directly from position 0 to position 2, never checking position 1 where the block was removed. Since the block at position 2 still exists in the flat index (unlike RadixTree, which would disconnect children when a parent is removed), the jump reports "all workers still active" and assigns score 3.

## Two Contributing Factors

1. **No cascade removal**: Unlike RadixTree (where removing a node disconnects its subtree), PositionalIndexer's flat `(position, local_hash)` index allows children to survive parent removal. This creates "orphan" blocks at later positions.

2. **Jump assumes monotonic coverage**: The jump optimization's correctness relies on the invariant that if a worker is present at positions A and B, it must be present at all positions [A+1, B-1]. Remove operations violate this invariant.

## Suggested Fix

Option A (simplest): When removing a block at position P for a worker, also remove all blocks at positions > P for that worker (cascade removal, matching RadixTree semantics).

Option B: In `jump_search_matches`, when `num_workers_at_next == active.len()`, don't skip — always linear scan after any remove has occurred. Could track a `has_removes` flag per worker.

Option C: After a remove, invalidate and re-insert blocks at positions > P with updated seq_hashes, maintaining the position continuity invariant.

Option A is recommended as it matches the RadixTree contract and is simplest to implement.

## Impact

Any workload that removes individual blocks (as opposed to clearing entire workers) will produce incorrect `find_matches` scores from `PositionalIndexer`. The scores will be inflated, potentially routing requests to workers with less KV cache overlap than reported.

Found by: triple differential fuzzer (`fuzz_triple_differential`) comparing RadixTree vs ConcurrentRadixTree vs PositionalIndexer.
