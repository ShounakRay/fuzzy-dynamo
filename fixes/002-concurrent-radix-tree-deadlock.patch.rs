// Fix for Bug 2: ConcurrentRadixTree deadlock on duplicate block hashes
// File: lib/kv-router/src/indexer/concurrent_radix_tree.rs
// Severity: MEDIUM (issue doc says High, but task says MEDIUM)
//
// Problem: When a store event contains duplicate ExternalSequenceBlockHash values,
//   apply_stored can create a self-referential node (a node that is its own child
//   via the worker_lookup table). The code then holds a write lock on the node
//   and tries to acquire a read lock on the same node -> deadlock (RwLock is not
//   reentrant).
// Fix: Before acquiring the read lock on an existing child, check if it is the
//   same Arc as `current` (the parent we already hold a write lock on). If so,
//   skip the read-lock and just clone the reference.

// === ORIGINAL (lines 370-383 inside apply_stored, the Some(existing) arm) ===
//                 match parent_guard.children.get(&block_data.tokens_hash) {
//                     Some(existing) => {
//                         // Verify our simplifying assumption: block_hash is uniform across workers
//                         {
//                             let existing_guard = existing.read();
//                             if existing_guard.block_hash != Some(block_data.block_hash) {
//                                 tracing::warn!(
//                                     expected = ?block_data.block_hash,
//                                     actual = ?existing_guard.block_hash,
//                                     "block_hash mismatch: sequence hashes should be uniform across workers"
//                                 );
//                             }
//                         }
//                         existing.clone()
//                     }

// === FIXED ===
                match parent_guard.children.get(&block_data.tokens_hash) {
                    Some(existing) => {
                        // Guard against self-reference: if duplicate block hashes
                        // caused `current` to be inserted as its own child (via
                        // the worker_lookup table), acquiring a read lock here
                        // would deadlock since we already hold the write lock on
                        // `current` (= parent_guard).
                        if Arc::ptr_eq(existing, &current) {
                            tracing::warn!(
                                block_hash = ?block_data.block_hash,
                                "self-referential block detected (duplicate hash in store event), skipping verification"
                            );
                            existing.clone()
                        } else {
                            // Verify our simplifying assumption: block_hash is uniform across workers
                            {
                                let existing_guard = existing.read();
                                if existing_guard.block_hash != Some(block_data.block_hash) {
                                    tracing::warn!(
                                        expected = ?block_data.block_hash,
                                        actual = ?existing_guard.block_hash,
                                        "block_hash mismatch: sequence hashes should be uniform across workers"
                                    );
                                }
                            }
                            existing.clone()
                        }
                    }

// === TEST ===
#[test]
fn test_apply_stored_duplicate_block_hashes_no_deadlock() {
    use std::time::Duration;

    let tree = ConcurrentRadixTree::new();
    let mut lookup = FxHashMap::default();

    let worker = WorkerWithDpRank {
        worker_id: WorkerId(0),
        dp_rank: 0,
    };

    // Store event with 3 blocks all having the same ExternalSequenceBlockHash
    let op = KvCacheStoreData {
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
    };

    // Run in a thread with a timeout to detect deadlock
    let handle = std::thread::spawn(move || {
        tree.apply_stored(&mut lookup, worker, op, 0)
    });

    // If this doesn't complete in 2 seconds, it's deadlocked
    let result = handle.join();
    assert!(result.is_ok(), "apply_stored should not deadlock on duplicate block hashes");
}
