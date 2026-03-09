# XML parser try_literal_eval corrupts argument values containing Python keywords

## Summary

The `try_literal_eval` function in the XML tool call parser uses global string replacements (`"True"` → `"true"`, `"False"` → `"false"`, `"None"` → `"null"`) to handle Python-style literals. These replacements corrupt argument values that contain these substrings, e.g., `"TrueNorth"` becomes `"trueNorth"` and `"NoneAvailable"` becomes `"nullAvailable"`.

## Severity

**Medium** — Data corruption in tool call arguments. The function name and parameter names are unaffected, but parameter values are silently modified. This can cause downstream tool execution to fail or produce wrong results.

## Steps to Reproduce

```rust
// In the XML parser's try_literal_eval function:
let input = r#"{"destination": "TrueNorth"}"#;
// After .replace("True", "true"):
// → {"destination": "trueNorth"}
// The capital T in "TrueNorth" is lowercased, corrupting the value
```

### Via parser:

```rust
use dynamo_parsers::xml::try_tool_call_parse_xml;
use dynamo_parsers::config::XmlParserConfig;

let input = r#"<tool_call><function=navigate><parameter=dest>TrueNorth</parameter></function></tool_call>"#;
let config = XmlParserConfig::default();
let (calls, _) = try_tool_call_parse_xml(input, &config, None).unwrap();
// calls[0].function.arguments will contain "trueNorth" instead of "TrueNorth"
```

### Additional cases:

```rust
// "Falsehood" → "falsehood"
let input = r#"{"claim": "Falsehood"}"#;

// "NoneAvailable" → "nullAvailable"
let input = r#"{"status": "NoneAvailable"}"#;
```

## Root Cause

In `xml/parser.rs:490-497`:

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

The `.replace()` calls are global — they replace ALL occurrences of these substrings, not just standalone Python keywords. When the value is already valid JSON (first `serde_json::from_str` succeeds), the replacements are skipped. But when the input uses Python-style single quotes, the replacements are applied globally.

## Existing Tests

The bug is acknowledged in the test suite:
- `test_try_literal_eval_corrupts_true_in_string_values` (line 945) — `"TrueNorth"` → `"trueNorth"`
- `test_try_literal_eval_corrupts_false_in_string_values` (line 957) — `"Falsehood"` → `"falsehood"`
- `test_try_literal_eval_corrupts_none_in_string_values` (line 968) — `"NoneAvailable"` → `"nullAvailable"`

## Suggested Fix

Use word-boundary-aware replacements instead of global string replace:

```rust
use regex::Regex;

let normalized = s.replace('\'', "\"");
let normalized = Regex::new(r"\bTrue\b").unwrap().replace_all(&normalized, "true");
let normalized = Regex::new(r"\bFalse\b").unwrap().replace_all(&normalized, "false");
let normalized = Regex::new(r"\bNone\b").unwrap().replace_all(&normalized, "null");
```

Or more efficiently, only apply replacements outside of quoted strings.

Found by: code review (exploration agent).
