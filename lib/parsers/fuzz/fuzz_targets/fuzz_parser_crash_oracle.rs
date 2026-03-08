#![no_main]
use libfuzzer_sys::fuzz_target;
use dynamo_parsers::*;
use dynamo_parsers::reasoning::ReasoningParser;
use dynamo_parsers::tool_calling::config::{DsmlParserConfig, Glm47ParserConfig};
use dynamo_parsers::tool_calling::xml::{
    try_tool_call_parse_glm47,
    find_tool_call_end_position_glm47, find_tool_call_end_position_kimi_k2,
    find_tool_call_end_position_xml,
};
use dynamo_parsers::tool_calling::dsml::find_tool_call_end_position_dsml;
use dynamo_parsers::tool_calling::pythonic::find_tool_call_end_position_pythonic;
use dynamo_parsers::tool_calling::json::{
    JsonParserConfig, JsonParserType,
    detect_tool_call_start_basic_json, try_tool_call_parse_basic_json,
    detect_tool_call_start_deepseek_v3, parse_tool_calls_deepseek_v3,
    detect_tool_call_start_deepseek_v3_1, parse_tool_calls_deepseek_v3_1,
};
use dynamo_parsers_fuzz::{REASONING_PARSER_TYPES, select_parser_type};

/// Consolidated crash oracle for all tool call parsers, reasoning parsers,
/// detect_tool_call_start, find_tool_call_end_position, and DeepSeek variants.
///
/// Merges: fuzz_tool_call_parsers, fuzz_parser_configs, fuzz_detect_start,
///         fuzz_end_positions, fuzz_redos, fuzz_reasoning_parsers,
///         fuzz_streaming_reasoning, fuzz_deepseek_parsers
fuzz_target!(|data: &[u8]| {
    let Ok(s) = std::str::from_utf8(data) else { return };

    // --- Tool call parsers (default configs) ---
    let _ = try_tool_call_parse_json(s, &JsonParserConfig::default(), None);
    let _ = try_tool_call_parse_xml(s, &XmlParserConfig::default(), None);
    let _ = try_tool_call_parse_pythonic(s, None);
    let _ = try_tool_call_parse_dsml(s, &DsmlParserConfig::default());
    let _ = try_tool_call_parse_kimi_k2(s, &KimiK2ParserConfig::default(), None);
    let _ = try_tool_call_parse_glm47(s, &Glm47ParserConfig::default(), None);

    // --- All named configs ---
    let configs: &[ToolCallConfig] = &[
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
    for config in configs {
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

    // --- detect_tool_call_start for all parser names ---
    for name in [
        "hermes", "nemotron_deci", "llama3_json", "mistral", "phi4",
        "pythonic", "harmony", "deepseek_v3", "deepseek_v3_1", "deepseek_v3_2",
        "qwen3_coder", "jamba", "minimax_m2", "glm47", "kimi_k2", "default",
    ] {
        let _ = detect_tool_call_start(s, Some(name));
    }
    let _ = detect_tool_call_start(s, None);

    // --- find_tool_call_end_position with bounds assertions ---
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

    // --- DeepSeek-specific parsers ---
    let basic_cfg = JsonParserConfig::default();
    let _ = detect_tool_call_start_basic_json(s, &basic_cfg);
    let _ = try_tool_call_parse_basic_json(s, &basic_cfg, None);

    let v3_cfg = JsonParserConfig {
        parser_type: JsonParserType::DeepseekV3,
        tool_call_start_tokens: vec!["<｜tool▁calls▁begin｜>".into(), "<｜tool▁call▁begin｜>".into()],
        tool_call_end_tokens: vec!["<｜tool▁calls▁end｜>".into(), "<｜tool▁call▁end｜>".into()],
        tool_call_separator_tokens: vec!["<｜tool▁sep｜>".into()],
        ..Default::default()
    };
    let _ = detect_tool_call_start_deepseek_v3(s, &v3_cfg);
    let _ = parse_tool_calls_deepseek_v3(s, &v3_cfg, None);

    let v31_cfg = JsonParserConfig { parser_type: JsonParserType::DeepseekV31, ..v3_cfg.clone() };
    let _ = detect_tool_call_start_deepseek_v3_1(s, &v31_cfg);
    let _ = parse_tool_calls_deepseek_v3_1(s, &v31_cfg, None);

    // --- Reasoning parsers (one-shot) ---
    for &t in REASONING_PARSER_TYPES {
        let mut parser = t.get_reasoning_parser();
        let _ = parser.detect_and_parse_reasoning(s, &[]);
    }

    // --- Reasoning parsers (streaming, fuzz-controlled chunking) ---
    if data.len() >= 2 {
        let mut parser = select_parser_type(data[0]).get_reasoning_parser();
        let bytes = s.as_bytes();
        let mut pos = 0;
        let mut seed = 1usize;
        while pos < bytes.len() {
            seed = seed.wrapping_mul(31).wrapping_add(7);
            let chunk_len = (seed % 16).max(1).min(bytes.len() - pos);
            let end = match std::str::from_utf8(&bytes[pos..(pos + chunk_len).min(bytes.len())]) {
                Ok(_) => (pos + chunk_len).min(bytes.len()),
                Err(e) => pos + e.valid_up_to(),
            };
            if end == pos { pos += 1; continue; }
            let chunk = unsafe { std::str::from_utf8_unchecked(&bytes[pos..end]) };
            let _ = parser.parse_reasoning_streaming_incremental(chunk, &[]);
            pos = end;
        }
    }
});
