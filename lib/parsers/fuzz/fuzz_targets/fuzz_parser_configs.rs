#![no_main]
use libfuzzer_sys::fuzz_target;
use dynamo_parsers::*;
use dynamo_parsers::tool_calling::config::{DsmlParserConfig, Glm47ParserConfig};
use dynamo_parsers::tool_calling::xml::try_tool_call_parse_glm47;

// Fuzz parsers with every named config preset.
// The default-config harness might miss bugs that only trigger with
// specific start/end token patterns (e.g. deepseek's unicode tokens).
fuzz_target!(|data: &[u8]| {
    let Ok(s) = std::str::from_utf8(data) else { return };

    let configs: Vec<ToolCallConfig> = vec![
        ToolCallConfig::hermes(),
        ToolCallConfig::nemotron_deci(),
        ToolCallConfig::llama3_json(),
        ToolCallConfig::mistral(),
        ToolCallConfig::phi4(),
        ToolCallConfig::pythonic(),
        ToolCallConfig::deepseek_v3(),
        ToolCallConfig::deepseek_v3_1(),
        ToolCallConfig::deepseek_v3_2(),
        ToolCallConfig::qwen3_coder(),
        ToolCallConfig::jamba(),
        ToolCallConfig::minimax_m2(),
        ToolCallConfig::glm47(),
        ToolCallConfig::kimi_k2(),
    ];

    for config in &configs {
        match &config.parser_config {
            ParserConfig::Json(c) => { let _ = try_tool_call_parse_json(s, c, None); }
            ParserConfig::Xml(c) => { let _ = try_tool_call_parse_xml(s, c, None); }
            ParserConfig::Pythonic => { let _ = try_tool_call_parse_pythonic(s, None); }
            ParserConfig::Dsml(c) => { let _ = try_tool_call_parse_dsml(s, c); }
            ParserConfig::Glm47(c) => { let _ = try_tool_call_parse_glm47(s, c, None); }
            ParserConfig::KimiK2(c) => { let _ = try_tool_call_parse_kimi_k2(s, c, None); }
            _ => {}
        }
    }
});
