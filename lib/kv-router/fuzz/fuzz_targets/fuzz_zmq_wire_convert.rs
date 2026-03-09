#![no_main]
use libfuzzer_sys::fuzz_target;

use std::sync::Arc;
use std::sync::atomic::AtomicU32;

use dynamo_kv_router::zmq_wire::{KvEventBatch, convert_event};

// End-to-end pipeline: decode msgpack → convert_event for each event.
// Known panic vectors:
// - compute_block_hash_for_seq with kv_block_size==0 (Bug 15)
// - create_stored_blocks slice panic when token_ids too short
fuzz_target!(|data: &[u8]| {
    if data.len() < 4 {
        return;
    }

    let kv_block_size = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    let remaining = &data[4..];

    // OOM guard only — do NOT guard against kv_block_size==0
    if kv_block_size > 8192 {
        return;
    }

    let batch: KvEventBatch = match rmp_serde::from_slice(remaining) {
        Ok(b) => b,
        Err(_) => return,
    };

    if batch.events.len() > 64 {
        return;
    }

    let dp_rank = batch.data_parallel_rank.unwrap_or(0) as u32;
    let warning_count = Arc::new(AtomicU32::new(0));

    for (i, event) in batch.events.into_iter().enumerate() {
        let _ = convert_event(event, i as u64, kv_block_size, dp_rank, &warning_count);
    }
});
