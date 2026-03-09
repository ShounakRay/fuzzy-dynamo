# Bug 2: ConcurrentRadixTree: Deadlock on Duplicate Block Hashes

## Summary

`ConcurrentRadixTree::apply_stored` deadlocks when a store event contains
blocks with duplicate `ExternalSequenceBlockHash` values. This is the
concurrent equivalent of Bug 16 (RadixTree RefCell reentrant borrow), but
manifests as a silent hang instead of a panic.

## Root Cause

In `apply_stored`, block iteration uses hand-over-hand write locking. When
two blocks in the same event share a hash:

1. Block 0 creates a new node B1, inserts it as a child of root, and stores
   B1 in `worker_lookup[hash]`
2. Block 1 finds B1 in `worker_lookup[hash]` and inserts B1 as a child of
   itself (self-reference through the lookup table)
3. Block 2 acquires a write lock on B1 (`current.write()`), then tries to
   read-lock B1 via `existing.read()` (since B1 is its own child) —
   **deadlock**: write lock held, read lock requested on the same RwLock

With `Rc<RefCell>` (RadixTree), this causes a `BorrowError` panic.
With `Arc<RwLock>` (ConcurrentRadixTree), this silently deadlocks forever
at 0% CPU.

## Steps to Reproduce

The deadlock requires duplicate block hashes in a single store event. In
production this would come from a worker reporting identical sequence hashes
for different blocks in the same KV cache update.

```rust
use dynamo_kv_router::ConcurrentRadixTree;
use dynamo_kv_router::protocols::*;

let tree = ConcurrentRadixTree::new();
let mut lookup = rustc_hash::FxHashMap::default();

// Store event with 3 blocks all having the same hash
let event = RouterEvent {
    worker_id: 0,
    event: KvCacheEvent {
        event_id: 0,
        data: KvCacheEventData::Stored(KvCacheStoreData {
            parent_hash: None,
            blocks: vec![
                KvCacheStoredBlockData {
                    tokens_hash: LocalBlockHash(0),
                    block_hash: ExternalSequenceBlockHash(0),
                    mm_extra_info: None,
                },
                KvCacheStoredBlockData {
                    tokens_hash: LocalBlockHash(0),
                    block_hash: ExternalSequenceBlockHash(0),
                    mm_extra_info: None,
                },
                KvCacheStoredBlockData {
                    tokens_hash: LocalBlockHash(0),
                    block_hash: ExternalSequenceBlockHash(0),
                    mm_extra_info: None,
                },
            ],
        }),
        dp_rank: 0,
    },
};

// This deadlocks forever
let _ = tree.apply_event(&mut lookup, event);
```

## Impact

- **Severity**: High — silent hang (DoS) when a worker sends duplicate
  block hashes in a single store event. The process hangs at 0% CPU with
  no error message.
- **Affected code**: `ConcurrentRadixTree::apply_stored` in
  `lib/kv-router/src/indexer/concurrent_radix_tree.rs`
- **Related**: Bug 16 (RadixTree RefCell panic on same condition)

## Suggested Fix

Add a self-reference check before acquiring the read lock on existing blocks
(same pattern as the suggested fix for Bug 16):

```rust
Some(existing) => {
    // Check for self-reference BEFORE acquiring read lock
    if Arc::ptr_eq(existing, &current) {
        tracing::warn!("self-referential block detected, skipping");
        existing.clone()
    } else {
        let existing_guard = existing.read();
        if existing_guard.block_hash != Some(block_data.block_hash) {
            tracing::warn!("block_hash mismatch");
        }
        existing.clone()
    }
}
```

Alternatively, deduplicate block hashes at the event ingestion boundary
before they reach the tree.
