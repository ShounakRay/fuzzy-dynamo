#![no_main]
use libfuzzer_sys::fuzz_target;
use dynamo_parsers::*;
use dynamo_parsers::tool_calling::config::{DsmlParserConfig, Glm47ParserConfig};
use dynamo_parsers::tool_calling::xml::{
    find_tool_call_end_position_glm47, find_tool_call_end_position_kimi_k2,
    find_tool_call_end_position_xml, try_tool_call_parse_glm47,
};
use dynamo_parsers::tool_calling::dsml::find_tool_call_end_position_dsml;
use dynamo_parsers::tool_calling::pythonic::find_tool_call_end_position_pythonic;

fuzz_target!(|data: &[u8]| {
    let Ok(s) = std::str::from_utf8(data) else { return };

    let xml_cfg = XmlParserConfig::default();
    let glm47_cfg = Glm47ParserConfig::default();
    let kimi_cfg = KimiK2ParserConfig::default();
    let dsml_cfg = DsmlParserConfig::default();

    for (pos, name) in [
        (find_tool_call_end_position_xml(s, &xml_cfg), "xml"),
        (find_tool_call_end_position_glm47(s, &glm47_cfg), "glm47"),
        (find_tool_call_end_position_kimi_k2(s, &kimi_cfg), "kimi_k2"),
        (find_tool_call_end_position_dsml(s, &dsml_cfg), "dsml"),
        (find_tool_call_end_position_pythonic(s), "pythonic"),
    ] {
        assert!(pos <= s.len(), "{name} end_position {pos} > len {}", s.len());
    }

    check(try_tool_call_parse_xml(s, &xml_cfg, None), s, "xml");
    check(try_tool_call_parse_pythonic(s, None), s, "pythonic");
    check(try_tool_call_parse_dsml(s, &dsml_cfg), s, "dsml");
    check(try_tool_call_parse_kimi_k2(s, &kimi_cfg, None), s, "kimi_k2");
    check(try_tool_call_parse_glm47(s, &glm47_cfg, None), s, "glm47");
});

fn check<E>(result: Result<(Vec<ToolCallResponse>, Option<String>), E>, input: &str, parser: &str) {
    let Ok((calls, normal_text)) = result else { return };
    for (i, call) in calls.iter().enumerate() {
        assert!(!call.function.name.is_empty(), "{parser}: call {i} empty name");
        assert!(
            serde_json::from_str::<serde_json::Value>(&call.function.arguments).is_ok(),
            "{parser}: call {i} ('{}') invalid JSON args: {}", call.function.name, call.function.arguments,
        );
    }
    if let Some(ref text) = normal_text {
        assert!(text.len() <= input.len(), "{parser}: normal_text exceeds input");
    }
}
