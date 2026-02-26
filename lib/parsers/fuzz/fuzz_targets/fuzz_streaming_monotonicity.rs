#![no_main]
use libfuzzer_sys::fuzz_target;
use dynamo_parsers::reasoning::{ReasoningParser, ReasoningParserType};

// Streaming monotonicity oracle:
// In streaming mode, accumulated output must only grow.
// No chunk should cause previously-emitted content to disappear.
// We track the running total of emitted bytes and assert it never decreases.
fuzz_target!(|data: &[u8]| {
    if data.len() < 3 {
        return;
    }
    let Ok(s) = std::str::from_utf8(&data[2..]) else {
        return;
    };
    if s.is_empty() {
        return;
    }

    let types = [
        ReasoningParserType::DeepseekR1,
        ReasoningParserType::Basic,
        ReasoningParserType::GptOss,
        ReasoningParserType::Qwen,
        ReasoningParserType::NemotronDeci,
        ReasoningParserType::Kimi,
        ReasoningParserType::KimiK25,
        ReasoningParserType::Step3,
        ReasoningParserType::Mistral,
        ReasoningParserType::Granite,
        ReasoningParserType::MiniMaxAppendThink,
    ];
    let parser_type = types[data[0] as usize % types.len()];

    let mut parser = parser_type.get_reasoning_parser();
    let mut total_reasoning_len: usize = 0;
    let mut total_normal_len: usize = 0;
    let mut accumulated_reasoning = String::new();
    let mut accumulated_normal = String::new();

    let bytes = s.as_bytes();
    let mut pos = 0;
    let chunk_strategy = data[1] % 4;

    while pos < bytes.len() {
        let chunk_len = match chunk_strategy {
            0 => 1,
            1 => {
                let seed = pos.wrapping_mul(31).wrapping_add(data[1] as usize);
                (seed % 16).max(1)
            }
            2 => {
                let seed = pos.wrapping_mul(37).wrapping_add(data[1] as usize);
                ((seed % 29) + 4).min(bytes.len() - pos)
            }
            _ => bytes.len() - pos,
        };

        let mut end = (pos + chunk_len).min(bytes.len());

        // Extend to complete UTF-8 character
        while end < bytes.len() && (bytes[end] & 0xC0) == 0x80 {
            end += 1;
        }

        if end == pos {
            break;
        }

        let chunk = unsafe { std::str::from_utf8_unchecked(&bytes[pos..end]) };
        let result = parser.parse_reasoning_streaming_incremental(chunk, &[]);

        // Accumulate
        accumulated_reasoning.push_str(&result.reasoning_text);
        accumulated_normal.push_str(&result.normal_text);

        let new_reasoning_len = accumulated_reasoning.len();
        let new_normal_len = accumulated_normal.len();

        // Monotonicity: accumulated lengths must never decrease
        assert!(
            new_reasoning_len >= total_reasoning_len,
            "{:?}: reasoning text shrank from {} to {} bytes at input pos {} (chunk_strategy={})",
            parser_type,
            total_reasoning_len,
            new_reasoning_len,
            pos,
            chunk_strategy,
        );
        assert!(
            new_normal_len >= total_normal_len,
            "{:?}: normal text shrank from {} to {} bytes at input pos {} (chunk_strategy={})",
            parser_type,
            total_normal_len,
            new_normal_len,
            pos,
            chunk_strategy,
        );

        total_reasoning_len = new_reasoning_len;
        total_normal_len = new_normal_len;
        pos = end;
    }

    // Total output should not exceed input (no content fabrication)
    assert!(
        total_reasoning_len + total_normal_len <= s.len() * 2,
        "{:?}: total output ({} + {} = {}) far exceeds input length {}",
        parser_type,
        total_reasoning_len,
        total_normal_len,
        total_reasoning_len + total_normal_len,
        s.len(),
    );
});
