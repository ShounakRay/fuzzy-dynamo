# Bug 9: KimiK2 parser: OnceLock caches regex from first config, ignores subsequent configs

## Summary

The KimiK2 tool call parser uses `OnceLock` to cache compiled regexes, but the regexes are built from `KimiK2ParserConfig` tokens (`call_start`, `argument_begin`, `call_end`). Since `OnceLock::get_or_init` only initializes once per process, the first config's tokens are baked into the regex permanently. All subsequent calls with different configs silently use the stale regex.

## Severity

**Medium** — Logic bug, not a crash. In a multi-model serving scenario where different models use different KimiK2 token formats, only the first model's config would work correctly. Other models would silently fail to parse tool calls.

## Steps to Reproduce

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

## Root Cause

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

## Existing Tests

The bug is acknowledged in the test suite:
- `test_oncelock_regex_depends_on_config_but_cached_statically` (line 636) — demonstrates the issue

## Suggested Fix

Replace `OnceLock` with a config-aware caching strategy:

1. **Simplest**: Remove caching entirely — compile regex on every call (regexes are cheap for small patterns)
2. **Better**: Use a `Mutex<HashMap<ConfigKey, Regex>>` where `ConfigKey` is derived from the config tokens
3. **Best**: Use `thread_local!` with per-thread caching keyed by config

Found by: code review during fuzz target development.
