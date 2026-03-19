# [BUG]: XML parser try_literal_eval corrupts argument values containing Python keywords

### Describe the Bug

The `try_literal_eval` function in `lib/parsers/src/xml/parser.rs` (lines 490-497) uses global string replacements (`"True"` -> `"true"`, `"False"` -> `"false"`, `"None"` -> `"null"`) to handle Python-style literals. These replacements corrupt argument values that contain these substrings, e.g., `"TrueNorth"` becomes `"trueNorth"` and `"NoneAvailable"` becomes `"nullAvailable"`.

```rust
fn try_literal_eval(s: &str) -> Result<Value, ()> {
    if let Ok(val) = serde_json::from_str::<Value>(s) {
        return Ok(val);
    }
    let normalized = s
        .replace('\'', "\"")
        .replace("True", "true")     // Global: corrupts "TrueNorth"
        .replace("False", "false")   // Global: corrupts "Falsehood"
        .replace("None", "null");    // Global: corrupts "NoneAvailable"
    serde_json::from_str::<Value>(&normalized).map_err(|_| ())
}
```

The `.replace()` calls are global -- they replace ALL occurrences of these substrings, not just standalone Python keywords. When the input uses Python-style single quotes, the replacements are applied globally.

### Steps to Reproduce

```rust
// In the XML parser's try_literal_eval function:
let input = r#"{"destination": "TrueNorth"}"#;
// After .replace("True", "true"):
// -> {"destination": "trueNorth"}
// The capital T in "TrueNorth" is lowercased, corrupting the value
```

Via parser:

```rust
use dynamo_parsers::xml::try_tool_call_parse_xml;
use dynamo_parsers::config::XmlParserConfig;

let input = r#"<tool_call><function=navigate><parameter=dest>TrueNorth</parameter></function></tool_call>"#;
let config = XmlParserConfig::default();
let (calls, _) = try_tool_call_parse_xml(input, &config, None).unwrap();
// calls[0].function.arguments will contain "trueNorth" instead of "TrueNorth"
```

Additional cases:

```rust
// "Falsehood" -> "falsehood"
let input = r#"{"claim": "Falsehood"}"#;

// "NoneAvailable" -> "nullAvailable"
let input = r#"{"status": "NoneAvailable"}"#;
```

### Expected Behavior

Values like `"TrueNorth"`, `"Falsehood"`, and `"NoneAvailable"` should be preserved as-is. Only standalone Python keyword literals (`True`, `False`, `None`) should be replaced with their JSON equivalents.

### Actual Behavior

All occurrences of `True`, `False`, and `None` are replaced globally, including when they appear as substrings within larger values:
- `"TrueNorth"` becomes `"trueNorth"`
- `"Falsehood"` becomes `"falsehood"`
- `"NoneAvailable"` becomes `"nullAvailable"`

The bug is acknowledged in the existing test suite (`test_try_literal_eval_corrupts_true_in_string_values` at line 945, and similar tests for False and None).

### Environment

On commit `57e6a79f5` in `dynamo-runtime` @ `lib/parsers/src/xml/parser.rs` (lines 490-497)

### Additional Context

This is a data corruption bug in tool call arguments. The function name and parameter names are unaffected, but parameter values are silently modified, which can cause downstream tool execution to fail or produce wrong results. A fix would be to use word-boundary-aware replacements (e.g., `\bTrue\b`, `\bFalse\b`, `\bNone\b` via regex) or to only apply replacements outside of quoted strings.

Found via fuzzing with cargo-fuzz / libfuzzer.

### Screenshots

_No response_
