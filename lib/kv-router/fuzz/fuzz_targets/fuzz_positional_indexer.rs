#![no_main]
use libfuzzer_sys::fuzz_target;

use dynamo_kv_router::PositionalIndexer;
use dynamo_kv_router::SyncIndexer;
use dynamo_kv_router_fuzz::{FuzzEventState, FuzzOp};

fuzz_target!(|data: &[u8]| {
    if data.len() < 3 { return; }

    let jump_size = (data[0] % 32) as usize + 1;
    let indexer = PositionalIndexer::new(jump_size);
    let mut state = FuzzEventState::new(1);

    while let Some((_op_type, _worker_id, op)) = state.next_event(data) {
        match op {
            FuzzOp::Event(event) => { let _ = indexer.apply_event(event); }
            FuzzOp::Query(seq, early_exit) => {
                let scores = indexer.find_matches(&seq, early_exit);
                for (_worker, &score) in &scores.scores {
                    assert!(score as usize <= seq.len(), "score {score} > seq len {}", seq.len());
                }
            }
            FuzzOp::Skip => {}
        }
    }

    for worker_id in 0..4u64 { indexer.remove_worker(worker_id); }
});
