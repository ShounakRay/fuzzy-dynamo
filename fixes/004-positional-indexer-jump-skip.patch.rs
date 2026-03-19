// Fix for Bug 4: PositionalIndexer jump optimization skips removed blocks
// File: lib/kv-router/src/indexer/positional.rs
// Severity: HIGH
//
// Problem: remove_blocks_impl deletes individual blocks without cascade-removing
// blocks at later positions. The jump optimization in jump_search_matches assumes
// continuous coverage: if a worker matches at positions A and A+jump_size, it must
// match at all positions in between. Remove operations break this invariant, causing
// inflated match scores.
//
// Fix: In remove_blocks_impl, after removing a block at position P for a worker,
// cascade-remove all blocks at positions > P for that worker. This matches
// RadixTree semantics where removing a node disconnects its subtree.

// === ORIGINAL (remove_blocks_impl, lines ~280-320) ===
// fn remove_blocks_impl(
//     &self,
//     worker_blocks: &mut FxHashMap<WorkerWithDpRank, LevelIndex>,
//     worker: WorkerWithDpRank,
//     seq_hashes: &Vec<ExternalSequenceBlockHash>,
//     event_id: u64,
// ) -> Result<(), KvCacheEventError> {
//     let worker_map = worker_blocks.get_mut(&worker).ok_or_else(|| { ... })?;
//
//     let mut num_removed_blocks = 0;
//
//     for seq_hash in seq_hashes {
//         let Some((position, local_hash)) = worker_map.remove(seq_hash) else {
//             ...
//             return Err(KvCacheEventError::BlockNotFound);
//         };
//
//         if let Some(mut entry) = self.index.get_mut(&(position, local_hash)) {
//             let _ = entry.remove(*seq_hash, worker);
//         }
//
//         num_removed_blocks += 1;
//     }
//
//     if let Some(size) = self.tree_sizes.get(&worker) {
//         size.fetch_sub(num_removed_blocks, Ordering::Relaxed);
//     }
//
//     Ok(())
// }

// === FIXED ===
fn remove_blocks_impl(
    &self,
    worker_blocks: &mut FxHashMap<WorkerWithDpRank, LevelIndex>,
    worker: WorkerWithDpRank,
    seq_hashes: &Vec<ExternalSequenceBlockHash>,
    event_id: u64,
) -> Result<(), KvCacheEventError> {
    let worker_map = worker_blocks.get_mut(&worker).ok_or_else(|| {
        tracing::warn!(
            worker_id = worker.worker_id.to_string(),
            dp_rank = worker.dp_rank,
            event_id,
            block_hashes = ?seq_hashes,
            "Failed to find worker blocks to remove"
        );
        KvCacheEventError::BlockNotFound
    })?;

    let mut num_removed_blocks = 0;

    for seq_hash in seq_hashes {
        let Some((position, local_hash)) = worker_map.remove(seq_hash) else {
            tracing::warn!(
                worker_id = worker.worker_id.to_string(),
                dp_rank = worker.dp_rank,
                event_id,
                block_hash = ?seq_hash,
                "Failed to find block to remove; skipping remove operation"
            );

            if let Some(size) = self.tree_sizes.get(&worker) {
                size.fetch_sub(num_removed_blocks, Ordering::Relaxed);
            }

            return Err(KvCacheEventError::BlockNotFound);
        };

        if let Some(mut entry) = self.index.get_mut(&(position, local_hash)) {
            let _ = entry.remove(*seq_hash, worker);
        }

        num_removed_blocks += 1;

        // CASCADE REMOVAL: Remove all blocks at positions > `position` for this worker.
        // This preserves the invariant that the jump optimization relies on:
        // if a worker matches at positions A and B, it matches at all positions in [A, B].
        // Without cascade removal, orphan blocks at later positions cause inflated scores.
        let orphans: Vec<(ExternalSequenceBlockHash, usize, LocalBlockHash)> = worker_map
            .iter()
            .filter(|(_, (pos, _))| *pos > position)
            .map(|(sh, (pos, lh))| (*sh, *pos, *lh))
            .collect();

        for (orphan_seq_hash, orphan_pos, orphan_local_hash) in &orphans {
            worker_map.remove(orphan_seq_hash);
            if let Some(mut entry) = self.index.get_mut(&(*orphan_pos, *orphan_local_hash)) {
                let _ = entry.remove(*orphan_seq_hash, worker);
            }
            num_removed_blocks += 1;
        }
    }

    if let Some(size) = self.tree_sizes.get(&worker) {
        size.fetch_sub(num_removed_blocks, Ordering::Relaxed);
    }

    Ok(())
}

// === TEST ===
#[test]
fn test_remove_cascades_to_later_positions() {
    // Regression test for Bug 4: removing a block at position 1 must also
    // remove the block at position 2, so find_matches doesn't over-count.
    use crate::indexer::SyncIndexer;
    use crate::protocols::*;

    let indexer = PositionalIndexer::new(32);
    let mut worker_blocks = FxHashMap::default();

    let worker = WorkerWithDpRank::new(WorkerId::from("w1".to_string()), 0);

    // Store blocks [hash_6, hash_2, hash_5] at positions 0, 1, 2
    let local_hashes = vec![
        LocalBlockHash(6),
        LocalBlockHash(2),
        LocalBlockHash(5),
    ];
    // Compute seq_hashes: seq[0] = 6, seq[1] = hash(6, 2), seq[2] = hash(hash(6,2), 5)
    let seq_hash_0 = ExternalSequenceBlockHash::from(6u64);
    let seq_hash_1 = ExternalSequenceBlockHash::from(
        PositionalIndexer::compute_next_seq_hash(seq_hash_0.0, 2),
    );
    let seq_hash_2 = ExternalSequenceBlockHash::from(
        PositionalIndexer::compute_next_seq_hash(seq_hash_1.0, 5),
    );

    let store_event = RouterEvent {
        worker_id: worker.worker_id,
        storage_tier: StorageTier::Device,
        event: KvCacheEvent {
            event_id: 0,
            data: KvCacheEventData::Stored(KvCacheStoreData {
                parent_hash: None,
                blocks: vec![
                    KvCacheStoredBlockData { block_hash: seq_hash_0, tokens_hash: local_hashes[0], mm_extra_info: None },
                    KvCacheStoredBlockData { block_hash: seq_hash_1, tokens_hash: local_hashes[1], mm_extra_info: None },
                    KvCacheStoredBlockData { block_hash: seq_hash_2, tokens_hash: local_hashes[2], mm_extra_info: None },
                ],
            }),
            dp_rank: 0,
        },
    };
    indexer.apply_event(&mut worker_blocks, store_event).unwrap();

    // Remove block at position 1 (seq_hash_1)
    let remove_event = RouterEvent {
        worker_id: worker.worker_id,
        storage_tier: StorageTier::Device,
        event: KvCacheEvent {
            event_id: 1,
            data: KvCacheEventData::Removed(KvCacheRemoveData {
                block_hashes: vec![seq_hash_1],
            }),
            dp_rank: 0,
        },
    };
    indexer.apply_event(&mut worker_blocks, remove_event).unwrap();

    // Query [6, 2, 5] — should get score 1 (only position 0 matches),
    // NOT score 3 (which would happen without cascade removal).
    let scores = indexer.find_matches(&local_hashes, false);
    let worker_score = scores.scores.get(&worker).copied().unwrap_or(0);
    assert_eq!(worker_score, 1, "Score should be 1 (only position 0), got {}", worker_score);
}
