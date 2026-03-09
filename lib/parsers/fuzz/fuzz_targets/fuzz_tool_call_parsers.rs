#![no_main]
use libfuzzer_sys::fuzz_target;
use dynamo_parsers::*;
use dynamo_parsers::tool_calling::config::{DsmlParserConfig, Glm47ParserConfig};
use dynamo_parsers::tool_calling::xml::{try_tool_call_parse_glm47, try_tool_call_parse_kimi_k2};

/// Crash oracle + determinism checks for all tool call parsers.
fuzz_target!(|data: &[u8]| {
    let Ok(s) = std::str::from_utf8(data) else { return };

    // JSON parser: determinism check
    let json_cfg = JsonParserConfig::default();
    let r1 = try_tool_call_parse_json(s, &json_cfg, None);
    let r2 = try_tool_call_parse_json(s, &json_cfg, None);
    if let (Ok((calls1, text1)), Ok((calls2, text2))) = (&r1, &r2) {
        assert_eq!(calls1.len(), calls2.len(), "JSON parser: non-deterministic call count");
        assert_eq!(text1, text2, "JSON parser: non-deterministic normal text");
    }

    // XML parser (skip single-quote function names — known bug #15)
    let xml_cfg = XmlParserConfig::default();
    let r1 = try_tool_call_parse_xml(s, &xml_cfg, None);
    let r2 = try_tool_call_parse_xml(s, &xml_cfg, None);
    if let (Ok((calls1, text1)), Ok((calls2, text2))) = (&r1, &r2) {
        assert_eq!(calls1.len(), calls2.len(), "XML parser: non-deterministic call count");
        assert_eq!(text1, text2, "XML parser: non-deterministic normal text");
    }

    let _ = try_tool_call_parse_pythonic(s, None);
    let _ = try_tool_call_parse_dsml(s, &DsmlParserConfig::default());
    let _ = try_tool_call_parse_kimi_k2(s, &KimiK2ParserConfig::default(), None);
    let _ = try_tool_call_parse_glm47(s, &Glm47ParserConfig::default(), None);
});
