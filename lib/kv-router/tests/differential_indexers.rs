use std::collections::{HashMap, HashSet};

use proptest::prelude::*;
use rustc_hash::FxHashMap;

use dynamo_kv_router::ConcurrentRadixTree;
use dynamo_kv_router::RadixTree;
use dynamo_kv_router::protocols::*;

fn make_store_event(
    worker_id: u64,
    event_id: u64,
    hashes: &[u64],
    parent_seq_hash: Option<u64>,
) -> (RouterEvent, Vec<u64>) {
    let mut seq_hashes = Vec::with_capacity(hashes.len());
    for (i, &h) in hashes.iter().enumerate() {
        let sh = if i == 0 {
            match parent_seq_hash {
                None => h,
                Some(parent) => {
                    let mut bytes = [0u8; 16];
                    bytes[..8].copy_from_slice(&parent.to_le_bytes());
                    bytes[8..].copy_from_slice(&h.to_le_bytes());
                    compute_hash(&bytes)
                }
            }
        } else {
            let mut bytes = [0u8; 16];
            bytes[..8].copy_from_slice(&seq_hashes[i - 1].to_le_bytes());
            bytes[8..].copy_from_slice(&h.to_le_bytes());
            compute_hash(&bytes)
        };
        seq_hashes.push(sh);
    }

    let event = RouterEvent {
        worker_id,
        event: KvCacheEvent {
            event_id,
            data: KvCacheEventData::Stored(KvCacheStoreData {
                parent_hash: parent_seq_hash.map(ExternalSequenceBlockHash),
                blocks: hashes
                    .iter()
                    .zip(seq_hashes.iter())
                    .map(|(&h, &sh)| KvCacheStoredBlockData {
                        tokens_hash: LocalBlockHash(h),
                        block_hash: ExternalSequenceBlockHash(sh),
                        mm_extra_info: None,
                    })
                    .collect(),
            }),
            dp_rank: 0,
        },
    };
    (event, seq_hashes)
}

