#![no_main]
use libfuzzer_sys::fuzz_target;

use dynamo_kv_router::RadixTree;
use dynamo_kv_router_fuzz::{FuzzEventState, FuzzOp};

fuzz_target!(|data: &[u8]| {
    if data.len() < 2 {
        return;
    }

    let mut tree = RadixTree::new();
    let mut state = FuzzEventState::new(0);

    while let Some((op_type, _worker_id, op)) = state.next_event(data) {
        match op {
            FuzzOp::Event(event) => {
                let _ = tree.apply_event(event);
            }
            FuzzOp::Query(seq, early_exit) => {
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
            FuzzOp::Skip => {
                if op_type == 2 {
                    // Clear was skipped (no stored data), but tree.apply_event
                    // was not called — this is fine
                }
            }
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
