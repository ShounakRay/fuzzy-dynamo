#![no_main]
use libfuzzer_sys::fuzz_target;
use dynamo_parsers::*;
use dynamo_parsers::tool_calling::config::{DsmlParserConfig, Glm47ParserConfig};
use dynamo_parsers::tool_calling::xml::try_tool_call_parse_glm47;
use dynamo_parsers::tool_calling::json::{
    JsonParserConfig, JsonParserType,
    try_tool_call_parse_basic_json,
    parse_tool_calls_deepseek_v3,
    parse_tool_calls_deepseek_v3_1,
};

// Find the largest valid char boundary <= pos in string s.
fn char_boundary(s: &str, pos: usize) -> usize {
    let pos = pos.min(s.len());
    // Walk backwards to find a char boundary
    let mut i = pos;
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

// Fuzz parsers with FUZZED configs — not just the default presets.
// The first few bytes select which parser and what token patterns to use.
// This catches bugs where specific combinations of start/end tokens
// cause unexpected parser behavior (e.g. overlapping tokens, empty
// tokens, tokens that are prefixes of each other).
fuzz_target!(|data: &[u8]| {
    if data.len() < 4 { return; }
    let Ok(s) = std::str::from_utf8(&data[4..]) else { return };

    match data[0] % 8 {
        // JSON parser with fuzzed tokens
        0 => {
            let split = char_boundary(s, data[1] as usize);
            let (start_tok, rest) = s.split_at(split);
            let cfg = JsonParserConfig {
                tool_call_start_tokens: vec![start_tok.to_string()],
                tool_call_end_tokens: vec!["</tool>".to_string()],
                parser_type: JsonParserType::Basic,
                ..Default::default()
            };
            let _ = try_tool_call_parse_basic_json(rest, &cfg, None);
        }
        // JSON with fuzzed end tokens
        1 => {
            let split = char_boundary(s, data[1] as usize);
            let (end_tok, rest) = s.split_at(split);
            let cfg = JsonParserConfig {
                tool_call_start_tokens: vec!["<tool>".to_string()],
                tool_call_end_tokens: vec![end_tok.to_string()],
                parser_type: JsonParserType::Basic,
                ..Default::default()
            };
            let _ = try_tool_call_parse_basic_json(rest, &cfg, None);
        }
        // XML parser with fuzzed token config
        2 => {
            let split = char_boundary(s, data[1] as usize);
            let (tok, rest) = s.split_at(split);
            let cfg = XmlParserConfig {
                tool_call_start_token: tok.to_string(),
                tool_call_end_token: "</tool>".to_string(),
                ..Default::default()
            };
            let _ = try_tool_call_parse_xml(rest, &cfg, None);
        }
        // DeepSeek V3 with fuzzed separator
        3 => {
            let split = char_boundary(s, data[1] as usize);
            let (sep, rest) = s.split_at(split);
            let cfg = JsonParserConfig {
                tool_call_start_tokens: vec!["<｜tool▁calls▁begin｜>".to_string()],
                tool_call_end_tokens: vec!["<｜tool▁calls▁end｜>".to_string()],
                tool_call_separator_tokens: vec![sep.to_string()],
                parser_type: JsonParserType::DeepseekV3,
                ..Default::default()
            };
            let _ = parse_tool_calls_deepseek_v3(rest, &cfg, None);
        }
        // DeepSeek V3.1 with fuzzed separator
        4 => {
            let split = char_boundary(s, data[1] as usize);
            let (sep, rest) = s.split_at(split);
            let cfg = JsonParserConfig {
                tool_call_start_tokens: vec!["<｜tool▁calls▁begin｜>".to_string()],
                tool_call_end_tokens: vec!["<｜tool▁calls▁end｜>".to_string()],
                tool_call_separator_tokens: vec![sep.to_string()],
                parser_type: JsonParserType::DeepseekV31,
                ..Default::default()
            };
            let _ = parse_tool_calls_deepseek_v3_1(rest, &cfg, None);
        }
        // DSML with fuzzed input
        5 => {
            let _ = try_tool_call_parse_dsml(s, &DsmlParserConfig::default());
        }
        // Kimi K2 with fuzzed input
        6 => {
            let _ = try_tool_call_parse_kimi_k2(s, &KimiK2ParserConfig::default(), None);
        }
        // GLM47 with fuzzed input
        _ => {
            let _ = try_tool_call_parse_glm47(s, &Glm47ParserConfig::default(), None);
        }
    }
});