fn make_remove_event(worker_id: u64, event_id: u64, seq_hashes: &[u64]) -> RouterEvent {
    RouterEvent {
        worker_id,
        event: KvCacheEvent {
            event_id,
            data: KvCacheEventData::Removed(KvCacheRemoveData {
                block_hashes: seq_hashes
                    .iter()
                    .map(|&sh| ExternalSequenceBlockHash(sh))
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

#[derive(Debug, Clone)]
enum DiffOp {
    Store { worker_id: u8, hashes: Vec<u8> },
    Remove { worker_id: u8, index: u8 },
    Clear { worker_id: u8 },
    Query { seq: Vec<u8>, early_exit: bool },
}

fn arb_op() -> impl Strategy<Value = DiffOp> {
    prop_oneof![
        (any::<u8>(), prop::collection::vec(0u8..16, 1..=3))
            .prop_map(|(w, h)| DiffOp::Store { worker_id: w, hashes: h }),
        (any::<u8>(), any::<u8>()).prop_map(|(w, i)| DiffOp::Remove { worker_id: w, index: i }),
        any::<u8>().prop_map(|w| DiffOp::Clear { worker_id: w }),
        (prop::collection::vec(0u8..16, 1..=4), any::<bool>())
            .prop_map(|(s, e)| DiffOp::Query { seq: s, early_exit: e }),
    ]
}

fn run_differential(ops: Vec<DiffOp>) {
    let mut radix = RadixTree::new();
    let concurrent = ConcurrentRadixTree::new();
    let mut concurrent_lookup = FxHashMap::default();
    let mut stored: HashMap<u64, Vec<(u64, u64)>> = HashMap::new(); // (local_hash, seq_hash)
    let mut all_hashes: HashMap<u64, HashSet<u64>> = HashMap::new();
    let mut event_id: u64 = 0;

    for op in &ops {
        match op {
            DiffOp::Store { worker_id, hashes } => {
                let wid = (*worker_id % 2) as u64;
                let worker_all = all_hashes.entry(wid).or_default();
                let mut seen = HashSet::new();
                let deduped: Vec<u64> = hashes
                    .iter()
                    .map(|&h| (h % 8) as u64)
                    .filter(|h| !worker_all.contains(h) && seen.insert(*h))
                    .collect();
                if deduped.is_empty() {
                    continue;
                }
                let parent_seq_hash = stored.get(&wid).and_then(|v| v.last().map(|&(_, sh)| sh));
                let (event, seq_hashes) = make_store_event(wid, event_id, &deduped, parent_seq_hash);
                let _ = radix.apply_event(event.clone());
                let _ = concurrent.apply_event(&mut concurrent_lookup, event);
                for &h in &deduped {
                    worker_all.insert(h);
                }
                let pairs: Vec<(u64, u64)> = deduped.iter().copied().zip(seq_hashes).collect();
                stored.entry(wid).or_default().extend(pairs);
                event_id += 1;
            }
            DiffOp::Remove { worker_id, index } => {
                let wid = (*worker_id % 2) as u64;
                if let Some(worker_hashes) = stored.get_mut(&wid) {
                    if !worker_hashes.is_empty() {
                        let idx = *index as usize % worker_hashes.len();
                        let (_, seq_hash) = worker_hashes.remove(idx);
                        let event = make_remove_event(wid, event_id, &[seq_hash]);
                        let _ = radix.apply_event(event.clone());
                        let _ = concurrent.apply_event(&mut concurrent_lookup, event);
                        event_id += 1;
                    }
                }
            }
            DiffOp::Clear { worker_id } => {
                let wid = (*worker_id % 2) as u64;
                let event = make_clear_event(wid, event_id);
                let _ = radix.apply_event(event.clone());
                let _ = concurrent.apply_event(&mut concurrent_lookup, event);
                stored.remove(&wid);
                all_hashes.remove(&wid);
                event_id += 1;
            }
            DiffOp::Query { seq, early_exit } => {
                let local_seq: Vec<LocalBlockHash> =
                    seq.iter().map(|&b| LocalBlockHash((b % 8) as u64)).collect();

                let r1 = radix.find_matches(local_seq.clone(), *early_exit);
                let r2 = concurrent.find_matches_impl(&local_seq, *early_exit);

                assert_eq!(
                    r1.scores, r2.scores,
                    "Score mismatch after {event_id} events.\n\
                     RadixTree:           {:?}\n\
                     ConcurrentRadixTree: {:?}\n\
                     Query: {:?}, early_exit: {}",
                    r1.scores, r2.scores, local_seq, early_exit
                );
            }
        }
    }

    // Cleanup
    for wid in 0..2u64 {
        radix.remove_worker(wid);
        concurrent.remove_or_clear_worker_blocks(&mut concurrent_lookup, wid, false);
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10000))]

    #[test]
    fn differential_radix_vs_concurrent(
        ops in prop::collection::vec(arb_op(), 1..32)
    ) {
        run_differential(ops);
    }
}

#[test]
fn differential_single_store_query() {
    run_differential(vec![
        DiffOp::Store {
            worker_id: 0,
            hashes: vec![1, 2, 3],
        },
        DiffOp::Query {
            seq: vec![1, 2, 3],
            early_exit: false,
        },
    ]);
}

#[test]
fn differential_two_workers_overlap() {
    run_differential(vec![
        DiffOp::Store {
            worker_id: 0,
            hashes: vec![1, 2],
        },
        DiffOp::Store {
            worker_id: 1,
            hashes: vec![1, 2, 3],
        },
        DiffOp::Query {
            seq: vec![1, 2, 3],
            early_exit: false,
        },
        DiffOp::Query {
            seq: vec![1, 2, 3],
            early_exit: true,
        },
    ]);
}

#[test]
fn differential_store_remove_clear() {
    run_differential(vec![
        DiffOp::Store {
            worker_id: 0,
            hashes: vec![5, 6, 7],
        },
        DiffOp::Remove {
            worker_id: 0,
            index: 0,
        },
        DiffOp::Query {
            seq: vec![5, 6, 7],
            early_exit: false,
        },
        DiffOp::Clear { worker_id: 0 },
        DiffOp::Query {
            seq: vec![5, 6, 7],
            early_exit: false,
        },
    ]);
}
