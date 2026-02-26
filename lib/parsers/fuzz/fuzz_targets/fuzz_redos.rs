#![no_main]
use libfuzzer_sys::fuzz_target;
use dynamo_parsers::*;
use dynamo_parsers::tool_calling::config::Glm47ParserConfig;
use dynamo_parsers::tool_calling::xml::try_tool_call_parse_glm47;

// ReDoS-focused fuzz harness.
//
// Exercises regex-heavy parsers with inputs designed to trigger catastrophic
// backtracking. Should be run with short per-input timeout (2s) and small
// max_len (1024 bytes):
//
//   FUZZ_TIMEOUT_PER_INPUT=2 FUZZ_MAX_LEN=1024 cargo +nightly fuzz run fuzz_redos
//
// Target parsers:
// - Pythonic: complex nested capture groups with .*? quantifiers
// - XML: parameter extraction regex with (?s) dotall mode
// - GLM-4.7: key-value extraction regex with (.*?) captures
fuzz_target!(|data: &[u8]| {
    let Ok(s) = std::str::from_utf8(data) else { return };

    // Pythonic parser: most complex regex with nested groups and .*? quantifiers.
    // Pattern: \[([a-zA-Z]+\w*\(([a-zA-Z]+\w*=.*?,\s*)*(...)\)\s*)+\]
    // Almost-matching inputs (e.g., "[func(a=" repeated) can cause exponential backtracking.
    let _ = try_tool_call_parse_pythonic(s, None);

    // XML parser: function and parameter extraction regexes use (?s) and (.*?)
    let _ = try_tool_call_parse_xml(s, &XmlParserConfig::default(), None);

    // GLM-4.7: key-value extraction regex with (.*?) in dotall mode
    let _ = try_tool_call_parse_glm47(s, &Glm47ParserConfig::default(), None);

    // Also exercise detect_tool_call_start which does partial string matching loops
    let _ = detect_tool_call_start(s, Some("pythonic"));
    let _ = detect_tool_call_start(s, Some("qwen3_coder"));
    let _ = detect_tool_call_start(s, Some("glm47"));
    let _ = detect_tool_call_start(s, Some("kimi_k2"));
});
