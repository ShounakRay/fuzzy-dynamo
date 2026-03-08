#![no_main]
use std::collections::{HashMap, HashSet};

use libfuzzer_sys::fuzz_target;
use rustc_hash::FxHashMap;

use dynamo_kv_router::ConcurrentRadixTree;
use dynamo_kv_router::RadixTree;
use dynamo_kv_router::protocols::LocalBlockHash;
use dynamo_kv_router_fuzz::{make_store_event, make_remove_event, make_clear_event};

#[derive(Debug, Clone)]
enum DiffOp {
    Store { worker_id: u8, h0: u8, h1: u8, h2: u8 },
    Remove { worker_id: u8, index: u8 },
    Clear { worker_id: u8 },
    Query { s0: u8, s1: u8, s2: u8, early_exit: bool },
}

fuzz_target!(|data: &[u8]| {
    if data.len() < 4 { return; }

    // Decode ops from raw bytes (5 bytes per op, max 16 ops)
    let mut ops = Vec::with_capacity(16);
    let mut i = 0;
    while i + 4 <= data.len() && ops.len() < 16 {
        let op = match data[i] % 4 {
            0 => {
                let op = DiffOp::Store {
                    worker_id: data[i+1],
                    h0: data[i+2],
                    h1: data[i+3],
                    h2: data.get(i+4).copied().unwrap_or(0),
                };
                i += 5;
                op
            }
            1 => {
                let op = DiffOp::Remove { worker_id: data[i+1], index: data[i+2] };
                i += 3;
                op
            }
            2 => {
                let op = DiffOp::Clear { worker_id: data[i+1] };
                i += 2;
                op
            }
            _ => {
                let op = DiffOp::Query {
                    s0: data[i+1],
                    s1: data[i+2],
                    s2: data[i+3],
                    early_exit: data.get(i+4).map_or(false, |b| b % 2 == 1),
                };
                i += 5;
                op
            }
        };
        ops.push(op);
    }
    if ops.is_empty() { return; }

    let mut radix = RadixTree::new();
    let concurrent = ConcurrentRadixTree::new();
    let mut concurrent_lookup = FxHashMap::default();
    let mut stored: HashMap<u64, Vec<u64>> = HashMap::new();
    let mut all_hashes: HashMap<u64, HashSet<u64>> = HashMap::new();
    let mut event_id: u64 = 0;

    for op in &ops {
        match op {
            DiffOp::Store { worker_id, h0, h1, h2 } => {
                let wid = (*worker_id % 2) as u64;
                let worker_all = all_hashes.entry(wid).or_default();
                let mut seen = HashSet::new();
                let hashes: Vec<u64> = [h0, h1, h2].iter()
                    .map(|&h| (*h % 8) as u64)
                    .filter(|h| !worker_all.contains(h) && seen.insert(*h))
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
            DiffOp::Remove { worker_id, index } => {
                let wid = (*worker_id % 2) as u64;
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
            DiffOp::Clear { worker_id } => {
                let wid = (*worker_id % 2) as u64;
                let event = make_clear_event(wid, event_id);
                let _ = radix.apply_event(event.clone());
                let _ = concurrent.apply_event(&mut concurrent_lookup, event);
                stored.remove(&wid);
                all_hashes.remove(&wid);
                event_id += 1;
            }
            DiffOp::Query { s0, s1, s2, early_exit } => {
                let seq: Vec<LocalBlockHash> = [s0, s1, s2].iter()
                    .map(|&b| LocalBlockHash((*b % 8) as u64))
                    .collect();

                let r1 = radix.find_matches(seq.clone(), *early_exit);
                let r2 = concurrent.find_matches_impl(&seq, *early_exit);

                assert_eq!(r1.scores, r2.scores,
                    "Score mismatch after {event_id} events.\n\
                     RadixTree:           {:?}\n\
                     ConcurrentRadixTree: {:?}\n\
                     Query: {:?}, early_exit: {}",
                    r1.scores, r2.scores, seq, early_exit);
            }
        }
    }

    for wid in 0..2u64 {
        radix.remove_worker(wid);
        concurrent.remove_or_clear_worker_blocks(&mut concurrent_lookup, wid, false);
    }
});
