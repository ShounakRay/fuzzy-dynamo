#![no_main]
use libfuzzer_sys::fuzz_target;
use arbitrary::Arbitrary;

use dynamo_tokens::TokenBlockSequence;

#[derive(Debug, Arbitrary)]
enum TokenOp {
    Append { token: u32 },
    Pop,
    Truncate { len: u16 },
    Unwind { count: u8 },
    Reset,
    Extend { tokens: Vec<u32> },
}

#[derive(Debug, Arbitrary)]
struct FuzzInput {
    block_size: u16,
    initial_tokens: Vec<u32>,
    ops: Vec<TokenOp>,
}

fuzz_target!(|input: FuzzInput| {
    let block_size = (input.block_size % 16).max(1) as u32;
    let initial_tokens: Vec<u32> = input.initial_tokens.iter().take(256).copied().collect();
    let ops = if input.ops.len() > 128 { &input.ops[..128] } else { &input.ops };

    let mut seq = TokenBlockSequence::from_slice(&initial_tokens, block_size, None);
    let mut expected_total = initial_tokens.len();
    assert_eq!(seq.total_tokens(), expected_total);

    for op in ops {
        match op {
            TokenOp::Append { token } => {
                let _ = seq.append(*token);
                expected_total += 1;
                assert_eq!(seq.total_tokens(), expected_total);
            }
            TokenOp::Pop => {
                let r = seq.pop();
                if expected_total > 0 {
                    assert!(r.is_some());
                    expected_total -= 1;
                } else {
                    assert!(r.is_none());
                }
                assert_eq!(seq.total_tokens(), expected_total);
            }
            TokenOp::Truncate { len } => {
                let new_len = *len as usize;
                if new_len <= expected_total {
                    let _ = seq.truncate(new_len);
                    expected_total = new_len;
                    assert_eq!(seq.total_tokens(), expected_total);
                }
            }
            TokenOp::Unwind { count } => {
                let c = *count as usize;
                if c <= expected_total {
                    let _ = seq.unwind(c);
                    expected_total -= c;
                    assert_eq!(seq.total_tokens(), expected_total);
                }
            }
            TokenOp::Reset => {
                seq.reset();
                expected_total = 0;
                assert_eq!(seq.total_tokens(), expected_total);
            }
            TokenOp::Extend { tokens } => {
                let tokens: Vec<u32> = tokens.iter().take(16).copied().collect();
                let count = tokens.len();
                let _ = seq.extend(tokens.into());
                expected_total += count;
                assert_eq!(seq.total_tokens(), expected_total);
            }
        }
    }
});
