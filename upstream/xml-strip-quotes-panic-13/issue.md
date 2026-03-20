# [BUG]: XML parser strip_quotes panics on single-character quote input

### Describe the Bug

The `strip_quotes` helper function in `lib/parsers/src/tool_calling/xml/parser.rs` panics with "begin <= end" when given a string that is exactly a single quote character (`"` or `'`). The function checks `starts_with('"') && ends_with('"')`, which is true for a single `"` character, then attempts `&trimmed[1..trimmed.len() - 1]` which evaluates to `&trimmed[1..0]` — an invalid slice where begin > end.

`strip_quotes` is called from line 151 on regex-captured function names and line 166 on parameter names, so a model output like `<function=">` triggers the crash.

### Steps to Reproduce

Direct reproduction:

```rust
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

Via the parser:

```rust
use dynamo_parsers::xml::try_tool_call_parse_xml;
use dynamo_parsers::config::XmlParserConfig;

let config = XmlParserConfig::default();
// Function name is a single quote character
let input = "<tool_call><function=\"><parameter=x>val</parameter></function></tool_call>";
let _ = try_tool_call_parse_xml(input, &config, None);
// PANICS at parser.rs:24
```

### Expected Behavior

Should handle single-character quote strings without panicking, either by returning an empty string or the original input.

### Actual Behavior

```
thread panicked at 'begin <= end (1 <= 0)'
  at lib/parsers/src/tool_calling/xml/parser.rs:24
```

### Environment

On commit `57e6a79f5` in `dynamo-runtime` @ `lib/parsers/src/tool_calling/xml/parser.rs`

### Additional Context

The bug is already acknowledged in the test suite: `test_strip_quotes_panics_on_single_quote_char` (line 912) is a `#[should_panic]` test confirming the bug exists, and `test_strip_quotes_single_quote_char_should_not_panic` (line 922) is a guard test that currently fails.

The fix would be to add a `trimmed.len() >= 2` check before the starts_with/ends_with condition.

Found via fuzzing with cargo-fuzz / libfuzzer.

### Screenshots

_No response_
