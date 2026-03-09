#![no_main]
use libfuzzer_sys::fuzz_target;

use dynamo_kv_router::protocols::{RequestExtraInfo, RequestMmObjectInfo};

fuzz_target!(|data: &[u8]| {
    if data.len() < 6 {
        return;
    }

    let block_size = u16::from_le_bytes([data[0], data[1]]) as usize;
    let total_tokens = u16::from_le_bytes([data[2], data[3]]) as usize;

    // block_size=0 causes division by zero — let fuzzer discover this
    if block_size == 0 {
        let info = RequestExtraInfo {
            mm_objects: vec![RequestMmObjectInfo { mm_hash: 42, offsets: vec![(0, 1)] }],
        };
        let _ = info.to_block_level(block_size, total_tokens);
        return;
    }

    if total_tokens == 0 {
        let info = RequestExtraInfo { mm_objects: vec![] };
        assert!(info.to_block_level(block_size, total_tokens).is_empty());
        return;
    }

    if block_size > 1024 || total_tokens > 4096 {
        return;
    }

    // Build mm_objects from remaining data
    let mut mm_objects = Vec::new();
    let mut pos = 4;
    while pos + 4 <= data.len() && mm_objects.len() < 8 {
        let mm_hash = u16::from_le_bytes([data[pos], data[pos + 1]]) as u64;
        let start = data[pos + 2] as usize;
        let end = start + data[pos + 3] as usize;
        pos += 4;
        if start < total_tokens && end <= total_tokens && start < end {
            mm_objects.push(RequestMmObjectInfo { mm_hash, offsets: vec![(start, end)] });
        }
    }

    let info = RequestExtraInfo { mm_objects };
    let result = info.to_block_level(block_size, total_tokens);

    let expected_blocks = total_tokens.div_ceil(block_size);
    assert_eq!(result.len(), expected_blocks, "block count mismatch");

    for (i, opt) in result.iter().enumerate() {
        if let Some(bi) = opt {
            for mm_obj in &bi.mm_objects {
                for &(start, end) in &mm_obj.offsets {
                    assert!(start < block_size, "block {i} start {start} >= block_size {block_size}");
                    assert!(end <= block_size || i == expected_blocks - 1,
                        "block {i} end {end} > block_size {block_size}");
                }
            }
        }
    }
});
