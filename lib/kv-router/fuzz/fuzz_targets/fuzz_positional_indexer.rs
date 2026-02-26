#![no_main]
use libfuzzer_sys::fuzz_target;

use dynamo_kv_router::protocols::{
    ExternalSequenceBlockHash, KvCacheEvent, KvCacheEventData, KvCacheRemoveData, KvCacheStoreData,
    KvCacheStoredBlockData, LocalBlockHash, RouterEvent,
};
use dynamo_kv_router::PositionalIndexer;
use dynamo_kv_router::SyncIndexer;

/// Build a store event for the positional indexer.
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
    if data.len() < 3 {
        return;
    }

    // Use first byte to set jump_size (1-32)
    let jump_size = (data[0] % 32) as usize + 1;
    let indexer = PositionalIndexer::new(jump_size);

    let mut pos = 1;
    let mut event_id = 0u64;
    let mut stored: std::collections::HashMap<u64, Vec<u64>> = std::collections::HashMap::new();

    while pos < data.len() {
        let op_byte = data[pos];
        pos += 1;
        if pos >= data.len() {
            break;
        }

        let worker_id = (data[pos] % 4) as u64;
        pos += 1;

        match op_byte % 4 {
            0 => {
                // Store
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

                let parent = stored.get(&worker_id).and_then(|v| v.last().copied());
                let event = make_store_event(worker_id, event_id, &hashes, parent);
                let _ = indexer.apply_event(event);
                stored.entry(worker_id).or_default().extend_from_slice(&hashes);
                event_id += 1;
            }
            1 => {
                // Remove
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
                        let _ = indexer.apply_event(event);
                        event_id += 1;
                    }
                }
            }
            2 => {
                // Clear
                let event = make_clear_event(worker_id, event_id);
                let _ = indexer.apply_event(event);
                stored.remove(&worker_id);
                event_id += 1;
            }
            3 => {
                // Query
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

                let scores = indexer.find_matches(&seq, early_exit);

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

    // remove_worker should not panic
    for worker_id in 0..4u64 {
        indexer.remove_worker(worker_id);
    }
});
