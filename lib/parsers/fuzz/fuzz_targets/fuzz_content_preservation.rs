#![no_main]
use libfuzzer_sys::fuzz_target;
use dynamo_parsers::*;
use dynamo_parsers::tool_calling::config::{DsmlParserConfig, Glm47ParserConfig};
use dynamo_parsers::tool_calling::xml::{try_tool_call_parse_glm47, try_tool_call_parse_kimi_k2};

fuzz_target!(|data: &[u8]| {
    let Ok(s) = std::str::from_utf8(data) else { return };

    check(try_tool_call_parse_xml(s, &XmlParserConfig::default(), None), s, "xml");
    check(try_tool_call_parse_json(s, &JsonParserConfig::default(), None), s, "json");
    check(try_tool_call_parse_pythonic(s, None), s, "pythonic");
    check(try_tool_call_parse_dsml(s, &DsmlParserConfig::default()), s, "dsml");
    check(try_tool_call_parse_kimi_k2(s, &KimiK2ParserConfig::default(), None), s, "kimi_k2");
    check(try_tool_call_parse_glm47(s, &Glm47ParserConfig::default(), None), s, "glm47");
});

fn check<E: std::fmt::Debug>(
    result: Result<(Vec<ToolCallResponse>, Option<String>), E>,
    input: &str,
    parser: &str,
) {
    let Ok((calls, normal_text)) = result else { return };

    if calls.is_empty() {
        if let Some(ref text) = normal_text {
            assert!(text.len() <= input.len(), "{parser}: normal_text exceeds input");
        }
        return;
    }

    let normal_len = normal_text.as_ref().map_or(0, |t| t.len());
    let tool_len: usize = calls.iter().map(|c| c.function.name.len() + c.function.arguments.len()).sum();
    assert!(normal_len + tool_len <= input.len() * 3, "{parser}: extracted content far exceeds input");

    if let Some(ref text) = normal_text {
        for ch in text.chars() {
            assert!(input.contains(ch), "{parser}: normal_text contains char {ch:?} not in input");
        }
    }
    for call in &calls {
        assert!(!call.function.name.is_empty(), "{parser}: empty function name");
    }
}
