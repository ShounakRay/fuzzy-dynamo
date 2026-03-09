#![no_main]
use std::sync::Arc;
use std::sync::atomic::AtomicU32;

use libfuzzer_sys::fuzz_target;
use rustc_hash::FxHashMap;

use dynamo_kv_router::ConcurrentRadixTree;
use dynamo_kv_router::zmq_wire::{convert_event, KvEventBatch, RawKvEvent};
use dynamo_kv_router::protocols::RouterEvent;

/// End-to-end fuzz target: JSON bytes → deserialize → convert_event → apply to radix tree.
/// This is the actual pipeline that runs in the ZMQ listener.
fuzz_target!(|data: &[u8]| {
    // Try batch format first, then single event
    let events: Vec<(RawKvEvent, u32)> = if let Ok(batch) = serde_json::from_slice::<KvEventBatch>(data) {
        let dp_rank = batch.data_parallel_rank.unwrap_or(0) as u32;
        batch.events.into_iter().map(|e| (e, dp_rank)).collect()
    } else if let Ok(event) = serde_json::from_slice::<RawKvEvent>(data) {
        vec![(event, 0)]
    } else {
        return;
    };

    let tree = ConcurrentRadixTree::new();
    let mut lookup = FxHashMap::default();
    let warning_count = Arc::new(AtomicU32::new(0));
    let worker_id = 0u64;

    for (i, (raw_event, dp_rank)) in events.into_iter().enumerate() {
        // convert_event can panic on mismatched sizes — catch those
        let converted = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            convert_event(raw_event, i as u64, 16, dp_rank, &warning_count)
        })) {
            Ok(event) => event,
            Err(_) => continue,
        };

        // Build RouterEvent and apply to tree
        let router_event = RouterEvent::new(worker_id, converted);
        let _ = tree.apply_event(&mut lookup, router_event);
    }

    // Query the tree after all events
    let query: Vec<dynamo_kv_router::protocols::LocalBlockHash> = vec![
        dynamo_kv_router::protocols::LocalBlockHash(0),
        dynamo_kv_router::protocols::LocalBlockHash(1),
    ];
    let _ = tree.find_matches_impl(&query, false);
});
