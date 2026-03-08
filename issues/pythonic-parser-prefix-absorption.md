# Pythonic Parser: Absorbs Prefix Characters into Function Name

## Summary

The pythonic tool call parser (`try_tool_call_parse_pythonic`) incorrectly
absorbs characters from text preceding a valid tool call into the function name.
When a valid call like `get_weather(location="NYC")` is preceded by certain
text (e.g., ending in `"m"`), the parser returns function name `"mget_weather"`
instead of `"get_weather"`.

## Steps to Reproduce

### Via fuzzing

```bash
cd lib/parsers/fuzz
~/.cargo/bin/cargo +nightly fuzz run fuzz_parser_semantic \
  artifacts/fuzz_parser_semantic/crash-e4b819f2e32c87ff59f0d9a85f9b762f1f0eaaa5
```

### Minimal Rust code

```rust
use dynamo_parsers::try_tool_call_parse_pythonic;

// Text with prefix ending in 'm' followed by valid pythonic call
let input = r#"...m get_weather(location="NYC")..."#;
let (calls, _) = try_tool_call_parse_pythonic(input, None).unwrap();
assert_eq!(calls[0].function.name, "get_weather");
// FAILS: returns "mget_weather" or similar prefix-absorbed name
```

## Root Cause

The pythonic parser's function name extraction regex or scanning logic does not
properly anchor to word boundaries. When scanning backwards from the opening
parenthesis to find the function name start, it includes adjacent non-whitespace
characters from the preceding text.

## Impact

- **Severity**: Medium — causes incorrect tool call extraction when tool calls
  are embedded in larger text (common in LLM output with reasoning before calls)
- **Affected parser**: `try_tool_call_parse_pythonic`
- **Workaround**: Ensure LLM output has whitespace or newline before pythonic
  tool calls

## Suggested Fix

Add a word boundary check in the function name extraction logic. The function
name should start only at:
- Beginning of input
- After whitespace or newline
- After certain delimiter characters (`;`, `,`, etc.)

This prevents adjacent text from being absorbed into the function identifier.
