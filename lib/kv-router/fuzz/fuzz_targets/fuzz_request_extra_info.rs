#![no_main]
use libfuzzer_sys::fuzz_target;

use dynamo_kv_router::protocols::{RequestExtraInfo, RequestMmObjectInfo};

fuzz_target!(|data: &[u8]| {
    if data.len() < 6 {
        return;
    }

    // Parse fuzz input:
    // bytes 0-1: block_size (u16, we limit to avoid huge allocations)
    // bytes 2-3: total_tokens (u16)
    // remaining: mm_object data
    let block_size = u16::from_le_bytes([data[0], data[1]]) as usize;
    let total_tokens = u16::from_le_bytes([data[2], data[3]]) as usize;

    // block_size = 0 would cause division by zero in to_block_level
    // This is a likely crash — the fuzzer should discover it
    if block_size == 0 {
        let info = RequestExtraInfo {
            mm_objects: vec![RequestMmObjectInfo {
                mm_hash: 42,
                offsets: vec![(0, 1)],
            }],
        };
        let _ = info.to_block_level(block_size, total_tokens);
        return;
    }

    if total_tokens == 0 {
        let info = RequestExtraInfo {
            mm_objects: vec![],
        };
        let result = info.to_block_level(block_size, total_tokens);
        assert!(result.is_empty());
        return;
    }

    // Limit sizes to prevent OOM
    if block_size > 1024 || total_tokens > 4096 {
        return;
    }

    // Build mm_objects from remaining data
    let mm_data = &data[4..];
    let mut mm_objects = Vec::new();
    let mut pos = 0;

    while pos + 4 <= mm_data.len() && mm_objects.len() < 8 {
        let mm_hash = u16::from_le_bytes([mm_data[pos], mm_data[pos + 1]]) as u64;
        let start = mm_data[pos + 2] as usize;
        let end_delta = mm_data[pos + 3] as usize;
        let end = start + end_delta;
        pos += 4;

        // Only add valid offsets
        if start < total_tokens && end <= total_tokens && start < end {
            mm_objects.push(RequestMmObjectInfo {
                mm_hash,
                offsets: vec![(start, end)],
            });
        }
    }

    let info = RequestExtraInfo { mm_objects };
    let result = info.to_block_level(block_size, total_tokens);

    // Invariant: result length should equal ceil(total_tokens / block_size)
    let expected_blocks = total_tokens.div_ceil(block_size);
    assert_eq!(
        result.len(),
        expected_blocks,
        "expected {} blocks but got {}",
        expected_blocks,
        result.len()
    );

    // Invariant: all offsets in block-level info should be within [0, block_size)
    for (block_idx, block_info_opt) in result.iter().enumerate() {
        if let Some(block_info) = block_info_opt {
            for mm_obj in &block_info.mm_objects {
                for (start, end) in &mm_obj.offsets {
                    assert!(
                        *start < block_size,
                        "block {} mm_obj start {} >= block_size {}",
                        block_idx,
                        start,
                        block_size
                    );
                    assert!(
                        *end <= block_size || (block_idx == expected_blocks - 1),
                        "block {} mm_obj end {} > block_size {}",
                        block_idx,
                        end,
                        block_size
                    );
                }
            }
        }
    }
});
