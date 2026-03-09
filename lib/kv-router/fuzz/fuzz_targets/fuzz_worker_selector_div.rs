#![no_main]
use std::collections::HashMap;

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;

use dynamo_kv_router::protocols::{OverlapScores, WorkerWithDpRank};
use dynamo_kv_router::scheduling::config::RouterConfigOverride;
use dynamo_kv_router::selector::{DefaultWorkerSelector, WorkerSelector};
use dynamo_kv_router::{KvSchedulerError, SchedulingRequest};

/// Minimal WorkerConfig for fuzzing
struct FuzzWorkerConfig {
    dp_start: u32,
    dp_size: u32,
}

impl dynamo_kv_router::protocols::WorkerConfigLike for FuzzWorkerConfig {
    fn data_parallel_start_rank(&self) -> u32 { self.dp_start }
    fn data_parallel_size(&self) -> u32 { self.dp_size }
    fn max_num_batched_tokens(&self) -> Option<u64> { None }
    fn total_kv_blocks(&self) -> Option<u64> { None }
}

#[derive(Debug, Arbitrary)]
struct FuzzInput {
    block_size: u32,
    num_workers: u8,
    isl_tokens: u16,
    temperature: u8,
    overlap_weight: u8,
    dp_sizes: Vec<u8>,
    overlap_scores: Vec<(u8, u8, u32)>, // (worker_idx, dp_rank, score)
}

fuzz_target!(|input: FuzzInput| {
    let num_workers = (input.num_workers % 8) as usize;
    if num_workers == 0 { return; }
    let isl = (input.isl_tokens as usize).max(1);
    // Allow block_size=0 to catch div-by-zero bugs, but also test non-zero paths
    let block_size = if input.block_size == 0 { input.block_size } else { input.block_size.max(1) };

    // Build workers map
    let mut workers: HashMap<u64, FuzzWorkerConfig> = HashMap::new();
    for i in 0..num_workers {
        let dp_size = input.dp_sizes.get(i).map(|&d| (d % 4).max(1) as u32).unwrap_or(1);
        workers.insert(i as u64, FuzzWorkerConfig {
            dp_start: 0,
            dp_size,
        });
    }

    // Build overlap scores
    let mut scores = OverlapScores::new();
    for &(worker_idx, dp_rank, score) in input.overlap_scores.iter().take(32) {
        let wid = (worker_idx as u64) % (num_workers as u64);
        let w = WorkerWithDpRank::new(wid, dp_rank as u32 % 4);
        scores.scores.insert(w, score);
    }

    let temperature = (input.temperature as f64) / 50.0; // 0.0 to 5.1
    let overlap_weight = (input.overlap_weight as f64) / 25.0 - 5.0; // -5.0 to 5.2

    let selector = DefaultWorkerSelector::default();

    let request = SchedulingRequest {
        maybe_request_id: None,
        token_seq: None,
        isl_tokens: isl,
        overlaps: scores,
        decode_blocks: HashMap::new(),
        prefill_tokens: HashMap::new(),
        router_config_override: Some(RouterConfigOverride {
            overlap_score_weight: Some(overlap_weight),
            router_temperature: Some(temperature),
            assume_kv_reuse: None,
        }),
        update_states: false,
        lora_name: None,
        priority_jump: 0.0,
        expected_output_tokens: None,
        allowed_worker_ids: None,
        resp_tx: None,
    };

    // This should NOT panic — but block_size=0 causes div_ceil to panic
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        selector.select_worker(&workers, &request, input.block_size)
    }));

    match result {
        Ok(Ok(selection)) => {
            // Valid selection — the worker should exist
            assert!(
                workers.contains_key(&selection.worker.worker_id),
                "Selected worker {} doesn't exist",
                selection.worker.worker_id
            );
        }
        Ok(Err(KvSchedulerError::NoEndpoints)) => {
            // Expected when no workers match
        }
        Ok(Err(_)) => {
            // Other errors are OK
        }
        Err(panic_info) => {
            // PANIC in select_worker — this is a bug!
            // Re-panic with details for crash artifact
            let msg = if let Some(s) = panic_info.downcast_ref::<String>() {
                s.clone()
            } else if let Some(s) = panic_info.downcast_ref::<&str>() {
                s.to_string()
            } else {
                "unknown panic".to_string()
            };
            panic!(
                "select_worker panicked with block_size={}: {}",
                input.block_size, msg
            );
        }
    }
});
