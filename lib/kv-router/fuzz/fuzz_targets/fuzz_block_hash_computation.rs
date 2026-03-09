#![no_main]
use libfuzzer_sys::fuzz_target;

use dynamo_kv_router::compute_block_hash_for_seq;
use dynamo_kv_router::protocols::{BlockExtraInfo, BlockMmObjectInfo};

fuzz_target!(|data: &[u8]| {
    if data.len() < 5 {
        return;
    }

    let kv_block_size = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    let flags = data[4];
    let token_data = &data[5..];

    // kv_block_size=0 causes chunks_exact(0) panic — let fuzzer discover this
    if kv_block_size == 0 {
        let _ = compute_block_hash_for_seq(&[1, 2, 3, 4], kv_block_size, None, None);
        return;
    }

    let tokens: Vec<u32> = token_data
        .chunks_exact(4)
        .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect();
    if tokens.is_empty() {
        return;
    }

    // Determinism
    let r1 = compute_block_hash_for_seq(&tokens, kv_block_size, None, None);
    let r2 = compute_block_hash_for_seq(&tokens, kv_block_size, None, None);
    assert_eq!(r1, r2, "hash must be deterministic");

    // Lora
    if flags & 1 != 0 {
        let _ = compute_block_hash_for_seq(&tokens, kv_block_size, None, Some("test-lora"));
    }

    // Empty lora == None
    let a = compute_block_hash_for_seq(&tokens, kv_block_size, None, Some(""));
    let b = compute_block_hash_for_seq(&tokens, kv_block_size, None, None);
    assert_eq!(a, b, "empty lora should equal None");

    // Multimodal info
    if flags & 2 != 0 && token_data.len() >= 8 {
        let num_blocks = tokens.len() / kv_block_size as usize;
        if num_blocks > 0 {
            let mm_infos: Vec<Option<BlockExtraInfo>> = (0..num_blocks)
                .map(|i| {
                    (i % 2 == 0).then(|| BlockExtraInfo {
                        mm_objects: vec![BlockMmObjectInfo {
                            mm_hash: u64::from_le_bytes(token_data[..8].try_into().unwrap_or([0; 8])),
                            offsets: vec![(0, 1)],
                        }],
                    })
                })
                .collect();
            let _ = compute_block_hash_for_seq(&tokens, kv_block_size, Some(&mm_infos), None);
        }
    }

    // seq_hash_for_block should not panic
    let _ = dynamo_kv_router::protocols::compute_seq_hash_for_block(&r1);
});
