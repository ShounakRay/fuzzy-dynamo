#![no_main]
use libfuzzer_sys::fuzz_target;
use std::collections::HashMap;

use dynamo_kv_router::RadixTree;
use dynamo_kv_router::protocols::LocalBlockHash;
use dynamo_kv_router_fuzz::{FuzzInput, FuzzOp, make_store_event, make_remove_event, make_clear_event};

fuzz_target!(|input: FuzzInput| {
    let ops = if input.ops.len() > 256 { &input.ops[..256] } else { &input.ops };

    let mut tree = RadixTree::new();
    let mut stored: HashMap<u64, Vec<(u64, u64)>> = HashMap::new(); // (local_hash, seq_hash)
    let mut event_id: u64 = 0;

    for op in ops {
        match op {
            FuzzOp::Store { worker_id, hashes } => {
                let wid = (*worker_id % 4) as u64;
                let hashes: Vec<u64> = hashes.iter().take(16).map(|&h| (h % 16) as u64).collect();
                if hashes.is_empty() { continue; }
                let parent_seq_hash = stored.get(&wid).and_then(|v| v.last().map(|&(_, sh)| sh));
                let (event, seq_hashes) = make_store_event(wid, event_id, &hashes, parent_seq_hash);
                let _ = tree.apply_event(event);
                let pairs: Vec<(u64, u64)> = hashes.iter().copied().zip(seq_hashes).collect();
                stored.entry(wid).or_default().extend(pairs);
                event_id += 1;
            }
            FuzzOp::Remove { worker_id, index } => {
                let wid = (*worker_id % 4) as u64;
                if let Some(worker_hashes) = stored.get_mut(&wid) {
                    if !worker_hashes.is_empty() {
                        let idx = *index as usize % worker_hashes.len();
                        let (_, seq_hash) = worker_hashes.remove(idx);
                        let event = make_remove_event(wid, event_id, &[seq_hash]);
                        let _ = tree.apply_event(event);
                        event_id += 1;
                    }
                }
            }
            FuzzOp::Clear { worker_id } => {
                let wid = (*worker_id % 4) as u64;
                let event = make_clear_event(wid, event_id);
                let _ = tree.apply_event(event);
                stored.remove(&wid);
                event_id += 1;
            }
            FuzzOp::Query { seq, early_exit } => {
                let seq: Vec<LocalBlockHash> = seq.iter()
                    .take(16)
                    .map(|&b| LocalBlockHash((b % 16) as u64))
                    .collect();
                if seq.is_empty() { continue; }
                let scores = tree.find_matches(seq.clone(), *early_exit);
                for (_worker, &score) in &scores.scores {
                    assert!(score as usize <= seq.len(), "score {score} > seq len {}", seq.len());
                }
            }
        }
    }

    for worker_id in 0..4u64 { tree.remove_worker(worker_id); }
    assert!(tree.get_workers().is_empty(), "workers remain after remove_worker");
});
