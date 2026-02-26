#![no_main]
use libfuzzer_sys::fuzz_target;

use dynamo_kv_router::protocols::{
    ExternalSequenceBlockHash, KvCacheEvent, KvCacheEventData, KvCacheRemoveData, KvCacheStoreData,
    KvCacheStoredBlockData, LocalBlockHash, RouterEvent,
};
use dynamo_kv_router::RadixTree;

/// Build a store event from fuzz bytes.
fn make_store_event(worker_id: u64, event_id: u64, hashes: &[u64], parent: Option<u64>) -> RouterEvent {
    RouterEvent {
        worker_id,
        event: KvCacheEvent {
            event_id,
            data: KvCacheEventData::Stored(KvCacheStoreData {
                parent_hash: parent.map(|h| ExternalSequenceBlockHash(h * 100)),
                blocks: hashes
                    .iter()
                    .map(|&h| KvCacheStoredBlockData {
                        tokens_hash: LocalBlockHash(h),
                        block_hash: ExternalSequenceBlockHash(h * 100),
                        mm_extra_info: None,
                    })
                    .collect(),
            }),
            dp_rank: 0,
        },
    }
}

/// Build a remove event from fuzz bytes.
fn make_remove_event(worker_id: u64, event_id: u64, hashes: &[u64]) -> RouterEvent {
    RouterEvent {
        worker_id,
        event: KvCacheEvent {
            event_id,
            data: KvCacheEventData::Removed(KvCacheRemoveData {
                block_hashes: hashes
                    .iter()
                    .map(|&h| ExternalSequenceBlockHash(h * 100))
                    .collect(),
            }),
            dp_rank: 0,
        },
    }
}

/// Build a clear event.
fn make_clear_event(worker_id: u64, event_id: u64) -> RouterEvent {
    RouterEvent {
        worker_id,
        event: KvCacheEvent {
            event_id,
            data: KvCacheEventData::Cleared,
            dp_rank: 0,
        },
    }
}

fuzz_target!(|data: &[u8]| {
    if data.len() < 2 {
        return;
    }

    let mut tree = RadixTree::new();
    let mut pos = 0;
    let mut event_id = 0u64;

    // Track what we've stored per worker so we can build valid parent refs
    // and valid remove targets.
    let mut stored: std::collections::HashMap<u64, Vec<u64>> = std::collections::HashMap::new();

    while pos < data.len() {
        let op_byte = data[pos];
        pos += 1;
        if pos >= data.len() {
            break;
        }

        // Worker ID: 0-3 (4 workers)
        let worker_id = (data[pos] % 4) as u64;
        pos += 1;

        match op_byte % 4 {
            0 => {
                // Store: read 1-4 block hashes
                let count = if pos < data.len() {
                    (data[pos] % 4) as usize + 1
                } else {
                    1
                };
                pos += 1;

                let mut hashes = Vec::new();
                for _ in 0..count {
                    if pos >= data.len() {
                        break;
                    }
                    hashes.push((data[pos] % 16) as u64);
                    pos += 1;
                }

                if hashes.is_empty() {
                    continue;
                }

                // Decide parent: use last stored hash for this worker or None
                let parent = stored.get(&worker_id).and_then(|v| v.last().copied());

                let event = make_store_event(worker_id, event_id, &hashes, parent);
                let _ = tree.apply_event(event);

                // Track stored hashes
                stored.entry(worker_id).or_default().extend_from_slice(&hashes);
                event_id += 1;
            }
            1 => {
                // Remove: pick a stored hash for this worker
                if let Some(worker_hashes) = stored.get_mut(&worker_id) {
                    if !worker_hashes.is_empty() {
                        let idx = if pos < data.len() {
                            data[pos] as usize % worker_hashes.len()
                        } else {
                            0
                        };
                        pos += 1;
                        let hash = worker_hashes.remove(idx);
                        let event = make_remove_event(worker_id, event_id, &[hash]);
                        let _ = tree.apply_event(event);
                        event_id += 1;
                    }
                }
            }
            2 => {
                // Clear
                let event = make_clear_event(worker_id, event_id);
                let _ = tree.apply_event(event);
                stored.remove(&worker_id);
                event_id += 1;
            }
            3 => {
                // Query: find_matches with a short sequence
                let count = if pos < data.len() {
                    (data[pos] % 8) as usize + 1
                } else {
                    1
                };
                pos += 1;

                let mut seq = Vec::new();
                for _ in 0..count {
                    if pos >= data.len() {
                        break;
                    }
                    seq.push(LocalBlockHash((data[pos] % 16) as u64));
                    pos += 1;
                }

                if seq.is_empty() {
                    continue;
                }

                let early_exit = pos < data.len() && data[pos] % 2 == 0;
                if pos < data.len() {
                    pos += 1;
                }

                let scores = tree.find_matches(seq.clone(), early_exit);

                // Invariant: all scores must be <= sequence length
                for (_worker, &score) in &scores.scores {
                    assert!(
                        score as usize <= seq.len(),
                        "score {} > sequence length {}",
                        score,
                        seq.len()
                    );
                }
            }
            _ => unreachable!(),
        }
    }

    // Final invariant: remove_worker should not panic
    for worker_id in 0..4u64 {
        tree.remove_worker(worker_id);
    }

    // Tree should be empty after removing all workers
    let workers = tree.get_workers();
    assert!(workers.is_empty(), "workers should be empty after remove_worker");
});
