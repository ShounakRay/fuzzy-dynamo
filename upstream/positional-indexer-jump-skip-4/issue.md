# [BUG]: PositionalIndexer jump optimization skips removed blocks, producing incorrect scores

### Describe the Bug

`PositionalIndexer::jump_search_matches` in `lib/kv-router/src/indexer/positional.rs` can report inflated match scores when blocks have been removed from the middle of a stored sequence. The jump optimization assumes that if N workers are active at position `current_pos` and N workers also match at position `next_pos`, then all N workers match at every position in between. This invariant holds for append-only sequences but breaks after remove operations, which can delete blocks from the middle of a sequence without cascade-removing subsequent positions.

When `jump_size=32` and the query has 3 elements, `next_pos = min(0+32, 2) = 2`. The jump goes directly from position 0 to position 2, never checking position 1 where the block was removed. Since the block at position 2 still exists in the flat index (unlike RadixTree, which would disconnect children when a parent is removed), the jump reports "all workers still active" and assigns score 3 instead of the correct score 1.

### Steps to Reproduce

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

### Expected Behavior

After removing the block at position 1, `find_matches` should report score 1 (only position 0 matches). The query should not report a full prefix match when intermediate blocks have been removed.

### Actual Behavior

`find_matches` reports score 3 (full match), because the jump optimization skips from position 0 directly to position 2 without checking that position 1 was removed. This inflated score causes incorrect worker selection during KV cache routing.

### Environment

On commit `57e6a79f5` in `dynamo-runtime` @ `lib/kv-router/src/indexer/positional.rs` (line ~619)

### Additional Context

Two contributing factors make this possible: (1) unlike RadixTree, PositionalIndexer's flat `(position, local_hash)` index allows children to survive parent removal, creating "orphan" blocks at later positions; and (2) the jump optimization's correctness relies on monotonic coverage, which remove operations violate.

A possible fix is to cascade removals: when removing a block at position P for a worker, also remove all blocks at positions > P for that worker, matching RadixTree semantics. Alternatively, always fall back to linear scan after any remove has occurred.

Found via fuzzing with cargo-fuzz / libfuzzer.

### Screenshots

_No response_
