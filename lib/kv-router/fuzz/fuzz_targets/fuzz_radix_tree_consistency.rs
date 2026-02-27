#![no_main]
use libfuzzer_sys::fuzz_target;
use dynamo_kv_router::RadixTree;
use dynamo_kv_router::protocols::{LocalBlockHash, WorkerWithDpRank};
use dynamo_kv_router_fuzz::make_store_event;

fuzz_target!(|data: &[u8]| {
    if data.len() < 3 { return; }

    let mut tree = RadixTree::new();
    let w0 = WorkerWithDpRank { worker_id: 0, dp_rank: 0 };

    // Build unique hash sequence (avoid known RefCell aliasing bug)
    let mut hashes = Vec::new();
    let mut seen = [false; 32];
    for &b in &data[1..] {
        let h = (b % 32) as u64;
        if !seen[h as usize] {
            seen[h as usize] = true;
            hashes.push(h);
        }
    }
    if hashes.is_empty() { return; }

    // Store for worker 0
    let event = make_store_event(0, 0, &hashes, None);
    if tree.apply_event(event).is_err() { return; }

    // Exact query — worker 0 should appear with score == len
    let query: Vec<LocalBlockHash> = hashes.iter().map(|&h| LocalBlockHash(h)).collect();
    let scores = tree.find_matches(query.clone(), false);
    let score = scores.scores.get(&w0).copied().unwrap_or(0);
    assert!(score > 0,
        "Worker 0 absent after storing {} blocks: {:?}", hashes.len(), hashes);
    assert_eq!(score as usize, hashes.len(),
        "Exact score {score} != stored len {}. Hashes: {:?}", hashes.len(), hashes);

    // Prefix query — score should be <= prefix length
    let plen = (data[0] as usize % hashes.len()) + 1;
    if plen < hashes.len() {
        let pq: Vec<LocalBlockHash> = hashes[..plen].iter().map(|&h| LocalBlockHash(h)).collect();
        let ps = tree.find_matches(pq, false);
        if let Some(&s) = ps.scores.get(&w0) {
            assert!(s as usize <= plen,
                "Prefix score {s} > prefix len {plen}. Hashes: {:?}", &hashes[..plen]);
        }
    }

    // Remove worker 0 and verify it's gone
    tree.remove_worker(0);
    let after = tree.find_matches(query, false);
    assert!(!after.scores.contains_key(&w0),
        "Worker 0 still present after removal. Hashes: {:?}", hashes);
});
