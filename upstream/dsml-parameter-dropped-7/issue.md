# [BUG]: DSML parser silently drops parameters with capitalized or missing string attribute

### Describe the Bug

The DeepSeek V3.2 DSML tool call parser (`try_tool_call_parse_dsml`) silently drops parameters when:
1. The `string` attribute is capitalized (e.g., `string="True"` instead of `string="true"`)
2. The `string` attribute is omitted entirely

In both cases, the parameter regex simply doesn't match, and the parameter is silently lost with no error or warning. The downstream tool receives incomplete arguments.

The regex in `dsml/parser.rs:173-176` requires a lowercase `string="(true|false)"` attribute. `True`, `False`, `TRUE`, `FALSE` all fail to match. If the attribute is omitted entirely, the entire parameter also fails to match and is silently skipped in the `captures_iter` loop.

### Steps to Reproduce

**Case 1: Capitalized string attribute**

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

**Case 2: Missing string attribute**

```rust
let input = r#"<｜DSML｜function_calls>
<｜DSML｜invoke name="test">
<｜DSML｜parameter name="value">42</｜DSML｜parameter>
</｜DSML｜invoke>
</｜DSML｜function_calls>"#;

let (calls, _) = try_tool_call_parse_dsml(input, &config).unwrap();
// BUG: args["value"] is null — parameter was silently dropped
```

### Expected Behavior

Parameters should be parsed regardless of whether `string` is capitalized (`"True"`) or omitted. When the attribute is missing, the parser should default to treating the value as non-string (try JSON parse, fall back to string).

### Actual Behavior

Parameters with capitalized or missing `string` attributes are silently dropped from the parsed result with no error or warning.

### Environment

On commit `57e6a79f5` in `dynamo-runtime` @ `lib/parsers/src/dsml/parser.rs`

### Additional Context

Both bugs are acknowledged in the test suite:
- `test_parse_parameter_with_capitalized_string_attr` (line 472) — currently fails
- `test_parse_parameter_without_string_attr` (line 492) — currently fails

A possible fix is to make the `string` attribute case-insensitive (`string=\"(?i)(true|false)\"`) and optional with a default (`(?:\s+string=\"(true|false)\")?\s*>`).

Found via fuzzing with cargo-fuzz / libfuzzer.

### Screenshots

_No response_
