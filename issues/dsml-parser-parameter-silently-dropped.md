# DSML parser silently drops parameters with capitalized or missing string attribute

## Summary

The DeepSeek V3.2 DSML tool call parser (`try_tool_call_parse_dsml`) silently drops parameters when:
1. The `string` attribute is capitalized (e.g., `string="True"` instead of `string="true"`)
2. The `string` attribute is omitted entirely

In both cases, the parameter regex simply doesn't match, and the parameter is silently lost with no error or warning.

## Severity

**Medium** — Data loss without any error. When an LLM emits a tool call with `string="True"` (common in Python-influenced models) or omits the attribute, the parameter silently disappears from the parsed result. The downstream tool receives incomplete arguments.

## Steps to Reproduce

### Case 1: Capitalized string attribute

```rust
use dynamo_parsers::dsml::try_tool_call_parse_dsml;
use dynamo_parsers::config::DsmlParserConfig;

let input = r#"<｜DSML｜function_calls>
<｜DSML｜invoke name="test">
<｜DSML｜parameter name="query" string="True">hello world</｜DSML｜parameter>
</｜DSML｜invoke>
</｜DSML｜function_calls>"#;

let config = DsmlParserConfig::default();
let (calls, _) = try_tool_call_parse_dsml(input, &config).unwrap();
assert_eq!(calls.len(), 1);

let args: serde_json::Value = serde_json::from_str(&calls[0].function.arguments).unwrap();
// BUG: args["query"] is null — parameter was silently dropped
assert_eq!(args.get("query"), None); // Missing!
```

### Case 2: Missing string attribute

```rust
let input = r#"<｜DSML｜function_calls>
<｜DSML｜invoke name="test">
<｜DSML｜parameter name="value">42</｜DSML｜parameter>
</｜DSML｜invoke>
</｜DSML｜function_calls>"#;

let (calls, _) = try_tool_call_parse_dsml(input, &config).unwrap();
// BUG: args["value"] is null — parameter was silently dropped
```

## Root Cause

In `dsml/parser.rs:173-176`:

```rust
let param_pattern = format!(
    r#"(?s){}\"([^"]+)\"\s+string=\"(true|false)\"\s*>(.*?){}"#,
    prefix_escaped, end_escaped
);
```

The regex `string=\"(true|false)\"` has two problems:
1. It only matches lowercase `true` and `false` — `True`, `False`, `TRUE`, `FALSE` all fail to match
2. The `string` attribute is required by the regex — if omitted, the entire parameter fails to match

When the regex doesn't match, the parameter is silently skipped in the `for param_match in param_regex.captures_iter(content)` loop.

## Existing Tests

Both bugs are acknowledged in the test suite:
- `test_parse_parameter_with_capitalized_string_attr` (line 472) — currently fails
- `test_parse_parameter_without_string_attr` (line 492) — currently fails

## Suggested Fix

1. Make the `string` attribute case-insensitive: `string=\"(?i)(true|false)\"`
2. Make the `string` attribute optional with a default: `(?:\s+string=\"(true|false)\")?\s*>`
3. When `string` attribute is missing, default to treating the value as non-string (try JSON parse, fall back to string)

Found by: code review during fuzz target development.
