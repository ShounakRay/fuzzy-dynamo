#![no_main]
use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use dynamo_parsers::*;
use dynamo_parsers::tool_calling::config::DsmlParserConfig;
use dynamo_parsers::tool_calling::json::{
    JsonParserConfig, JsonParserType, try_tool_call_parse_basic_json,
    parse_tool_calls_deepseek_v3,
};

#[derive(Debug, Arbitrary)]
struct FuzzInput {
    test_case: u8,
    split_pos: u8,
    text: String,
}

/// Round-trip semantic oracle — embeds known-valid tool calls into
/// fuzz-controlled surrounding text and verifies extraction correctness.
///
/// Bug-specific regression tests live in each parser's #[cfg(test)] module.
fuzz_target!(|input: FuzzInput| {
    let text = &input.text;
    if text.is_empty() { return; }

    let mut split = (input.split_pos as usize) % (text.len() + 1);
    while split < text.len() && !text.is_char_boundary(split) { split += 1; }
    let (pfx, sfx) = text.split_at(split);

    match input.test_case % 5 {
        // --- Round-trip: XML ---
        0 => {
            let input = format!(
                r#"{pfx}<tool_call>{{"name":"get_weather","arguments":{{"location":"NYC"}}}}</tool_call>{sfx}"#
            );
            let Ok((calls, _)) = try_tool_call_parse_xml(&input, &XmlParserConfig::default(), None) else { return };
            verify_roundtrip(&calls, "get_weather", "location", "NYC", "xml");
        }
        // --- Round-trip: Pythonic ---
        1 => {
            let input = format!(r#"{pfx}get_weather(location="NYC"){sfx}"#);
            let Ok((calls, _)) = try_tool_call_parse_pythonic(&input, None) else { return };
            verify_roundtrip(&calls, "get_weather", "location", "NYC", "pythonic");
        }
        // --- Round-trip: DSML ---
        2 => {
            let input = format!(
                "{pfx}<｜DSML｜function_calls><｜DSML｜invoke name=\"search\">\
                 <｜DSML｜parameter name=\"query\" string=\"true\">hello\
                 </｜DSML｜parameter></｜DSML｜invoke></｜DSML｜function_calls>{sfx}"
            );
            let Ok((calls, _)) = try_tool_call_parse_dsml(&input, &DsmlParserConfig::default()) else { return };
            verify_roundtrip(&calls, "search", "query", "hello", "dsml");
        }
        // --- Round-trip: Basic JSON ---
        3 => {
            let cfg = JsonParserConfig {
                tool_call_start_tokens: vec!["<tool>".into()],
                tool_call_end_tokens: vec!["</tool>".into()],
                parser_type: JsonParserType::Basic,
                ..Default::default()
            };
            let input = format!(
                r#"{pfx}<tool>{{"name":"search","arguments":{{"q":"test"}}}}</tool>{sfx}"#
            );
            let Ok((calls, _)) = try_tool_call_parse_basic_json(&input, &cfg, None) else { return };
            verify_roundtrip(&calls, "search", "q", "test", "json");
        }
        // --- Round-trip: DeepSeek V3 ---
        _ => {
            let cfg = JsonParserConfig {
                tool_call_start_tokens: vec!["<｜tool▁calls▁begin｜>".into()],
                tool_call_end_tokens: vec!["<｜tool▁calls▁end｜>".into()],
                tool_call_separator_tokens: vec!["<｜tool▁sep｜>".into()],
                parser_type: JsonParserType::DeepseekV3,
                ..Default::default()
            };
            let tc = "<｜tool▁calls▁begin｜><｜tool▁call▁begin｜>function\
                       <｜tool▁sep｜>calc\n```json\n{\"x\":\"42\"}\n```\
                       <｜tool▁call▁end｜><｜tool▁calls▁end｜>";
            let input = format!("{pfx}{tc}{sfx}");
            let Ok((calls, _)) = parse_tool_calls_deepseek_v3(&input, &cfg, None) else { return };
            verify_roundtrip(&calls, "calc", "x", "42", "deepseek");
        }
    }
});

fn verify_roundtrip(calls: &[ToolCallResponse], func: &str, key: &str, val: &str, parser: &str) {
    if calls.is_empty() { return; }
    assert_eq!(calls[0].function.name, func, "{parser}: func name wrong");
    let args: serde_json::Value = serde_json::from_str(&calls[0].function.arguments)
        .unwrap_or_else(|e| panic!("{parser}: invalid args '{}': {e}", calls[0].function.arguments));
    assert_eq!(args[key].as_str(), Some(val),
        "{parser}: arg[{key}]={:?}, expected {val:?}", args[key]);
}
