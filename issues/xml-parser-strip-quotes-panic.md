# XML parser strip_quotes panics on single-character quote input

## Summary

The `strip_quotes` helper function in the XML tool call parser panics with "begin <= end" when given a string that is exactly a single quote character (`"` or `'`). This function is called on regex-captured function names and parameter names, so a model output like `<function=">` triggers the crash.

## Severity

**Medium** — Requires specific model output to trigger (a function name or parameter name that is exactly one quote character). Unlikely in normal operation but possible with adversarial prompts or model hallucinations.

## Steps to Reproduce

```rust
// Direct reproduction:
fn strip_quotes(s: &str) -> &str {
    let trimmed = s.trim();
    if (trimmed.starts_with('"') && trimmed.ends_with('"'))
        || (trimmed.starts_with('\'') && trimmed.ends_with('\''))
    {
        &trimmed[1..trimmed.len() - 1]  // PANICS when len == 1
    } else {
        trimmed
    }
}

strip_quotes("\"");  // PANICS: "begin <= end (1 <= 0)"
strip_quotes("'");   // PANICS: same reason
```

### Via parser:

```rust
use dynamo_parsers::xml::try_tool_call_parse_xml;
use dynamo_parsers::config::XmlParserConfig;

let config = XmlParserConfig::default();
// Function name is a single quote character
let input = "<tool_call><function=\"><parameter=x>val</parameter></function></tool_call>";
let _ = try_tool_call_parse_xml(input, &config, None);
// PANICS at parser.rs:24
```

## Root Cause

In `lib/parsers/src/tool_calling/xml/parser.rs:19-28`:

```rust
fn strip_quotes(s: &str) -> &str {
    let trimmed = s.trim();
    if (trimmed.starts_with('"') && trimmed.ends_with('"'))
        || (trimmed.starts_with('\'') && trimmed.ends_with('\''))
    {
        &trimmed[1..trimmed.len() - 1]  // line 24
    } else {
        trimmed
    }
}
```

When `trimmed` is `"\"" ` (length 1):
1. `starts_with('"')` → true
2. `ends_with('"')` → true (same character)
3. `&trimmed[1..0]` → panic: slice begin (1) > end (0)

The check doesn't verify `trimmed.len() >= 2` before slicing.

## Call Path

`strip_quotes` is called from:
- Line 151: `strip_quotes(function_name_raw)` — on regex-captured function names
- Line 166: `strip_quotes(param_name_raw)` — on regex-captured parameter names

Both are reachable from `try_tool_call_parse_xml` with crafted/fuzz input.

## Existing Tests

The bug is already acknowledged in the test suite:
- `test_strip_quotes_panics_on_single_quote_char` (line 912) — `#[should_panic]` test confirming the bug exists
- `test_strip_quotes_single_quote_char_should_not_panic` (line 922) — guard test that currently fails

## Suggested Fix

Add a length check before slicing:

```rust
fn strip_quotes(s: &str) -> &str {
    let trimmed = s.trim();
    if trimmed.len() >= 2
        && ((trimmed.starts_with('"') && trimmed.ends_with('"'))
            || (trimmed.starts_with('\'') && trimmed.ends_with('\'')))
    {
        &trimmed[1..trimmed.len() - 1]
    } else {
        trimmed
    }
}
```

Found by: code review during fuzz target development.
