#![no_main]
use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use dynamo_parsers::reasoning::ReasoningParser;
use dynamo_parsers_fuzz::{StreamingChunker, select_parser_type, truncate};

#[derive(Debug, Arbitrary)]
struct FuzzInput {
    parser_type: u8,
    chunk_size: u8,
    text: String,
}

fuzz_target!(|input: FuzzInput| {
    let s = &input.text;
    if s.is_empty() { return; }

    let parser_type = select_parser_type(input.parser_type);

    let mut oneshot = parser_type.get_reasoning_parser();
    let oneshot_result = oneshot.detect_and_parse_reasoning(s, &[]);

    let mut streaming = parser_type.get_reasoning_parser();
    let mut stream_reasoning = String::new();
    let mut stream_normal = String::new();
    for chunk in StreamingChunker::new(s, input.chunk_size, input.chunk_size) {
        let r = streaming.parse_reasoning_streaming_incremental(chunk, &[]);
        stream_reasoning.push_str(&r.reasoning_text);
        stream_normal.push_str(&r.normal_text);
    }

    let cs = input.chunk_size % 4;
    assert_eq!(oneshot_result.reasoning_text, stream_reasoning,
        "Reasoning mismatch for {parser_type:?} (cs={cs}).\n\
         Input: {:?}\nOne-shot: {:?}\nStreaming: {:?}",
        truncate(s, 200), truncate(&oneshot_result.reasoning_text, 200), truncate(&stream_reasoning, 200));
    assert_eq!(oneshot_result.normal_text, stream_normal,
        "Normal mismatch for {parser_type:?} (cs={cs}).\n\
         Input: {:?}\nOne-shot: {:?}\nStreaming: {:?}",
        truncate(s, 200), truncate(&oneshot_result.normal_text, 200), truncate(&stream_normal, 200));
});
