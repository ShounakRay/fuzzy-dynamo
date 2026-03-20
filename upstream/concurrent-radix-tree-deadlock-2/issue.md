# [BUG]: ConcurrentRadixTree deadlocks on duplicate block hashes in store event

### Describe the Bug

`ConcurrentRadixTree::apply_stored` deadlocks when a store event contains blocks with duplicate `ExternalSequenceBlockHash` values. In `apply_stored`, block iteration uses hand-over-hand write locking. When two blocks in the same event share a hash:

1. Block 0 creates a new node B1, inserts it as a child of root, and stores B1 in `worker_lookup[hash]`
2. Block 1 finds B1 in `worker_lookup[hash]` and inserts B1 as a child of itself (self-reference through the lookup table)
3. Block 2 acquires a write lock on B1 (`current.write()`), then tries to read-lock B1 via `existing.read()` (since B1 is its own child) — **deadlock**: write lock held, read lock requested on the same RwLock

With `Rc<RefCell>` (RadixTree), this same condition causes a `BorrowError` panic. With `Arc<RwLock>` (ConcurrentRadixTree), it silently deadlocks forever at 0% CPU.

### Steps to Reproduce

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

### Expected Behavior

Should detect the self-reference condition and handle it gracefully (e.g., skip the duplicate or return an error), not deadlock.

### Actual Behavior

The process hangs indefinitely at 0% CPU with no error message. The deadlock occurs when `current.write()` is held and `existing.read()` is requested on the same `Arc<RwLock>` node.

### Environment

On commit `57e6a79f5` in `dynamo-runtime` @ `lib/kv-router/src/indexer/concurrent_radix_tree.rs`

### Additional Context

This is a high-severity silent hang (DoS) triggered when a worker sends duplicate block hashes in a single store event. This is the concurrent equivalent of the RadixTree RefCell reentrant borrow panic, but manifests as a silent hang instead of a panic.

A possible fix is to add a self-reference check before acquiring the read lock on existing blocks (e.g., `Arc::ptr_eq(existing, &current)`) or to deduplicate block hashes at the event ingestion boundary before they reach the tree.

Found via fuzzing with cargo-fuzz / libfuzzer.

### Screenshots

_No response_
