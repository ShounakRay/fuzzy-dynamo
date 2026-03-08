#![no_main]
use std::collections::{HashMap, HashSet};

use libfuzzer_sys::fuzz_target;
use rustc_hash::FxHashMap;

use dynamo_kv_router::ConcurrentRadixTree;
use dynamo_kv_router::RadixTree;
use dynamo_kv_router::protocols::LocalBlockHash;
use dynamo_kv_router_fuzz::{FuzzInput, FuzzOp, make_store_event, make_remove_event, make_clear_event};

fuzz_target!(|input: FuzzInput| {
    let ops = if input.ops.len() > 128 { &input.ops[..128] } else { &input.ops };

    let mut radix = RadixTree::new();
    let concurrent = ConcurrentRadixTree::new();
    let mut concurrent_lookup = FxHashMap::default();
    let mut stored: HashMap<u64, Vec<u64>> = HashMap::new();
    // Track all hashes ever stored per worker to avoid cross-op collisions
    // that trigger the known RadixTree RefCell aliasing bug.
    let mut all_hashes: HashMap<u64, HashSet<u64>> = HashMap::new();
    let mut event_id: u64 = 0;

    for op in ops {
        match op {
            FuzzOp::Store { worker_id, hashes } => {
                let wid = (*worker_id % 4) as u64;
                let worker_all = all_hashes.entry(wid).or_default();
                // Deduplicate: skip hashes already stored for this worker
                let hashes: Vec<u64> = hashes.iter()
                    .take(16)
                    .map(|&h| (h % 16) as u64)
                    .filter(|h| !worker_all.contains(h))
                    .collect();
                if hashes.is_empty() { continue; }
                let parent = stored.get(&wid).and_then(|v| v.last().copied());
                let event = make_store_event(wid, event_id, &hashes, parent);
                let _ = radix.apply_event(event.clone());
                let _ = concurrent.apply_event(&mut concurrent_lookup, event);
                for &h in &hashes { worker_all.insert(h); }
                stored.entry(wid).or_default().extend_from_slice(&hashes);
                event_id += 1;
            }
            FuzzOp::Remove { worker_id, index } => {
                let wid = (*worker_id % 4) as u64;
                if let Some(worker_hashes) = stored.get_mut(&wid) {
                    if !worker_hashes.is_empty() {
                        let idx = *index as usize % worker_hashes.len();
                        let hash = worker_hashes.remove(idx);
                        let event = make_remove_event(wid, event_id, &[hash]);
                        let _ = radix.apply_event(event.clone());
                        let _ = concurrent.apply_event(&mut concurrent_lookup, event);
                        event_id += 1;
                    }
                }
            }
            FuzzOp::Clear { worker_id } => {
                let wid = (*worker_id % 4) as u64;
                let event = make_clear_event(wid, event_id);
                let _ = radix.apply_event(event.clone());
                let _ = concurrent.apply_event(&mut concurrent_lookup, event);
                stored.remove(&wid);
                all_hashes.remove(&wid);
                event_id += 1;
            }
            FuzzOp::Query { seq, early_exit } => {
                let seq: Vec<LocalBlockHash> = seq.iter()
                    .take(16)
                    .map(|&b| LocalBlockHash((b % 16) as u64))
                    .collect();
                if seq.is_empty() { continue; }

                let r1 = radix.find_matches(seq.clone(), *early_exit);
                let r2 = concurrent.find_matches_impl(&seq, *early_exit);

                // Compare scores — both implementations should agree
                assert_eq!(r1.scores, r2.scores,
                    "Score mismatch after {event_id} events.\n\
                     RadixTree:           {:?}\n\
                     ConcurrentRadixTree: {:?}\n\
                     Query: {:?}, early_exit: {}",
                    r1.scores, r2.scores, seq, early_exit);
            }
        }
    }

    // After all ops, remove all workers and verify both are empty
    for wid in 0..4u64 {
        radix.remove_worker(wid);
        concurrent.remove_or_clear_worker_blocks(&mut concurrent_lookup, wid, false);
    }
    assert!(radix.get_workers().is_empty(), "RadixTree workers remain after removal");
    assert!(concurrent.get_workers().is_empty(), "ConcurrentRadixTree workers remain after removal");
});
