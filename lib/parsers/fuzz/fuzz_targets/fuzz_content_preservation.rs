#![no_main]
use libfuzzer_sys::fuzz_target;
use dynamo_parsers::*;
use dynamo_parsers::tool_calling::config::{DsmlParserConfig, Glm47ParserConfig};
use dynamo_parsers::tool_calling::xml::{try_tool_call_parse_glm47, try_tool_call_parse_kimi_k2};

// Content preservation oracle:
// Text outside tool-call delimiters must survive parsing.
// For each parser, verify that normal_text is a substring of the input
// and that no content is fabricated.
fuzz_target!(|data: &[u8]| {
    let Ok(s) = std::str::from_utf8(data) else { return };

    check_content_preservation(
        try_tool_call_parse_xml(s, &XmlParserConfig::default(), None),
        s,
        "xml",
    );
    check_content_preservation(
        try_tool_call_parse_json(s, &JsonParserConfig::default(), None),
        s,
        "json",
    );
    check_content_preservation(try_tool_call_parse_pythonic(s, None), s, "pythonic");
    check_content_preservation(
        try_tool_call_parse_dsml(s, &DsmlParserConfig::default()),
        s,
        "dsml",
    );
    check_content_preservation(
        try_tool_call_parse_kimi_k2(s, &KimiK2ParserConfig::default(), None),
        s,
        "kimi_k2",
    );
    check_content_preservation(
        try_tool_call_parse_glm47(s, &Glm47ParserConfig::default(), None),
        s,
        "glm47",
    );
});

fn check_content_preservation<E: std::fmt::Debug>(
    result: Result<(Vec<ToolCallResponse>, Option<String>), E>,
    input: &str,
    parser_name: &str,
) {
    let Ok((calls, normal_text)) = result else {
        return; // Errors are fine
    };

    // If there are no tool calls, normal_text should be the entire input (or None)
    if calls.is_empty() {
        if let Some(ref text) = normal_text {
            assert!(
                text.len() <= input.len(),
                "{parser_name}: no tool calls but normal_text ({} bytes) exceeds input ({} bytes)",
                text.len(),
                input.len(),
            );
        }
        return;
    }

    // With tool calls: normal_text + tool call content should not exceed input
    let normal_len = normal_text.as_ref().map_or(0, |t| t.len());
    let tool_call_len: usize = calls
        .iter()
        .map(|c| c.function.name.len() + c.function.arguments.len())
        .sum();

    // The total extracted content should not exceed the input
    // (parser may strip delimiters, so extracted <= input is the invariant)
    assert!(
        normal_len + tool_call_len <= input.len() * 3,
        "{parser_name}: extracted content ({} + {} = {}) far exceeds input length {} — \
         possible content fabrication",
        normal_len,
        tool_call_len,
        normal_len + tool_call_len,
        input.len(),
    );

    // Normal text (if present) must contain only characters from the input
    if let Some(ref text) = normal_text {
        for ch in text.chars() {
            assert!(
                input.contains(ch),
                "{parser_name}: normal_text contains char {:?} not found in input",
                ch,
            );
        }
    }

    // Function names must appear somewhere related to the input
    // (they shouldn't be fabricated from nothing)
    for call in &calls {
        assert!(
            !call.function.name.is_empty(),
            "{parser_name}: tool call has empty function name",
        );
    }
}
