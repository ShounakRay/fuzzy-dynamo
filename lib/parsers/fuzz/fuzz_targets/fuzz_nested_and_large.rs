#![no_main]
use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use dynamo_parsers::*;
use dynamo_parsers::tool_calling::config::DsmlParserConfig;
use dynamo_parsers::tool_calling::json::{JsonParserConfig, JsonParserType, parse_tool_calls_deepseek_v3};

#[derive(Debug, Arbitrary)]
struct FuzzInput {
    depth: u8,
    variant: u8,
    payload: String,
}

fuzz_target!(|input: FuzzInput| {
    let depth = (input.depth as usize % 64) + 1;
    let payload = &input.payload;
    if payload.is_empty() { return; }

    match input.variant % 6 {
        0 => {
            let mut s = String::with_capacity(depth * 40 + payload.len());
            for _ in 0..depth { s.push_str(r#"{"name":"f","arguments":{"x":"#); }
            s.push_str(payload);
            for _ in 0..depth { s.push_str(r#""}}"#); }
            let _ = try_tool_call_parse_json(&s, &JsonParserConfig::default(), None);
        }
        1 => {
            let mut s = String::with_capacity(depth * 80 + payload.len());
            for i in 0..depth {
                s.push_str(&format!(r#"<tool_call>{{"name":"f{i}","arguments":{{}}}}</tool_call>"#));
            }
            s.push_str(payload);
            let _ = try_tool_call_parse_xml(&s, &XmlParserConfig::default(), None);
        }
        2 => {
            let mut s = String::with_capacity(depth * 100);
            s.push_str("<｜DSML｜function_calls>");
            for i in 0..depth {
                s.push_str(&format!(
                    "<｜DSML｜invoke name=\"f{i}\"><｜DSML｜parameter name=\"x\" string=\"true\">{payload}</｜DSML｜parameter></｜DSML｜invoke>"
                ));
            }
            s.push_str("</｜DSML｜function_calls>");
            let _ = try_tool_call_parse_dsml(&s, &DsmlParserConfig::default());
        }
        3 => {
            let mut s = String::with_capacity(depth * 100);
            s.push_str("<｜tool▁calls▁begin｜>");
            for i in 0..depth {
                s.push_str(&format!(
                    "<｜tool▁call▁begin｜>function<｜tool▁sep｜>f{i}\n```json\n{{\"x\":\"{payload}\"}}\n```<｜tool▁call▁end｜>"
                ));
            }
            s.push_str("<｜tool▁calls▁end｜>");
            let cfg = JsonParserConfig {
                tool_call_start_tokens: vec!["<｜tool▁calls▁begin｜>".into()],
                tool_call_end_tokens: vec!["<｜tool▁calls▁end｜>".into()],
                tool_call_separator_tokens: vec!["<｜tool▁sep｜>".into()],
                parser_type: JsonParserType::DeepseekV3,
                ..Default::default()
            };
            let _ = parse_tool_calls_deepseek_v3(&s, &cfg, None);
        }
        4 => {
            let mut s = String::with_capacity(depth * 40);
            for i in 0..depth { s.push_str(&format!("f{i}(x=\"{payload}\")\n")); }
            let _ = try_tool_call_parse_pythonic(&s, None);
        }
        _ => {
            let mut s = String::with_capacity(depth * 80);
            for i in 0..depth {
                s.push_str(&format!("<|tool_call|>\nfunctions.f{i}:0\n{{\"x\":\"{payload}\"}}\n<|tool_call|>\n"));
            }
            let _ = try_tool_call_parse_kimi_k2(&s, &KimiK2ParserConfig::default(), None);
        }
    }
});
