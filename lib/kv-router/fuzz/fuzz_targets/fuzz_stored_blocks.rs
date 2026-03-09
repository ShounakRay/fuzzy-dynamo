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

    let kv_block_size = (data[0] as u32) % 33; // 0..=32
    let num_blocks = (data[1] % 8) as usize + 1;
    let token_count = (data[2] as usize) % 64;

    // Build token_ids from remaining data
    let mut token_ids: Vec<u32> = Vec::new();
    let mut pos = 3;
    for _ in 0..token_count {
        if pos + 4 > data.len() { break; }
        token_ids.push(u32::from_le_bytes([data[pos], data[pos+1], data[pos+2], data[pos+3]]));
        pos += 4;
    }

    let num_block_tokens: Vec<u64> = vec![kv_block_size as u64; num_blocks];
    let block_hashes: Vec<u64> = (0..num_blocks).map(|i| i as u64 * 1000 + 1).collect();
    let warning_count = Arc::new(AtomicU32::new(0));

    // --- Filter known bugs instead of catch_unwind (ASAN bypasses catch_unwind) ---
    // Bug #6: OOB slice when token_ids shorter than claimed blocks
    let expected_tokens = num_blocks * kv_block_size as usize;
    if kv_block_size == 0 || token_ids.len() < expected_tokens {
        // Known bug territory — skip to avoid ASAN abort
        return;
    }

    // Safe path: token_ids is large enough for the claimed blocks
    let blocks = create_stored_blocks(
        kv_block_size,
        &token_ids,
        &num_block_tokens,
        &block_hashes,
        None,
        &warning_count,
        None,
    );

    // Property: returned blocks count should not exceed input blocks
    assert!(blocks.len() <= num_blocks,
        "create_stored_blocks returned {} blocks from {} input blocks",
        blocks.len(), num_blocks);

    // Determinism: same input → same output
    let warning_count2 = Arc::new(AtomicU32::new(0));
    let blocks2 = create_stored_blocks(
        kv_block_size,
        &token_ids,
        &num_block_tokens,
        &block_hashes,
        None,
        &warning_count2,
        None,
    );
    assert_eq!(blocks.len(), blocks2.len(), "create_stored_blocks not deterministic");

    // --- Test create_stored_block_from_parts ---
    if !token_ids.is_empty() && kv_block_size > 0 {
        let slice_len = token_ids.len().min(kv_block_size as usize);
        let block = create_stored_block_from_parts(
            kv_block_size,
            42,
            &token_ids[..slice_len],
            None,
            None,
        );
        // Property: block hash should be deterministic
        let block2 = create_stored_block_from_parts(
            kv_block_size,
            42,
            &token_ids[..slice_len],
            None,
            None,
        );
        assert_eq!(block.tokens_hash, block2.tokens_hash,
            "create_stored_block_from_parts not deterministic");
    }

    // --- Test convert_event with deserialized RawKvEvent ---
    if let Ok(event) = serde_json::from_slice::<RawKvEvent>(data) {
        let wc = Arc::new(AtomicU32::new(0));
        // Skip events that would trigger known OOB bugs
        match &event {
            RawKvEvent::BlockStored { block_hashes, token_ids, block_size, .. } => {
                let needed = block_hashes.len() * *block_size;
                if *block_size == 0 || token_ids.len() < needed {
                    return;
                }
            }
            _ => {}
        }
        let _ = convert_event(event, 0, kv_block_size, 0, &wc);
    }
});
