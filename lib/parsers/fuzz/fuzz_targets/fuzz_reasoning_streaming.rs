#![no_main]
use libfuzzer_sys::fuzz_target;
use dynamo_parsers::reasoning::ReasoningParser;
use dynamo_parsers_fuzz::{StreamingChunker, select_parser_type};

/// Focused crash oracle for streaming reasoning parser with fuzz-controlled chunking.
fuzz_target!(|data: &[u8]| {
    if data.len() < 2 { return; }
    let Ok(s) = std::str::from_utf8(&data[2..]) else { return };
    if s.is_empty() { return; }

    let mut parser = select_parser_type(data[0]).get_reasoning_parser();
    for chunk in StreamingChunker::new(s, data[1], data[1]) {
        let _ = parser.parse_reasoning_streaming_incremental(chunk, &[]);
    }
});
