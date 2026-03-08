#![no_main]
use std::collections::{HashMap, HashSet};

use libfuzzer_sys::fuzz_target;

use dynamo_kv_router::DefaultWorkerSelector;
use dynamo_kv_router::WorkerSelector;
use dynamo_kv_router::protocols::{OverlapScores, WorkerConfigLike, WorkerWithDpRank};
use dynamo_kv_router::scheduling::config::KvRouterConfig;
use dynamo_kv_router::SchedulingRequest;

/// Minimal implementation of WorkerConfigLike for fuzzing.
struct FuzzWorkerConfig {
    dp_size: u32,
    dp_start_rank: u32,
    max_batched: Option<u64>,
    total_blocks: Option<u64>,
}

impl WorkerConfigLike for FuzzWorkerConfig {
    fn data_parallel_start_rank(&self) -> u32 { self.dp_start_rank }
    fn data_parallel_size(&self) -> u32 { self.dp_size }
    fn max_num_batched_tokens(&self) -> Option<u64> { self.max_batched }
    fn total_kv_blocks(&self) -> Option<u64> { self.total_blocks }
}

fuzz_target!(|data: &[u8]| {
    if data.len() < 12 { return; }

    // Decode fuzzing parameters from input
    let num_workers = (data[0] % 8) as u64 + 1; // 1..=8 workers
    let isl_tokens = u16::from_le_bytes([data[1], data[2]]) as usize;
    if isl_tokens == 0 { return; }
    let block_size = (data[3] % 64) as u32 + 1; // 1..=64

    // Temperature: interpret 2 bytes as f64 in interesting range
    let temp_raw = u16::from_le_bytes([data[4], data[5]]);
    let temperature = match temp_raw % 8 {
        0 => 0.0,             // greedy
        1 => 0.001,           // near-greedy
        2 => 0.1,             // low temp
        3 => 1.0,             // standard
        4 => 10.0,            // high temp
        5 => 100.0,           // very high
        6 => f64::MIN_POSITIVE, // smallest positive
        _ => (temp_raw as f64) / 1000.0, // arbitrary
    };

    let overlap_weight_raw = data[6];
    let overlap_weight = match overlap_weight_raw % 6 {
        0 => 0.0,
        1 => 0.5,
        2 => 1.0,
        3 => 10.0,
        4 => -1.0,  // negative weight
        _ => (overlap_weight_raw as f64) / 10.0,
    };

    // Build workers
    let mut workers: HashMap<u64, FuzzWorkerConfig> = HashMap::new();
    for i in 0..num_workers {
        let byte_idx = 7 + (i as usize * 2) % (data.len() - 7).max(1);
        let dp_size = (data.get(byte_idx).copied().unwrap_or(1) % 4) as u32 + 1;
        workers.insert(i, FuzzWorkerConfig {
            dp_size,
            dp_start_rank: 0,
            max_batched: Some(1024),
            total_blocks: Some(256),
        });
    }

    // Build overlap scores from remaining bytes
    let mut overlaps = OverlapScores::new();
    let mut decode_blocks = HashMap::new();
    let mut prefill_tokens = HashMap::new();
    let score_start = 7 + num_workers as usize * 2;

    for (worker_id, config) in &workers {
        for dp_rank in 0..config.dp_size {
            let w = WorkerWithDpRank::new(*worker_id, dp_rank);
            let idx = score_start + (worker_id * 3 + dp_rank as u64) as usize;
            let score_byte = data.get(idx % data.len()).copied().unwrap_or(0);
            let score = (score_byte % 32) as u32;
            overlaps.scores.insert(w, score);
            overlaps.tree_sizes.insert(w, score as usize * 2);

            let decode_byte = data.get((idx + 1) % data.len()).copied().unwrap_or(0);
            decode_blocks.insert(w, decode_byte as usize);

            let prefill_byte = data.get((idx + 2) % data.len()).copied().unwrap_or(0);
            prefill_tokens.insert(w, (prefill_byte as usize).max(1));
        }
    }

    let mut config = KvRouterConfig::default();
    config.overlap_score_weight = overlap_weight;
    config.router_temperature = temperature;

    let selector = DefaultWorkerSelector::new(Some(config));

    let request = SchedulingRequest {
        maybe_request_id: None,
        token_seq: None,
        isl_tokens,
        overlaps,
        decode_blocks,
        prefill_tokens,
        router_config_override: None,
        update_states: false,
        lora_name: None,
        priority_jump: 0.0,
        expected_output_tokens: None,
        allowed_worker_ids: None,
        resp_tx: None,
    };

    // This must not panic (NaN, division by zero, etc.)
    let result = selector.select_worker(&workers, &request, block_size);

    match result {
        Ok(selection) => {
            // Sanity checks on the result
            assert!(workers.contains_key(&selection.worker.worker_id),
                "Selected worker {} not in worker set", selection.worker.worker_id);
            assert!(selection.required_blocks > 0,
                "Required blocks should be > 0 for isl_tokens > 0");
        }
        Err(_) => {} // NoEndpoints is fine
    }

    // Also test with allowed_worker_ids restriction
    if num_workers >= 2 {
        let mut allowed = HashSet::new();
        allowed.insert(0u64);
        let restricted_request = SchedulingRequest {
            maybe_request_id: None,
            token_seq: None,
            isl_tokens,
            overlaps: request.overlaps.clone(),
            decode_blocks: request.decode_blocks.clone(),
            prefill_tokens: request.prefill_tokens.clone(),
            router_config_override: None,
            update_states: false,
            lora_name: None,
            priority_jump: 0.0,
            expected_output_tokens: None,
            allowed_worker_ids: Some(allowed),
            resp_tx: None,
        };
        let result = selector.select_worker(&workers, &restricted_request, block_size);
        if let Ok(selection) = result {
            assert_eq!(selection.worker.worker_id, 0,
                "With allowed_worker_ids={{0}}, must select worker 0");
        }
    }
});
