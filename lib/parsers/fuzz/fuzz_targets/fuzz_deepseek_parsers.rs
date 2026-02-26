#![no_main]
use libfuzzer_sys::fuzz_target;
use dynamo_parsers::tool_calling::json::{
    JsonParserConfig, JsonParserType,
    detect_tool_call_start_basic_json, try_tool_call_parse_basic_json,
    detect_tool_call_start_deepseek_v3, parse_tool_calls_deepseek_v3,
    detect_tool_call_start_deepseek_v3_1, parse_tool_calls_deepseek_v3_1,
};

// Fuzz the DeepSeek V3/V3.1 parsers directly with their specific configs.
// These parsers use unicode tokens (e.g. <｜tool▁call▁begin｜>) and have
// separate regex-based extraction logic worth testing independently.
fuzz_target!(|data: &[u8]| {
    let Ok(s) = std::str::from_utf8(data) else { return };

    // Basic JSON parser (generic path)
    let basic_cfg = JsonParserConfig::default();
    let _ = detect_tool_call_start_basic_json(s, &basic_cfg);
    let _ = try_tool_call_parse_basic_json(s, &basic_cfg, None);

    // DeepSeek V3 with its actual config
    let v3_cfg = JsonParserConfig {
        parser_type: JsonParserType::DeepseekV3,
        tool_call_start_tokens: vec![
            "<｜tool▁calls▁begin｜>".to_string(),
            "<｜tool▁call▁begin｜>".to_string(),
        ],
        tool_call_end_tokens: vec![
            "<｜tool▁calls▁end｜>".to_string(),
            "<｜tool▁call▁end｜>".to_string(),
        ],
        tool_call_separator_tokens: vec!["<｜tool▁sep｜>".to_string()],
        ..JsonParserConfig::default()
    };
    let _ = detect_tool_call_start_deepseek_v3(s, &v3_cfg);
    let _ = parse_tool_calls_deepseek_v3(s, &v3_cfg, None);

    // DeepSeek V3.1 with its actual config
    let v31_cfg = JsonParserConfig {
        parser_type: JsonParserType::DeepseekV31,
        tool_call_start_tokens: vec![
            "<｜tool▁calls▁begin｜>".to_string(),
            "<｜tool▁call▁begin｜>".to_string(),
        ],
        tool_call_end_tokens: vec![
            "<｜tool▁calls▁end｜>".to_string(),
            "<｜tool▁call▁end｜>".to_string(),
        ],
        tool_call_separator_tokens: vec!["<｜tool▁sep｜>".to_string()],
        ..JsonParserConfig::default()
    };
    let _ = detect_tool_call_start_deepseek_v3_1(s, &v31_cfg);
    let _ = parse_tool_calls_deepseek_v3_1(s, &v31_cfg, None);
});
