#![no_main]
use libfuzzer_sys::fuzz_target;
use dynamo_parsers::reasoning::ReasoningParser;
use dynamo_parsers_fuzz::{StreamingChunker, select_parser_type, truncate};

fuzz_target!(|data: &[u8]| {
    if data.len() < 3 { return; }
    let Ok(s) = std::str::from_utf8(&data[2..]) else { return };
    if s.is_empty() { return; }

    let parser_type = select_parser_type(data[0]);

    let mut oneshot = parser_type.get_reasoning_parser();
    let oneshot_result = oneshot.detect_and_parse_reasoning(s, &[]);

    let mut streaming = parser_type.get_reasoning_parser();
    let mut stream_reasoning = String::new();
    let mut stream_normal = String::new();
    for chunk in StreamingChunker::new(s, data[1], data[1]) {
        let r = streaming.parse_reasoning_streaming_incremental(chunk, &[]);
        stream_reasoning.push_str(&r.reasoning_text);
        stream_normal.push_str(&r.normal_text);
    }

    let cs = data[1] % 4;
    assert_eq!(oneshot_result.reasoning_text, stream_reasoning,
        "Reasoning mismatch for {parser_type:?} (cs={cs}).\n\
         Input: {:?}\nOne-shot: {:?}\nStreaming: {:?}",
        truncate(s, 200), truncate(&oneshot_result.reasoning_text, 200), truncate(&stream_reasoning, 200));
    assert_eq!(oneshot_result.normal_text, stream_normal,
        "Normal mismatch for {parser_type:?} (cs={cs}).\n\
         Input: {:?}\nOne-shot: {:?}\nStreaming: {:?}",
        truncate(s, 200), truncate(&oneshot_result.normal_text, 200), truncate(&stream_normal, 200));
});
