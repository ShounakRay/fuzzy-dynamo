#![no_main]
use libfuzzer_sys::fuzz_target;
use dynamo_parsers::*;

/// Focused crash oracle for detect_tool_call_start across all parser names.
fuzz_target!(|data: &[u8]| {
    let Ok(s) = std::str::from_utf8(data) else { return };

    for name in [
        "hermes", "nemotron_deci", "llama3_json", "mistral", "phi4",
        "pythonic", "harmony", "deepseek_v3", "deepseek_v3_1", "deepseek_v3_2",
        "qwen3_coder", "jamba", "minimax_m2", "glm47", "kimi_k2", "default",
    ] {
        let _ = detect_tool_call_start(s, Some(name));
    }
    let _ = detect_tool_call_start(s, None);
});
