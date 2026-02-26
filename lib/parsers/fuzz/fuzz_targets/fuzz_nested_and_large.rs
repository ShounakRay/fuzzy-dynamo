#![no_main]
use libfuzzer_sys::fuzz_target;
use dynamo_parsers::*;
use dynamo_parsers::tool_calling::config::{DsmlParserConfig, Glm47ParserConfig};
use dynamo_parsers::tool_calling::xml::try_tool_call_parse_glm47;
use dynamo_parsers::tool_calling::json::{
    JsonParserConfig, JsonParserType,
    try_tool_call_parse_basic_json,
    parse_tool_calls_deepseek_v3,
};

// Stress test: construct deeply nested / repetitive structures from
// fuzz input and feed them to parsers. Catches:
// - Stack overflow from recursive parsing
// - Quadratic regex behavior on repetitive patterns
// - Excessive allocation from many matches
fuzz_target!(|data: &[u8]| {
    if data.len() < 3 { return; }

    let depth = (data[0] as usize % 64) + 1;
    let parser_id = data[1] % 6;
    let Ok(payload) = std::str::from_utf8(&data[2..]) else { return };

    match parser_id {
        // Deeply nested JSON tool calls
        0 => {
            let mut nested = String::with_capacity(depth * 40 + payload.len());
            for _ in 0..depth {
                nested.push_str(r#"{"name":"f","arguments":{"x":"#);
            }
            nested.push_str(payload);
            for _ in 0..depth {
                nested.push_str(r#""}}"#);
            }
            let _ = try_tool_call_parse_json(&nested, &JsonParserConfig::default(), None);
        }
        // Repeated XML tool call tags
        1 => {
            let mut repeated = String::with_capacity(depth * 80 + payload.len());
            for i in 0..depth {
                repeated.push_str(&format!(
                    r#"<tool_call>{{"name":"f{i}","arguments":{{}}}}</tool_call>"#
                ));
            }
            repeated.push_str(payload);
            let _ = try_tool_call_parse_xml(&repeated, &XmlParserConfig::default(), None);
        }
        // Repeated DSML blocks
        2 => {
            let mut dsml = String::with_capacity(depth * 100);
            dsml.push_str("<｜DSML｜function_calls>");
            for i in 0..depth {
                dsml.push_str(&format!(
                    "<｜DSML｜invoke name=\"f{i}\"><｜DSML｜parameter name=\"x\" string=\"true\">{payload}</｜DSML｜parameter></｜DSML｜invoke>"
                ));
            }
            dsml.push_str("</｜DSML｜function_calls>");
            let _ = try_tool_call_parse_dsml(&dsml, &DsmlParserConfig::default());
        }
        // DeepSeek V3 with many tool calls
        3 => {
            let mut ds = String::with_capacity(depth * 100);
            ds.push_str("<｜tool▁calls▁begin｜>");
            for i in 0..depth {
                ds.push_str(&format!(
                    "<｜tool▁call▁begin｜>function<｜tool▁sep｜>f{i}\n```json\n{{\"x\":\"{payload}\"}}\n```<｜tool▁call▁end｜>"
                ));
            }
            ds.push_str("<｜tool▁calls▁end｜>");
            let cfg = JsonParserConfig {
                tool_call_start_tokens: vec!["<｜tool▁calls▁begin｜>".to_string()],
                tool_call_end_tokens: vec!["<｜tool▁calls▁end｜>".to_string()],
                tool_call_separator_tokens: vec!["<｜tool▁sep｜>".to_string()],
                parser_type: JsonParserType::DeepseekV3,
                ..Default::default()
            };
            let _ = parse_tool_calls_deepseek_v3(&ds, &cfg, None);
        }
        // Pythonic with many function calls
        4 => {
            let mut py = String::with_capacity(depth * 40);
            for i in 0..depth {
                py.push_str(&format!("f{i}(x=\"{payload}\")\n"));
            }
            let _ = try_tool_call_parse_pythonic(&py, None);
        }
        // Kimi K2 with many tool calls
        _ => {
            let mut kimi = String::with_capacity(depth * 80);
            for i in 0..depth {
                kimi.push_str(&format!(
                    "<|tool_call|>\nfunctions.f{i}:0\n{{\"x\":\"{payload}\"}}\n<|tool_call|>\n"
                ));
            }
            let _ = try_tool_call_parse_kimi_k2(&kimi, &KimiK2ParserConfig::default(), None);
        }
    }
});
