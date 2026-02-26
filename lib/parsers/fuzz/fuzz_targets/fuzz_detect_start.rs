#![no_main]
use libfuzzer_sys::fuzz_target;
use dynamo_parsers::*;

// Fuzz the top-level detect_tool_call_start with all parser names.
// This exercises the parser registry + each parser's detection logic.
fuzz_target!(|data: &[u8]| {
    let Ok(s) = std::str::from_utf8(data) else { return };

    let parsers = [
        "hermes", "nemotron_deci", "llama3_json", "mistral", "phi4",
        "pythonic", "harmony", "deepseek_v3", "deepseek_v3_1", "deepseek_v3_2",
        "qwen3_coder", "jamba", "minimax_m2", "glm47", "kimi_k2", "default",
    ];

    for name in parsers {
        let _ = detect_tool_call_start(s, Some(name));
    }
    // Also with None (auto-detect path)
    let _ = detect_tool_call_start(s, None);
});
