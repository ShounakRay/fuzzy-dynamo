#![no_main]
use libfuzzer_sys::fuzz_target;
use dynamo_parsers::*;
use dynamo_parsers::tool_calling::config::DsmlParserConfig;
use dynamo_parsers::tool_calling::json::{
    JsonParserConfig, JsonParserType, parse_tool_calls_deepseek_v3,
    try_tool_call_parse_basic_json,
};

fuzz_target!(|data: &[u8]| {
    if data.len() < 4 { return; }
    let Ok(text) = std::str::from_utf8(&data[2..]) else { return };

    let mut split = (data[1] as usize) % (text.len() + 1);
    while split < text.len() && !text.is_char_boundary(split) { split += 1; }
    let (pfx, sfx) = text.split_at(split);

    match data[0] % 5 {
        0 => {
            let input = format!(
                r#"{pfx}<tool_call>{{"name":"get_weather","arguments":{{"location":"NYC"}}}}</tool_call>{sfx}"#
            );
            let Ok((calls, _)) = try_tool_call_parse_xml(&input, &XmlParserConfig::default(), None) else { return };
            verify(&calls, "get_weather", "location", "NYC", "xml");
        }
        1 => {
            let input = format!(r#"{pfx}get_weather(location="NYC"){sfx}"#);
            let Ok((calls, _)) = try_tool_call_parse_pythonic(&input, None) else { return };
            verify(&calls, "get_weather", "location", "NYC", "pythonic");
        }
        2 => {
            let input = format!(
                "{pfx}<｜DSML｜function_calls><｜DSML｜invoke name=\"search\">\
                 <｜DSML｜parameter name=\"query\" string=\"true\">hello\
                 </｜DSML｜parameter></｜DSML｜invoke></｜DSML｜function_calls>{sfx}"
            );
            let Ok((calls, _)) = try_tool_call_parse_dsml(&input, &DsmlParserConfig::default()) else { return };
            verify(&calls, "search", "query", "hello", "dsml");
        }
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
            verify(&calls, "search", "q", "test", "json");
        }
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
            verify(&calls, "calc", "x", "42", "deepseek");
        }
    }
});

fn verify(calls: &[ToolCallResponse], func: &str, key: &str, val: &str, parser: &str) {
    if calls.is_empty() { return; }
    assert_eq!(calls[0].function.name, func, "{parser}: func name wrong");
    let args: serde_json::Value = serde_json::from_str(&calls[0].function.arguments)
        .unwrap_or_else(|e| panic!("{parser}: invalid args '{}': {e}", calls[0].function.arguments));
    assert_eq!(args[key].as_str(), Some(val),
        "{parser}: arg[{key}]={:?}, expected {val:?}", args[key]);
}
