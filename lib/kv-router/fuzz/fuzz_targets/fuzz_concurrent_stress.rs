#![no_main]
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use libfuzzer_sys::fuzz_target;
use rustc_hash::FxHashMap;

use dynamo_kv_router::ConcurrentRadixTree;
use dynamo_kv_router::protocols::LocalBlockHash;
use dynamo_kv_router_fuzz::{make_store_event, make_remove_event, make_clear_event};

/// Multi-threaded stress test for ConcurrentRadixTree.
///
/// Spawns threads that concurrently apply events and query the tree.
/// The tree uses DashMap + parking_lot::RwLock internally, so this
/// exercises real lock contention paths.
fuzz_target!(|data: &[u8]| {
    if data.len() < 8 { return; }

    let num_threads = (data[0] % 4) as usize + 2; // 2..=5 threads
    let data = &data[1..];

    // Parse operations per thread from the input
    let chunk_size = data.len() / num_threads;
    if chunk_size < 4 { return; }

    let tree = Arc::new(ConcurrentRadixTree::new());

    // Each thread gets its own worker ID range and lookup map.
    // Thread 0: workers 0,1. Thread 1: workers 2,3. etc.
    let mut handles = Vec::new();

    for t in 0..num_threads {
        let tree = Arc::clone(&tree);
        let thread_data = data[t * chunk_size..(t + 1) * chunk_size].to_vec();
        let worker_base = (t * 2) as u64;

        handles.push(std::thread::spawn(move || {
            let mut lookup = FxHashMap::default();
            let mut stored: HashMap<u64, Vec<(u64, u64)>> = HashMap::new();
            let mut all_hashes: HashMap<u64, HashSet<u64>> = HashMap::new();
            let mut event_id = worker_base * 1000; // non-overlapping event IDs

            let mut i = 0;
            while i + 4 <= thread_data.len() {
                match thread_data[i] % 4 {
                    0 => {
                        // Store
                        let wid = worker_base + (thread_data[i + 1] % 2) as u64;
                        let worker_all = all_hashes.entry(wid).or_default();
                        let mut seen = HashSet::new();
                        let h0 = (thread_data[i + 2] % 16) as u64;
                        let h1 = (thread_data[i + 3] % 16) as u64;
                        let hashes: Vec<u64> = [h0, h1].into_iter()
                            .filter(|h| !worker_all.contains(h) && seen.insert(*h))
                            .collect();
                        i += 4;
                        if hashes.is_empty() { continue; }
                        let parent = stored.get(&wid).and_then(|v| v.last().map(|&(_, sh)| sh));
                        let (event, seq_hashes) = make_store_event(wid, event_id, &hashes, parent);
                        let _ = tree.apply_event(&mut lookup, event);
                        for &h in &hashes { worker_all.insert(h); }
                        let pairs: Vec<(u64, u64)> = hashes.into_iter().zip(seq_hashes).collect();
                        stored.entry(wid).or_default().extend(pairs);
                        event_id += 1;
                    }
                    1 => {
                        // Remove
                        let wid = worker_base + (thread_data[i + 1] % 2) as u64;
                        i += 4;
                        if let Some(worker_hashes) = stored.get_mut(&wid) {
                            if !worker_hashes.is_empty() {
                                let idx = thread_data[i.saturating_sub(2)] as usize % worker_hashes.len();
                                let (_, seq_hash) = worker_hashes.remove(idx);
                                let event = make_remove_event(wid, event_id, &[seq_hash]);
                                let _ = tree.apply_event(&mut lookup, event);
                                event_id += 1;
                            }
                        }
                    }
                    2 => {
                        // Clear
                        let wid = worker_base + (thread_data[i + 1] % 2) as u64;
                        i += 2;
                        let event = make_clear_event(wid, event_id);
                        let _ = tree.apply_event(&mut lookup, event);
                        stored.remove(&wid);
                        all_hashes.remove(&wid);
                        event_id += 1;
                    }
                    _ => {
                        // Query (concurrent read while other threads write)
                        let seq: Vec<LocalBlockHash> = thread_data[i + 1..i + 4].iter()
                            .map(|&b| LocalBlockHash((b % 16) as u64))
                            .collect();
                        i += 4;
                        let result = tree.find_matches_impl(&seq, false);
                        // Must not panic; scores must be reasonable
                        for (_worker, &score) in &result.scores {
                            assert!(score as usize <= seq.len(),
                                "Score {} > query len {}", score, seq.len());
                        }
                    }
                }
            }
        }));
    }

    // Join all threads — any panic propagates
    for h in handles {
        h.join().expect("Thread panicked during concurrent radix tree operations");
    }
});
