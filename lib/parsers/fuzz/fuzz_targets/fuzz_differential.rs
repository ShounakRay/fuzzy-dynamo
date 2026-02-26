#![no_main]
use libfuzzer_sys::fuzz_target;
use dynamo_parsers::reasoning::ReasoningParser;
use dynamo_parsers_fuzz::{StreamingChunker, select_parser_type, truncate};

// Differential fuzz harness: compare streaming vs one-shot reasoning parsing.
//
// Feed the same input to detect_and_parse_reasoning (one-shot) and
// parse_reasoning_streaming_incremental (chunked), then assert they produce
// identical reasoning_text and normal_text.
fuzz_target!(|data: &[u8]| {
    if data.len() < 3 { return; }
    let Ok(s) = std::str::from_utf8(&data[2..]) else { return };
    if s.is_empty() { return; }

    let parser_type = select_parser_type(data[0]);

    // One-shot parse
    let mut oneshot_parser = parser_type.get_reasoning_parser();
    let oneshot_result = oneshot_parser.detect_and_parse_reasoning(s, &[]);

    // Streaming parse
    let mut streaming_parser = parser_type.get_reasoning_parser();
    let mut streaming_reasoning = String::new();
    let mut streaming_normal = String::new();

    for chunk in StreamingChunker::new(s, data[1], data[1]) {
        let result = streaming_parser.parse_reasoning_streaming_incremental(chunk, &[]);
        streaming_reasoning.push_str(&result.reasoning_text);
        streaming_normal.push_str(&result.normal_text);
    }

    let chunk_strategy = data[1] % 4;

    assert_eq!(
        oneshot_result.reasoning_text, streaming_reasoning,
        "Reasoning text mismatch for {:?} (chunk_strategy={}).\n\
         Input ({} bytes): {:?}\n\
         One-shot reasoning: {:?}\n\
         Streaming reasoning: {:?}",
        parser_type, chunk_strategy,
        s.len(), truncate(s, 200),
        truncate(&oneshot_result.reasoning_text, 200),
        truncate(&streaming_reasoning, 200),
    );

    assert_eq!(
        oneshot_result.normal_text, streaming_normal,
        "Normal text mismatch for {:?} (chunk_strategy={}).\n\
         Input ({} bytes): {:?}\n\
         One-shot normal: {:?}\n\
         Streaming normal: {:?}",
        parser_type, chunk_strategy,
        s.len(), truncate(s, 200),
        truncate(&oneshot_result.normal_text, 200),
        truncate(&streaming_normal, 200),
    );
});
