#![no_main]
use libfuzzer_sys::fuzz_target;
use dynamo_parsers::reasoning::ReasoningParser;
use dynamo_parsers_fuzz::{StreamingChunker, select_parser_type};

// Streaming monotonicity oracle:
// In streaming mode, accumulated output must only grow.
// No chunk should cause previously-emitted content to disappear.
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

    let parser_type = select_parser_type(data[0]);
    let chunk_strategy = data[1] % 4;

    let mut parser = parser_type.get_reasoning_parser();
    let mut total_reasoning_len: usize = 0;
    let mut total_normal_len: usize = 0;
    let mut accumulated_reasoning = String::new();
    let mut accumulated_normal = String::new();
    let mut pos = 0;

    for chunk in StreamingChunker::new(s, data[1], data[1]) {
        let result = parser.parse_reasoning_streaming_incremental(chunk, &[]);

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
        pos += chunk.len();
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
