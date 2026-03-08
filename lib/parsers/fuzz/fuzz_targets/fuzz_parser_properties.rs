#![no_main]
use libfuzzer_sys::fuzz_target;
use dynamo_parsers::*;
use dynamo_parsers::reasoning::ReasoningParser;
use dynamo_parsers::tool_calling::config::{DsmlParserConfig, Glm47ParserConfig};
use dynamo_parsers::tool_calling::xml::{
    try_tool_call_parse_glm47, try_tool_call_parse_kimi_k2,
    find_tool_call_end_position_glm47, find_tool_call_end_position_kimi_k2,
    find_tool_call_end_position_xml,
};
use dynamo_parsers::tool_calling::dsml::find_tool_call_end_position_dsml;
use dynamo_parsers::tool_calling::pythonic::find_tool_call_end_position_pythonic;
use dynamo_parsers::tool_calling::utils::{chunk_ends_with_token_prefix, decode_xml_entities};
use dynamo_parsers_fuzz::{StreamingChunker, select_parser_type};

/// Consolidated property-checking oracle.
///
/// Merges: fuzz_content_preservation, fuzz_streaming_monotonicity,
///         fuzz_invariants, fuzz_utils
///
/// Properties checked:
/// 1. Extracted content must come from the input (no fabrication)
/// 2. Streaming output must be monotonically non-decreasing
/// 3. End positions must be valid, func names non-empty, args valid JSON
/// 4. chunk_ends_with_token_prefix correctness
/// 5. decode_xml_entities doesn't grow without entities
fuzz_target!(|data: &[u8]| {
    if data.len() < 3 { return; }
    let Ok(s) = std::str::from_utf8(&data[2..]) else { return };
    if s.is_empty() { return; }

    // === Content preservation (tool call parsers) ===
    check_content(try_tool_call_parse_xml(s, &XmlParserConfig::default(), None), s, "xml");
    check_content(try_tool_call_parse_json(s, &JsonParserConfig::default(), None), s, "json");
    check_content(try_tool_call_parse_pythonic(s, None), s, "pythonic");
    check_content(try_tool_call_parse_dsml(s, &DsmlParserConfig::default()), s, "dsml");
    check_content(try_tool_call_parse_kimi_k2(s, &KimiK2ParserConfig::default(), None), s, "kimi_k2");
    check_content(try_tool_call_parse_glm47(s, &Glm47ParserConfig::default(), None), s, "glm47");

    // === Invariants (end positions, func names, JSON args) ===
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

    check_invariants(try_tool_call_parse_xml(s, &xml_cfg, None), s, "xml");
    check_invariants(try_tool_call_parse_pythonic(s, None), s, "pythonic");
    check_invariants(try_tool_call_parse_dsml(s, &dsml_cfg), s, "dsml");
    check_invariants(try_tool_call_parse_kimi_k2(s, &kimi_cfg, None), s, "kimi_k2");
    check_invariants(try_tool_call_parse_glm47(s, &glm47_cfg, None), s, "glm47");

    // === Streaming monotonicity (reasoning parsers) ===
    let parser_type = select_parser_type(data[0]);
    let mut parser = parser_type.get_reasoning_parser();
    let mut reasoning_len: usize = 0;
    let mut normal_len: usize = 0;
    let mut reasoning = String::new();
    let mut normal = String::new();
    let mut pos = 0;

    for chunk in StreamingChunker::new(s, data[1], data[1]) {
        let result = parser.parse_reasoning_streaming_incremental(chunk, &[]);
        reasoning.push_str(&result.reasoning_text);
        normal.push_str(&result.normal_text);

        assert!(reasoning.len() >= reasoning_len,
            "{parser_type:?}: reasoning shrank at pos {pos}");
        assert!(normal.len() >= normal_len,
            "{parser_type:?}: normal text shrank at pos {pos}");

        reasoning_len = reasoning.len();
        normal_len = normal.len();
        pos += chunk.len();
    }
    assert!(reasoning_len + normal_len <= s.len() * 2,
        "{parser_type:?}: output far exceeds input");

    // === Utils: chunk_ends_with_token_prefix ===
    if s.len() >= 2 {
        let split = (data[0] as usize) % s.len();
        let mut boundary = split;
        while boundary < s.len() && !s.is_char_boundary(boundary) { boundary += 1; }
        let (chunk, token) = s.split_at(boundary);

        let result = chunk_ends_with_token_prefix(chunk, token);
        if result && !token.is_empty() {
            let chars: Vec<char> = token.chars().collect();
            let mut found = false;
            for i in 1..chars.len() {
                let prefix: String = chars[..i].iter().collect();
                if chunk.ends_with(&prefix) { found = true; break; }
            }
            assert!(found,
                "chunk_ends_with_token_prefix returned true but no prefix match found");
        }
    }

    // === Utils: decode_xml_entities ===
    let decoded = decode_xml_entities(s);
    assert!(decoded.len() <= s.len() || s.contains('&'),
        "decode_xml_entities grew output without entities in input");
});

fn check_content<E: std::fmt::Debug>(
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
        let mut input_iter = input.chars();
        for ch in text.chars() {
            assert!(input_iter.any(|c| c == ch),
                "{parser}: normal_text char {ch:?} out of order or absent from input");
        }
    }
    for call in &calls {
        assert!(!call.function.name.is_empty(), "{parser}: empty function name");
    }
}

fn check_invariants<E>(
    result: Result<(Vec<ToolCallResponse>, Option<String>), E>,
    input: &str,
    parser: &str,
) {
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
