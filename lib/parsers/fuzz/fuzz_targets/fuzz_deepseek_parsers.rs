#![no_main]
use libfuzzer_sys::fuzz_target;
use dynamo_parsers::tool_calling::json::{
    JsonParserConfig, JsonParserType,
    detect_tool_call_start_basic_json, try_tool_call_parse_basic_json,
    detect_tool_call_start_deepseek_v3, parse_tool_calls_deepseek_v3,
    detect_tool_call_start_deepseek_v3_1, parse_tool_calls_deepseek_v3_1,
};

fuzz_target!(|data: &[u8]| {
    let Ok(s) = std::str::from_utf8(data) else { return };

    let basic_cfg = JsonParserConfig::default();
    let _ = detect_tool_call_start_basic_json(s, &basic_cfg);
    let _ = try_tool_call_parse_basic_json(s, &basic_cfg, None);

    let v3_cfg = JsonParserConfig {
        parser_type: JsonParserType::DeepseekV3,
        tool_call_start_tokens: vec!["<｜tool▁calls▁begin｜>".into(), "<｜tool▁call▁begin｜>".into()],
        tool_call_end_tokens: vec!["<｜tool▁calls▁end｜>".into(), "<｜tool▁call▁end｜>".into()],
        tool_call_separator_tokens: vec!["<｜tool▁sep｜>".into()],
        ..Default::default()
    };
    let _ = detect_tool_call_start_deepseek_v3(s, &v3_cfg);
    let _ = parse_tool_calls_deepseek_v3(s, &v3_cfg, None);

    let v31_cfg = JsonParserConfig { parser_type: JsonParserType::DeepseekV31, ..v3_cfg.clone() };
    let _ = detect_tool_call_start_deepseek_v3_1(s, &v31_cfg);
    let _ = parse_tool_calls_deepseek_v3_1(s, &v31_cfg, None);
});
