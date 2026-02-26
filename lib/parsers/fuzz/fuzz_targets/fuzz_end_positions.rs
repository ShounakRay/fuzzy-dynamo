#![no_main]
use libfuzzer_sys::fuzz_target;
use dynamo_parsers::*;

// Fuzz find_tool_call_end_position with all parser names.
// Asserts the returned position is always <= input length (a violation
// would cause out-of-bounds slicing in callers).
fuzz_target!(|data: &[u8]| {
    let Ok(s) = std::str::from_utf8(data) else { return };

    let parsers = [
        "hermes", "nemotron_deci", "llama3_json", "mistral", "phi4",
        "pythonic", "harmony", "deepseek_v3", "deepseek_v3_1", "deepseek_v3_2",
        "qwen3_coder", "jamba", "minimax_m2", "glm47", "kimi_k2", "default",
    ];

    for name in parsers {
        let pos = find_tool_call_end_position(s, Some(name));
        assert!(pos <= s.len(), "end_position({name}) = {pos} > len {}", s.len());
    }
    let pos = find_tool_call_end_position(s, None);
    assert!(pos <= s.len(), "end_position(None) = {pos} > len {}", s.len());
});
