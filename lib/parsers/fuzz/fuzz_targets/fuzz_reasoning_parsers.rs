#![no_main]
use libfuzzer_sys::fuzz_target;
use dynamo_parsers::reasoning::ReasoningParser;
use dynamo_parsers_fuzz::REASONING_PARSER_TYPES;

// Fuzz all reasoning parser variants with detect_and_parse_reasoning.
// Each parser type gets a fresh instance per input.
fuzz_target!(|data: &[u8]| {
    let Ok(s) = std::str::from_utf8(data) else { return };

    for &t in REASONING_PARSER_TYPES {
        let mut parser = t.get_reasoning_parser();
        let _ = parser.detect_and_parse_reasoning(s, &[]);
    }
});
