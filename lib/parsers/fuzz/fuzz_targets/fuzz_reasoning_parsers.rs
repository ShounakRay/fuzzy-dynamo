#![no_main]
use libfuzzer_sys::fuzz_target;
use dynamo_parsers::reasoning::{ReasoningParser, ReasoningParserType};

// Fuzz all reasoning parser variants with detect_and_parse_reasoning.
// Each parser type gets a fresh instance per input.
fuzz_target!(|data: &[u8]| {
    let Ok(s) = std::str::from_utf8(data) else { return };

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

    for t in types {
        let mut parser = t.get_reasoning_parser();
        let _ = parser.detect_and_parse_reasoning(s, &[]);
    }
});
