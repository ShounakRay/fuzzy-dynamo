#![no_main]
use std::sync::Arc;
use std::sync::atomic::AtomicU32;

use libfuzzer_sys::fuzz_target;
use dynamo_kv_router::zmq_wire::{
    parse_mm_hash_from_extra_key, extra_keys_to_block_mm_infos,
    convert_event, KvEventBatch, RawKvEvent,
};

fuzz_target!(|data: &[u8]| {
    // --- KvEventBatch: msgpack deserialization (real ZMQ wire format) ---
    if let Ok(batch) = rmp_serde::from_slice::<KvEventBatch>(data) {
        let warning_count = Arc::new(AtomicU32::new(0));
        let dp_rank = batch.data_parallel_rank.unwrap_or(0) as u32;
        for (i, event) in batch.events.into_iter().enumerate() {
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                convert_event(event, i as u64, 16, dp_rank, &warning_count)
            }));
        }
    }

    // --- RawKvEvent: single event msgpack deserialization + convert ---
    if let Ok(event) = rmp_serde::from_slice::<RawKvEvent>(data) {
        let warning_count = Arc::new(AtomicU32::new(0));
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            convert_event(event, 0, 16, 0, &warning_count)
        }));
    }

    // --- KvEventBatch: full JSON deserialization + convert_event pipeline ---
    if let Ok(batch) = serde_json::from_slice::<KvEventBatch>(data) {
        let warning_count = Arc::new(AtomicU32::new(0));
        let dp_rank = batch.data_parallel_rank.unwrap_or(0) as u32;
        for (i, event) in batch.events.into_iter().enumerate() {
            // This exercises the full ZMQ → router pipeline including
            // create_stored_blocks which can OOB on mismatched sizes
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                convert_event(event, i as u64, 16, dp_rank, &warning_count)
            }));
        }
    }

    // --- RawKvEvent: single event deserialization + convert ---
    if let Ok(event) = serde_json::from_slice::<RawKvEvent>(data) {
        let warning_count = Arc::new(AtomicU32::new(0));
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            convert_event(event, 0, 16, 0, &warning_count)
        }));
    }

    // --- Original parse_mm_hash_from_extra_key tests ---
    let Ok(s) = std::str::from_utf8(data) else { return };

    // parse_mm_hash_from_extra_key must not panic
    let result = parse_mm_hash_from_extra_key(s);

    // If the string is exactly 64 hex chars, we should get Some
    if s.len() == 64 && s.chars().all(|c| c.is_ascii_hexdigit()) {
        assert!(result.is_some(),
            "Expected Some for 64-char hex string, got None: {:?}", &s[..16]);
    }
    // If it's not 64 chars or has non-hex, should be None
    if s.len() != 64 || !s.chars().all(|c| c.is_ascii_hexdigit()) {
        assert!(result.is_none(),
            "Expected None for non-64-hex string (len={}), got {:?}", s.len(), result);
    }

    // Determinism
    let result2 = parse_mm_hash_from_extra_key(s);
    assert_eq!(result, result2, "parse_mm_hash_from_extra_key not deterministic");

    // extra_keys_to_block_mm_infos with fuzz-generated nested structure
    if data.len() >= 4 {
        let num_blocks = (data[0] % 8) as usize;
        let mut blocks: Vec<Option<Vec<String>>> = Vec::new();
        let mut pos = 1;
        for _ in 0..num_blocks {
            if pos >= data.len() { break; }
            match data[pos] % 3 {
                0 => { blocks.push(None); pos += 1; }
                1 => { blocks.push(Some(vec![])); pos += 1; }
                _ => {
                    pos += 1;
                    let count = if pos < data.len() { (data[pos] % 4) as usize + 1 } else { 1 };
                    pos += 1;
                    let mut keys = Vec::new();
                    for _ in 0..count {
                        if pos + 2 > data.len() { break; }
                        let key_len = (data[pos] % 32) as usize;
                        pos += 1;
                        let end = (pos + key_len).min(data.len());
                        if let Ok(key) = std::str::from_utf8(&data[pos..end]) {
                            keys.push(key.to_string());
                        }
                        pos = end;
                    }
                    blocks.push(Some(keys));
                }
            }
        }

        let _ = extra_keys_to_block_mm_infos(Some(blocks));
    }

    // None case
    let _ = extra_keys_to_block_mm_infos(None);
});
