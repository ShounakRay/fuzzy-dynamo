#![no_main]
use libfuzzer_sys::fuzz_target;

use dynamo_kv_router::RadixTree;
use dynamo_kv_router_fuzz::{FuzzEventState, FuzzOp};

fuzz_target!(|data: &[u8]| {
    if data.len() < 2 { return; }

    let mut tree = RadixTree::new();
    let mut state = FuzzEventState::new(0);

    while let Some((_op_type, _worker_id, op)) = state.next_event(data) {
        match op {
            FuzzOp::Event(event) => { let _ = tree.apply_event(event); }
            FuzzOp::Query(seq, early_exit) => {
                let scores = tree.find_matches(seq.clone(), early_exit);
                for (_worker, &score) in &scores.scores {
                    assert!(score as usize <= seq.len(), "score {score} > seq len {}", seq.len());
                }
            }
            FuzzOp::Skip => {}
        }
    }

    for worker_id in 0..4u64 { tree.remove_worker(worker_id); }
    assert!(tree.get_workers().is_empty(), "workers remain after remove_worker");
});
