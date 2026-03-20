# [BUG]: KimiK2 parser OnceLock caches regex from first config, ignores subsequent configs

### Describe the Bug

The KimiK2 tool call parser uses `OnceLock` to cache compiled regexes, but the regexes are built from `KimiK2ParserConfig` tokens (`call_start`, `argument_begin`, `call_end`). Since `OnceLock::get_or_init` only initializes once per process, the first config's tokens are baked into the regex permanently. All subsequent calls with different configs silently use the stale regex.

In `kimi_k2_parser.rs:17-33`:

```rust
static TOOL_CALL_REGEX: OnceLock<Regex> = OnceLock::new();

fn get_tool_call_regex(config: &KimiK2ParserConfig) -> &'static Regex {
    TOOL_CALL_REGEX.get_or_init(|| {
        let pattern = format!(
            r"(?s){}\s*(?P<function_id>[\w.]+:\d+)\s*{}\s*(?P<arguments>\{{.*?\}})\s*{}",
            regex::escape(&config.call_start),
            regex::escape(&config.argument_begin),
            regex::escape(&config.call_end),
        );
        Regex::new(&pattern).expect("invalid tool call regex")
    })
}
```

`OnceLock::get_or_init` initializes the value exactly once. The `config` parameter is only used during the first call — all subsequent calls return the cached regex regardless of what config is passed.

### Steps to Reproduce

```rust
use dynamo_parsers::config::KimiK2ParserConfig;
use dynamo_parsers::xml::try_tool_call_parse_kimi_k2;

// First call with default config — works fine
let default_config = KimiK2ParserConfig::default();
let input1 = r#"<|tool_calls_section_begin|><|tool_call_begin|>functions.test:0<|tool_call_argument_begin|>{"key":"val"}<|tool_call_end|><|tool_calls_section_end|>"#;
let (calls, _) = try_tool_call_parse_kimi_k2(input1, &default_config, None).unwrap();
assert_eq!(calls.len(), 1); // OK

// Second call with custom config — silently broken
let custom_config = KimiK2ParserConfig {
    call_start: "[CALL_START]".to_string(),
    argument_begin: "[ARGS]".to_string(),
    call_end: "[CALL_END]".to_string(),
    ..default_config.clone()
};
let input2 = r#"<|tool_calls_section_begin|>[CALL_START]functions.test:0[ARGS]{"key":"val"}[CALL_END]<|tool_calls_section_end|>"#;
let (calls, _) = try_tool_call_parse_kimi_k2(input2, &custom_config, None).unwrap();
assert_eq!(calls.len(), 0); // BUG: returns 0 instead of 1
```

### Expected Behavior

The parser should use the config passed to each call to build or select the appropriate regex pattern, returning 1 parsed tool call for the custom config input.

### Actual Behavior

The second call silently uses the stale regex from the first config and returns 0 tool calls. No error or warning is emitted.

### Environment

On commit `57e6a79f5` in `dynamo-runtime` @ `lib/parsers/src/kimi_k2_parser.rs`

### Additional Context

In a multi-model serving scenario where different models use different KimiK2 token formats, only the first model's config would work correctly. Other models would silently fail to parse tool calls.

The bug is acknowledged in the test suite: `test_oncelock_regex_depends_on_config_but_cached_statically` (line 636) demonstrates the issue.

A possible fix is to replace `OnceLock` with a config-aware caching strategy (e.g., remove caching entirely since regexes are cheap for small patterns, or use a `Mutex<HashMap<ConfigKey, Regex>>` keyed by config tokens).

Found via fuzzing with cargo-fuzz / libfuzzer.

### Screenshots

_No response_
