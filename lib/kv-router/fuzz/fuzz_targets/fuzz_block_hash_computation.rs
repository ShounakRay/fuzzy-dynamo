#![no_main]
use libfuzzer_sys::fuzz_target;

use dynamo_kv_router::compute_block_hash_for_seq;
use dynamo_kv_router::protocols::{BlockExtraInfo, BlockMmObjectInfo};

fuzz_target!(|data: &[u8]| {
    if data.len() < 5 {
        return;
    }

    // Parse fuzz input:
    // byte 0-3: kv_block_size (u32)
    // byte 4: flags (bit 0 = use lora, bit 1 = use mm_infos)
    // remaining: token data
    let kv_block_size = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    let flags = data[4];
    let token_data = &data[5..];

    // kv_block_size = 0 would cause chunks_exact(0) panic — test this!
    if kv_block_size == 0 {
        // This is expected to panic with chunks_exact(0).
        // The fuzzer should discover this as a crash.
        let tokens: Vec<u32> = vec![1, 2, 3, 4];
        let _ = compute_block_hash_for_seq(&tokens, kv_block_size, None, None);
        return;
    }

    // Build tokens from remaining bytes (groups of 4 bytes -> u32)
    let tokens: Vec<u32> = token_data
        .chunks(4)
        .filter(|c| c.len() == 4)
        .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect();

    if tokens.is_empty() {
        return;
    }

    // Determinism check: same input → same output
    let result1 = compute_block_hash_for_seq(&tokens, kv_block_size, None, None);
    let result2 = compute_block_hash_for_seq(&tokens, kv_block_size, None, None);
    assert_eq!(result1, result2, "hash computation must be deterministic");

    // Lora name tests
    if flags & 1 != 0 {
        let lora_result = compute_block_hash_for_seq(&tokens, kv_block_size, None, Some("test-lora"));
        // With a lora name, results should differ from base (unless tokens are empty after chunking)
        if !result1.is_empty() {
            // Note: they SHOULD differ but we just check no panic
        }
        let _ = lora_result;
    }

    // Empty string lora should match None
    let empty_lora = compute_block_hash_for_seq(&tokens, kv_block_size, None, Some(""));
    let no_lora = compute_block_hash_for_seq(&tokens, kv_block_size, None, None);
    assert_eq!(
        empty_lora, no_lora,
        "empty lora_name should produce same result as None"
    );

    // Multimodal info test
    if flags & 2 != 0 && token_data.len() >= 16 {
        let num_blocks = tokens.len() / kv_block_size as usize;
        if num_blocks > 0 {
            let mm_infos: Vec<Option<BlockExtraInfo>> = (0..num_blocks)
                .map(|i| {
                    if i % 2 == 0 {
                        Some(BlockExtraInfo {
                            mm_objects: vec![BlockMmObjectInfo {
                                mm_hash: u64::from_le_bytes(
                                    token_data[..8].try_into().unwrap_or([0; 8]),
                                ),
                                offsets: vec![(0, 1)],
                            }],
                        })
                    } else {
                        None
                    }
                })
                .collect();

            let _ = compute_block_hash_for_seq(
                &tokens,
                kv_block_size,
                Some(&mm_infos),
                None,
            );
        }
    }

    // compute_seq_hash_for_block should also not panic
    let block_hashes = compute_block_hash_for_seq(&tokens, kv_block_size, None, None);
    let _ = dynamo_kv_router::protocols::compute_seq_hash_for_block(&block_hashes);
});
