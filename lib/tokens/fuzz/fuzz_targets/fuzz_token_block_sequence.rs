#![no_main]
use libfuzzer_sys::fuzz_target;

use dynamo_tokens::TokenBlockSequence;

fuzz_target!(|data: &[u8]| {
    if data.len() < 3 {
        return;
    }

    // First byte: block_size (1-16 to keep it reasonable)
    let block_size = (data[0] % 16) as u32 + 1;

    // Second byte: salt_hash control
    let salt_hash = if data[1] & 1 != 0 {
        Some(u64::from_le_bytes([
            data[1],
            data.get(2).copied().unwrap_or(0),
            0, 0, 0, 0, 0, 0,
        ]))
    } else {
        None
    };

    // Build initial tokens from remaining data
    let token_data = &data[2..];
    let initial_tokens: Vec<u32> = token_data
        .chunks(2)
        .map(|c| {
            let lo = c[0] as u32;
            let hi = c.get(1).copied().unwrap_or(0) as u32;
            (hi << 8) | lo
        })
        .collect();

    // Limit to prevent huge allocations
    if initial_tokens.len() > 256 {
        return;
    }

    let mut seq = TokenBlockSequence::from_slice(&initial_tokens, block_size, salt_hash);

    // Verify total_tokens after construction
    assert_eq!(
        seq.total_tokens(),
        initial_tokens.len(),
        "total_tokens mismatch after construction"
    );

    // Apply random operations based on remaining data
    let ops_data = if token_data.len() > 10 {
        &token_data[10..]
    } else {
        &[]
    };

    let mut pos = 0;
    while pos < ops_data.len() {
        let op = ops_data[pos] % 6;
        pos += 1;

        let prev_total = seq.total_tokens();

        match op {
            0 => {
                // Append single token
                if pos < ops_data.len() {
                    let token = ops_data[pos] as u32;
                    pos += 1;
                    let _ = seq.append(token);
                    assert_eq!(
                        seq.total_tokens(),
                        prev_total + 1,
                        "total_tokens should increase by 1 after append"
                    );
                }
            }
            1 => {
                // Pop
                let result = seq.pop();
                if prev_total > 0 {
                    assert!(result.is_some(), "pop on non-empty sequence returned None");
                    assert_eq!(
                        seq.total_tokens(),
                        prev_total - 1,
                        "total_tokens should decrease by 1 after pop"
                    );
                } else {
                    assert!(result.is_none(), "pop on empty sequence returned Some");
                }
            }
            2 => {
                // Truncate
                if pos < ops_data.len() {
                    let new_len = ops_data[pos] as usize;
                    pos += 1;
                    if new_len <= prev_total {
                        let _ = seq.truncate(new_len);
                        assert_eq!(
                            seq.total_tokens(),
                            new_len,
                            "total_tokens mismatch after truncate to {}",
                            new_len
                        );
                    }
                }
            }
            3 => {
                // Unwind
                if pos < ops_data.len() {
                    let count = (ops_data[pos] as usize) % 8;
                    pos += 1;
                    if count <= prev_total {
                        let _ = seq.unwind(count);
                        assert_eq!(
                            seq.total_tokens(),
                            prev_total - count,
                            "total_tokens mismatch after unwind({})",
                            count
                        );
                    }
                }
            }
            4 => {
                // Reset
                seq.reset();
                assert_eq!(
                    seq.total_tokens(),
                    0,
                    "total_tokens should be 0 after reset"
                );
            }
            5 => {
                // Extend with multiple tokens
                if pos + 1 < ops_data.len() {
                    let count = (ops_data[pos] as usize % 8) + 1;
                    pos += 1;
                    let tokens: Vec<u32> = (0..count)
                        .map(|i| ops_data.get(pos + i).copied().unwrap_or(0) as u32)
                        .collect();
                    pos += count;
                    let _ = seq.extend(tokens.into());
                    assert_eq!(
                        seq.total_tokens(),
                        prev_total + count,
                        "total_tokens mismatch after extend({})",
                        count
                    );
                }
            }
            _ => unreachable!(),
        }
    }
});
