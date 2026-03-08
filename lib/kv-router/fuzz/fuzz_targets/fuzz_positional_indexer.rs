#![no_main]
use std::collections::{HashMap, HashSet};

use libfuzzer_sys::fuzz_target;
use rustc_hash::FxHashMap;

use dynamo_kv_router::PositionalIndexer;
use dynamo_kv_router::indexer::SyncIndexer;
use dynamo_kv_router::protocols::LocalBlockHash;
use dynamo_kv_router_fuzz::{make_store_event, make_remove_event, make_clear_event};

#[derive(Debug, Clone)]
enum PosOp {
    Store { worker_id: u8, h0: u8, h1: u8, h2: u8 },
    Remove { worker_id: u8, index: u8 },
    Clear { worker_id: u8 },
    Query { len: u8, s0: u8, s1: u8, s2: u8, early_exit: bool },
}

fuzz_target!(|data: &[u8]| {
    if data.len() < 4 { return; }

    // Decode jump_size from first byte: 1..=64
    let jump_size = (data[0] % 64) as usize + 1;
    let data = &data[1..];

    // Decode ops from raw bytes
    let mut ops = Vec::with_capacity(16);
    let mut i = 0;
    while i + 4 <= data.len() && ops.len() < 16 {
        let op = match data[i] % 4 {
            0 => {
                let op = PosOp::Store {
                    worker_id: data[i+1],
                    h0: data[i+2],
                    h1: data[i+3],
                    h2: data.get(i+4).copied().unwrap_or(0),
                };
                i += 5;
                op
            }
            1 => {
                let op = PosOp::Remove { worker_id: data[i+1], index: data[i+2] };
                i += 3;
                op
            }
            2 => {
                let op = PosOp::Clear { worker_id: data[i+1] };
                i += 2;
                op
            }
            _ => {
                let op = PosOp::Query {
                    len: data[i+1],
                    s0: data[i+2],
                    s1: data[i+3],
                    s2: data.get(i+4).copied().unwrap_or(0),
                    early_exit: data.get(i+5).map_or(false, |b| b % 2 == 1),
                };
                i += 6;
                op
            }
        };
        ops.push(op);
    }
    if ops.is_empty() { return; }

    let indexer = PositionalIndexer::new(jump_size);
    let mut worker_blocks = FxHashMap::default();
    let mut stored: HashMap<u64, Vec<(u64, u64)>> = HashMap::new(); // (local_hash, seq_hash)
    let mut all_hashes: HashMap<u64, HashSet<u64>> = HashMap::new();
    let mut event_id: u64 = 0;

    for op in &ops {
        match op {
            PosOp::Store { worker_id, h0, h1, h2 } => {
                let wid = (*worker_id % 2) as u64;
                let worker_all = all_hashes.entry(wid).or_default();
                let mut seen = HashSet::new();
                let hashes: Vec<u64> = [h0, h1, h2].iter()
                    .map(|&h| (*h % 16) as u64)
                    .filter(|h| !worker_all.contains(h) && seen.insert(*h))
                    .collect();
                if hashes.is_empty() { continue; }
                let parent_seq_hash = stored.get(&wid).and_then(|v| v.last().map(|&(_, sh)| sh));
                let (event, seq_hashes) = make_store_event(wid, event_id, &hashes, parent_seq_hash);
                let _ = indexer.apply_event(&mut worker_blocks, event);
                for &h in &hashes { worker_all.insert(h); }
                let pairs: Vec<(u64, u64)> = hashes.iter().copied().zip(seq_hashes).collect();
                stored.entry(wid).or_default().extend(pairs);
                event_id += 1;
            }
            PosOp::Remove { worker_id, index } => {
                let wid = (*worker_id % 2) as u64;
                if let Some(worker_hashes) = stored.get_mut(&wid) {
                    if !worker_hashes.is_empty() {
                        let idx = *index as usize % worker_hashes.len();
                        let (_, seq_hash) = worker_hashes.remove(idx);
                        let event = make_remove_event(wid, event_id, &[seq_hash]);
                        let _ = indexer.apply_event(&mut worker_blocks, event);
                        event_id += 1;
                    }
                }
            }
            PosOp::Clear { worker_id } => {
                let wid = (*worker_id % 2) as u64;
                let event = make_clear_event(wid, event_id);
                let _ = indexer.apply_event(&mut worker_blocks, event);
                stored.remove(&wid);
                all_hashes.remove(&wid);
                event_id += 1;
            }
            PosOp::Query { len, s0, s1, s2, early_exit } => {
                // Test with various sequence lengths, including empty
                let full_seq: Vec<LocalBlockHash> = [s0, s1, s2].iter()
                    .map(|&b| LocalBlockHash((*b % 16) as u64))
                    .collect();
                let query_len = (*len as usize) % (full_seq.len() + 1);
                let seq = &full_seq[..query_len];

                // This must not panic — even with empty sequences
                let result = indexer.find_matches(seq, *early_exit);

                // Scores must be non-negative and <= sequence length
                for (_worker, &score) in &result.scores {
                    assert!(
                        score as usize <= seq.len(),
                        "Score {} exceeds sequence length {} for query {:?}",
                        score, seq.len(), seq
                    );
                }
            }
        }
    }
});
