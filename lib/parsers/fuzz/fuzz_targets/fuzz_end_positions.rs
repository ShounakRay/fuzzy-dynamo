#![no_main]
use libfuzzer_sys::fuzz_target;
use dynamo_parsers::*;
use dynamo_parsers::tool_calling::config::{DsmlParserConfig, Glm47ParserConfig};
use dynamo_parsers::tool_calling::xml::{
    find_tool_call_end_position_glm47, find_tool_call_end_position_kimi_k2,
    find_tool_call_end_position_xml,
};
use dynamo_parsers::tool_calling::dsml::find_tool_call_end_position_dsml;
use dynamo_parsers::tool_calling::pythonic::find_tool_call_end_position_pythonic;

/// Focused crash oracle for find_end_position calls with bounds checks.
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

    for name in [
        "hermes", "nemotron_deci", "llama3_json", "mistral", "phi4",
        "pythonic", "harmony", "deepseek_v3", "deepseek_v3_1", "deepseek_v3_2",
        "qwen3_coder", "jamba", "minimax_m2", "glm47", "kimi_k2", "default",
    ] {
        let pos = find_tool_call_end_position(s, Some(name));
        assert!(pos <= s.len(), "end_position({name}) = {pos} > len {}", s.len());
    }
});
