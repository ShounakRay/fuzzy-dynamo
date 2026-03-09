#![no_main]
use std::sync::Arc;
use std::sync::atomic::AtomicU32;

use libfuzzer_sys::fuzz_target;
use dynamo_kv_router::zmq_wire::{
    convert_event, create_stored_blocks, create_stored_block_from_parts,
    RawKvEvent,
};

fuzz_target!(|data: &[u8]| {
    if data.len() < 8 { return; }

    let kv_block_size = (data[0] as u32) % 33; // 0..=32, including 0 to test zero case
    let num_blocks = (data[1] % 8) as usize + 1; // 1..=8
    let token_count = (data[2] as usize) % 64; // 0..=63 tokens, possibly mismatched

    // Build token_ids from remaining data
    let mut token_ids: Vec<u32> = Vec::new();
    let mut pos = 3;
    for _ in 0..token_count {
        if pos + 4 > data.len() { break; }
        token_ids.push(u32::from_le_bytes([data[pos], data[pos+1], data[pos+2], data[pos+3]]));
        pos += 4;
    }

    // Build block hashes and num_block_tokens
    let num_block_tokens: Vec<u64> = vec![kv_block_size as u64; num_blocks];
    let block_hashes: Vec<u64> = (0..num_blocks).map(|i| i as u64 * 1000 + 1).collect();

    let warning_count = Arc::new(AtomicU32::new(0));

    // --- Test create_stored_blocks with potentially mismatched sizes ---
    // This can panic if token_ids is too short for the claimed blocks
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        create_stored_blocks(
            kv_block_size,
            &token_ids,
            &num_block_tokens,
            &block_hashes,
            None,
            &warning_count,
            None,
        )
    }));
    // Record if this panicked — that's a bug in the function
    if result.is_err() {
        // Panic detected — this is the bug we're looking for
        // Re-panic to save crash artifact
        create_stored_blocks(
            kv_block_size,
            &token_ids,
            &num_block_tokens,
            &block_hashes,
            None,
            &warning_count,
            None,
        );
    }

    // --- Test create_stored_block_from_parts with mismatched sizes ---
    let result2 = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        create_stored_block_from_parts(
            kv_block_size,
            42,
            &token_ids,
            None,
            None,
        )
    }));
    if result2.is_err() {
        create_stored_block_from_parts(
            kv_block_size,
            42,
            &token_ids,
            None,
            None,
        );
    }

    // --- Test convert_event with deserialized RawKvEvent ---
    // Try to deserialize the raw bytes as a RawKvEvent and feed through convert_event
    if let Ok(event) = serde_json::from_slice::<RawKvEvent>(data) {
        let warning_count2 = Arc::new(AtomicU32::new(0));
        let result3 = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            convert_event(event, 0, kv_block_size, 0, &warning_count2)
        }));
        if let Err(e) = result3 {
            // Re-deserialize and re-panic to save artifact
            let event = serde_json::from_slice::<RawKvEvent>(data).unwrap();
            let warning_count3 = Arc::new(AtomicU32::new(0));
            convert_event(event, 0, kv_block_size, 0, &warning_count3);
        }
    }
});
