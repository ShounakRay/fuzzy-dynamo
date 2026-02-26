#![no_main]
use libfuzzer_sys::fuzz_target;
use dynamo_parsers::*;
use dynamo_parsers::tool_calling::config::{DsmlParserConfig, Glm47ParserConfig};
use dynamo_parsers::tool_calling::xml::try_tool_call_parse_glm47;

// Fuzz all sync tool call parsers with arbitrary string input.
// Each parser gets default config and no tool definitions.
fuzz_target!(|data: &[u8]| {
    let Ok(s) = std::str::from_utf8(data) else { return };

    let _ = try_tool_call_parse_json(s, &JsonParserConfig::default(), None);
    let _ = try_tool_call_parse_xml(s, &XmlParserConfig::default(), None);
    let _ = try_tool_call_parse_pythonic(s, None);
    let _ = try_tool_call_parse_dsml(s, &DsmlParserConfig::default());
    let _ = try_tool_call_parse_kimi_k2(s, &KimiK2ParserConfig::default(), None);
    let _ = try_tool_call_parse_glm47(s, &Glm47ParserConfig::default(), None);
});
