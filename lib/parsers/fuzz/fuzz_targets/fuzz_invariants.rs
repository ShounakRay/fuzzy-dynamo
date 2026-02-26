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

// Invariant-checking fuzz harness.
// Instead of just checking "doesn't crash", this verifies parser postconditions:
// 1. All find_tool_call_end_position variants return pos <= input.len()
// 2. Parsed tool calls have non-empty function names
// 3. Parsed tool call arguments are valid JSON (or at least valid strings)
// 4. Normal text returned doesn't exceed input length
fuzz_target!(|data: &[u8]| {
    let Ok(s) = std::str::from_utf8(data) else { return };

    // === Invariant 1: End positions never exceed input length ===
    // Test each parser-specific end position function directly.
    let xml_cfg = XmlParserConfig::default();
    let glm47_cfg = Glm47ParserConfig::default();
    let kimi_cfg = KimiK2ParserConfig::default();
    let dsml_cfg = DsmlParserConfig::default();

    let pos = find_tool_call_end_position_xml(s, &xml_cfg);
    assert!(pos <= s.len(), "xml end_position {pos} > len {}", s.len());

    let pos = find_tool_call_end_position_glm47(s, &glm47_cfg);
    assert!(pos <= s.len(), "glm47 end_position {pos} > len {}", s.len());

    let pos = find_tool_call_end_position_kimi_k2(s, &kimi_cfg);
    assert!(pos <= s.len(), "kimi_k2 end_position {pos} > len {}", s.len());

    let pos = find_tool_call_end_position_dsml(s, &dsml_cfg);
    assert!(pos <= s.len(), "dsml end_position {pos} > len {}", s.len());

    let pos = find_tool_call_end_position_pythonic(s);
    assert!(pos <= s.len(), "pythonic end_position {pos} > len {}", s.len());

    // === Invariant 2 & 3: Tool calls have valid names and arguments ===
    // Test each parser and validate its output.
    check_tool_call_invariants(try_tool_call_parse_xml(s, &xml_cfg, None), s, "xml");
    check_tool_call_invariants(try_tool_call_parse_pythonic(s, None), s, "pythonic");
    check_tool_call_invariants(try_tool_call_parse_dsml(s, &dsml_cfg), s, "dsml");
    check_tool_call_invariants(
        try_tool_call_parse_kimi_k2(s, &kimi_cfg, None),
        s,
        "kimi_k2",
    );
    check_tool_call_invariants(
        try_tool_call_parse_glm47(s, &glm47_cfg, None),
        s,
        "glm47",
    );
});

fn check_tool_call_invariants<E>(
    result: Result<(Vec<ToolCallResponse>, Option<String>), E>,
    input: &str,
    parser_name: &str,
) {
    let Ok((calls, normal_text)) = result else {
        return; // Errors are fine, we're checking postconditions of successful parses
    };

    for (i, call) in calls.iter().enumerate() {
        // Function name must be non-empty
        assert!(
            !call.function.name.is_empty(),
            "{parser_name}: tool call {i} has empty function name"
        );

        // Arguments must be valid JSON (parsers promise JSON string output)
        let args = &call.function.arguments;
        assert!(
            serde_json::from_str::<serde_json::Value>(args).is_ok(),
            "{parser_name}: tool call {i} ('{}') has invalid JSON arguments: {args}",
            call.function.name,
        );
    }

    // Normal text (if any) should not be longer than the input
    // (parsers shouldn't fabricate content)
    if let Some(ref text) = normal_text {
        assert!(
            text.len() <= input.len(),
            "{parser_name}: normal_text length {} exceeds input length {}",
            text.len(),
            input.len(),
        );
    }
}
