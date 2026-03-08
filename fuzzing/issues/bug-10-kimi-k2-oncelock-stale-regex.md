### [BUG]: Kimi K2 parser caches regex from first config, ignores subsequent configs

### What This Bug Is (Plain English)

The Kimi K2 parser builds a regular expression (a text-matching pattern) based on its configuration — things like what tokens mark the start and end of a tool call. To avoid rebuilding this pattern every time, it caches it globally the first time it's called.

The problem: it caches the pattern from the *first* config it ever sees and then uses that same pattern for all future calls, regardless of what config you pass in. If you later call it with different start/end tokens, it silently ignores your config and parses with the old pattern. The function signature promises "give me a config and I'll use it," but it's lying after the first call.

Currently benign because only one config exists in practice, but it's a landmine for anyone who tries to use a second config.

### Describe the Bug

The Kimi K2 tool call parser in `lib/parsers/src/tool_calling/xml/kimi_k2_parser.rs` (lines 26-36) builds a regex from the config parameter but stores it in a static `OnceLock`:

```rust
fn get_tool_call_regex(config: &KimiK2ParserConfig) -> &'static Regex {
    TOOL_CALL_REGEX.get_or_init(|| {
        let pattern = format!(
            r"(?s){}\s*(?P<function_id>[\w.]+:\d+)\s*{}\s*(?P<arguments>\{{.*?\}})\s*{}",
            regex::escape(&config.call_start),
            regex::escape(&config.argument_begin),
            regex::escape(&config.call_end),
        );
        Regex::new(&pattern).expect("Failed to compile kimi k2 tool call regex")
    })
}
```

Only the **first** call's config is used to build the regex. `OnceLock::get_or_init` ignores the closure (and therefore the `config` parameter) on all subsequent calls. If the function is ever called with different config values in the same process, parsing silently uses the stale cached regex from the first invocation.

### Steps to Reproduce

```rust
use dynamo_parsers::tool_calling::xml::kimi_k2_parser;

// First call — regex built from config_a
let config_a = KimiK2ParserConfig {
    call_start: "<|tool_call|>".into(),
    // ...
};
let _ = try_tool_call_parse_kimi_k2(input, &config_a, None);

// Second call with different config — silently uses config_a's regex
let config_b = KimiK2ParserConfig {
    call_start: "<|custom_tool|>".into(),
    // ...
};
let _ = try_tool_call_parse_kimi_k2(input, &config_b, None); // WRONG regex used
```

### Expected Behavior

Each distinct config should produce a correctly-compiled regex. Or at minimum, the function should validate that the provided config matches the cached regex.

### Actual Behavior

The second call silently uses the first call's regex, causing incorrect parsing results with no error.

### Suggested Fix

Either:
1. Compute the regex each time (it's fast enough for the call frequency)
2. Use a `HashMap<ConfigKey, Regex>` keyed by the config's tokens
3. Assert that the config matches the cached version and panic/warn if not

### Additional Context

**Note**: This is currently benign because only one `KimiK2ParserConfig` exists in practice (the default). The bug would only manifest if a second distinct config were introduced. However, the API signature accepting `&KimiK2ParserConfig` creates a false contract — callers reasonably expect different configs to produce different behavior.

### Environment

- dynamo: main branch
- Crate: `dynamo-parsers`
- File: `lib/parsers/src/tool_calling/xml/kimi_k2_parser.rs`, lines 26-36
