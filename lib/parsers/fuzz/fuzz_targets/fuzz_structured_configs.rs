#![no_main]
use libfuzzer_sys::fuzz_target;
use dynamo_parsers::*;
use dynamo_parsers::tool_calling::config::{DsmlParserConfig, Glm47ParserConfig};
use dynamo_parsers::tool_calling::xml::try_tool_call_parse_glm47;
use dynamo_parsers::tool_calling::json::{
    JsonParserConfig, JsonParserType, try_tool_call_parse_basic_json,
    parse_tool_calls_deepseek_v3, parse_tool_calls_deepseek_v3_1,
};

fn char_boundary(s: &str, pos: usize) -> usize {
    let mut i = pos.min(s.len());
    while i > 0 && !s.is_char_boundary(i) { i -= 1; }
    i
}

fuzz_target!(|data: &[u8]| {
    if data.len() < 4 { return; }
    let Ok(s) = std::str::from_utf8(&data[4..]) else { return };

    match data[0] % 8 {
        0 => {
            let split = char_boundary(s, data[1] as usize);
            let (start_tok, rest) = s.split_at(split);
            let cfg = JsonParserConfig {
                tool_call_start_tokens: vec![start_tok.into()],
                tool_call_end_tokens: vec!["</tool>".into()],
                parser_type: JsonParserType::Basic,
                ..Default::default()
            };
            let _ = try_tool_call_parse_basic_json(rest, &cfg, None);
        }
        1 => {
            let split = char_boundary(s, data[1] as usize);
            let (end_tok, rest) = s.split_at(split);
            let cfg = JsonParserConfig {
                tool_call_start_tokens: vec!["<tool>".into()],
                tool_call_end_tokens: vec![end_tok.into()],
                parser_type: JsonParserType::Basic,
                ..Default::default()
            };
            let _ = try_tool_call_parse_basic_json(rest, &cfg, None);
        }
        2 => {
            let split = char_boundary(s, data[1] as usize);
            let (tok, rest) = s.split_at(split);
            let cfg = XmlParserConfig {
                tool_call_start_token: tok.into(),
                tool_call_end_token: "</tool>".into(),
                ..Default::default()
            };
            let _ = try_tool_call_parse_xml(rest, &cfg, None);
        }
        3 => {
            let split = char_boundary(s, data[1] as usize);
            let (sep, rest) = s.split_at(split);
            let cfg = JsonParserConfig {
                tool_call_start_tokens: vec!["<｜tool▁calls▁begin｜>".into()],
                tool_call_end_tokens: vec!["<｜tool▁calls▁end｜>".into()],
                tool_call_separator_tokens: vec![sep.into()],
                parser_type: JsonParserType::DeepseekV3,
                ..Default::default()
            };
            let _ = parse_tool_calls_deepseek_v3(rest, &cfg, None);
        }
        4 => {
            let split = char_boundary(s, data[1] as usize);
            let (sep, rest) = s.split_at(split);
            let cfg = JsonParserConfig {
                tool_call_start_tokens: vec!["<｜tool▁calls▁begin｜>".into()],
                tool_call_end_tokens: vec!["<｜tool▁calls▁end｜>".into()],
                tool_call_separator_tokens: vec![sep.into()],
                parser_type: JsonParserType::DeepseekV31,
                ..Default::default()
            };
            let _ = parse_tool_calls_deepseek_v3_1(rest, &cfg, None);
        }
        5 => { let _ = try_tool_call_parse_dsml(s, &DsmlParserConfig::default()); }
        6 => { let _ = try_tool_call_parse_kimi_k2(s, &KimiK2ParserConfig::default(), None); }
        _ => { let _ = try_tool_call_parse_glm47(s, &Glm47ParserConfig::default(), None); }
    }
});
