// Fix for Bug 9: KimiK2 parser OnceLock caches regex from first config
// File: lib/parsers/src/tool_calling/xml/kimi_k2_parser.rs
// Severity: MEDIUM
//
// Problem: OnceLock::get_or_init only initializes once per process. The first
//   KimiK2ParserConfig's tokens are permanently baked into the regex; all
//   subsequent calls with different configs silently use the stale regex.
// Fix: Remove the static OnceLock and compile the regex fresh each call.
//   Regex compilation for short patterns is ~1-2 us -- negligible vs network IO.

// === ORIGINAL (lines 17-37) ===
// static TOOL_CALL_REGEX: OnceLock<Regex> = OnceLock::new();
//
// fn get_tool_call_regex(config: &KimiK2ParserConfig) -> &'static Regex {
//     TOOL_CALL_REGEX.get_or_init(|| {
//         let pattern = format!(
//             r"(?s){}\s*(?P<function_id>[\w.]+:\d+)\s*{}\s*(?P<arguments>\{{.*?\}})\s*{}",
//             regex::escape(&config.call_start),
//             regex::escape(&config.argument_begin),
//             regex::escape(&config.call_end),
//         );
//         Regex::new(&pattern).expect("Failed to compile kimi k2 tool call regex")
//     })
// }

// === FIXED ===
fn get_tool_call_regex(config: &KimiK2ParserConfig) -> Regex {
    let pattern = format!(
        r"(?s){}\s*(?P<function_id>[\w.]+:\d+)\s*{}\s*(?P<arguments>\{{.*?\}})\s*{}",
        regex::escape(&config.call_start),
        regex::escape(&config.argument_begin),
        regex::escape(&config.call_end),
    );
    Regex::new(&pattern).expect("Failed to compile kimi k2 tool call regex")
}
// Note: callers must change `get_tool_call_regex(config)` return usage from
// `&'static Regex` to `Regex` (owned). Since the regex is used immediately
// in captures_iter, no further changes are needed beyond adjusting the type.

// === TEST ===
#[test]
fn test_kimi_k2_regex_respects_different_configs() {
    use regex::Regex;

    let default_config = KimiK2ParserConfig::default();
    let custom_config = KimiK2ParserConfig {
        call_start: "[CALL_START]".to_string(),
        argument_begin: "[ARGS]".to_string(),
        call_end: "[CALL_END]".to_string(),
        ..default_config.clone()
    };

    let re_default = get_tool_call_regex(&default_config);
    let re_custom = get_tool_call_regex(&custom_config);

    // Default config regex should match default-format input
    let input_default = r#"<|tool_call_begin|>functions.test:0<|tool_call_argument_begin|>{"key":"val"}<|tool_call_end|>"#;
    assert!(re_default.is_match(input_default));
    assert!(!re_custom.is_match(input_default));

    // Custom config regex should match custom-format input
    let input_custom = r#"[CALL_START]functions.test:0[ARGS]{"key":"val"}[CALL_END]"#;
    assert!(!re_default.is_match(input_custom));
    assert!(re_custom.is_match(input_custom));
}
