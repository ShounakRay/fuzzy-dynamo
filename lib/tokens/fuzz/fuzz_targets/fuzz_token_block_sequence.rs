#![no_main]
use libfuzzer_sys::fuzz_target;

use dynamo_tokens::TokenBlockSequence;

fuzz_target!(|data: &[u8]| {
    if data.len() < 3 {
        return;
    }

    let block_size = (data[0] % 16) as u32 + 1;
    let salt_hash = (data[1] & 1 != 0).then(|| u64::from_le_bytes([data[1], data.get(2).copied().unwrap_or(0), 0, 0, 0, 0, 0, 0]));

    let initial_tokens: Vec<u32> = data[2..].chunks(2)
        .map(|c| (c.get(1).copied().unwrap_or(0) as u32) << 8 | c[0] as u32)
        .collect();
    if initial_tokens.len() > 256 {
        return;
    }

    let mut seq = TokenBlockSequence::from_slice(&initial_tokens, block_size, salt_hash);
    assert_eq!(seq.total_tokens(), initial_tokens.len());

    let ops_data = if data.len() > 12 { &data[12..] } else { &[] };
    let mut pos = 0;

    while pos < ops_data.len() {
        let op = ops_data[pos] % 6;
        pos += 1;
        let prev = seq.total_tokens();

        match op {
            0 if pos < ops_data.len() => {
                let _ = seq.append(ops_data[pos] as u32);
                pos += 1;
                assert_eq!(seq.total_tokens(), prev + 1);
            }
            1 => {
                let r = seq.pop();
                if prev > 0 { assert!(r.is_some()); assert_eq!(seq.total_tokens(), prev - 1); }
                else { assert!(r.is_none()); }
            }
            2 if pos < ops_data.len() => {
                let new_len = ops_data[pos] as usize;
                pos += 1;
                if new_len <= prev {
                    let _ = seq.truncate(new_len);
                    assert_eq!(seq.total_tokens(), new_len);
                }
            }
            3 if pos < ops_data.len() => {
                let count = (ops_data[pos] as usize) % 8;
                pos += 1;
                if count <= prev {
                    let _ = seq.unwind(count);
                    assert_eq!(seq.total_tokens(), prev - count);
                }
            }
            4 => {
                seq.reset();
                assert_eq!(seq.total_tokens(), 0);
            }
            5 if pos + 1 < ops_data.len() => {
                let count = (ops_data[pos] as usize % 8) + 1;
                pos += 1;
                let tokens: Vec<u32> = (0..count).map(|i| ops_data.get(pos + i).copied().unwrap_or(0) as u32).collect();
                pos += count;
                let _ = seq.extend(tokens.into());
                assert_eq!(seq.total_tokens(), prev + count);
            }
            _ => {}
        }
    }
});
