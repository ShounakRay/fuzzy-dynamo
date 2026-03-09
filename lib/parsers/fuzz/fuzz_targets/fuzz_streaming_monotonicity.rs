#![no_main]
use libfuzzer_sys::fuzz_target;
use dynamo_parsers::reasoning::ReasoningParser;
use dynamo_parsers_fuzz::{StreamingChunker, select_parser_type};

fuzz_target!(|data: &[u8]| {
    if data.len() < 3 { return; }
    let Ok(s) = std::str::from_utf8(&data[2..]) else { return };
    if s.is_empty() { return; }

    let parser_type = select_parser_type(data[0]);
    let mut parser = parser_type.get_reasoning_parser();
    let mut reasoning_len: usize = 0;
    let mut normal_len: usize = 0;
    let mut reasoning = String::new();
    let mut normal = String::new();
    let mut pos = 0;

    for chunk in StreamingChunker::new(s, data[1], data[1]) {
        let result = parser.parse_reasoning_streaming_incremental(chunk, &[]);
        reasoning.push_str(&result.reasoning_text);
        normal.push_str(&result.normal_text);

        assert!(reasoning.len() >= reasoning_len,
            "{parser_type:?}: reasoning shrank at pos {pos}");
        assert!(normal.len() >= normal_len,
            "{parser_type:?}: normal text shrank at pos {pos}");

        reasoning_len = reasoning.len();
        normal_len = normal.len();
        pos += chunk.len();
    }

    assert!(reasoning_len + normal_len <= s.len() * 2,
        "{parser_type:?}: output far exceeds input");
});
