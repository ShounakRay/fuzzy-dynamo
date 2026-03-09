# Pythonic tool call parser includes prefix text in function name

## Summary

`try_tool_call_parse_pythonic` incorrectly includes characters from surrounding text in the extracted function name. When a tool call like `get_weather(location="NYC")` is embedded in arbitrary text, the parser may include adjacent characters as part of the function name.

## Severity

**High** — Incorrect function name extraction means tool calls are dispatched to the wrong function. In LLM inference serving, this causes tool call failures or, worse, calling the wrong tool with valid arguments.

## Steps to Reproduce

```rust
use dynamo_parsers::try_tool_call_parse_pythonic;

// Tool call embedded in surrounding text
let input = r#"[vvvvvvvvv[v[vv]get_weather(location="NYC")"#;

let (calls, _) = try_tool_call_parse_pythonic(input, None).unwrap();
assert_eq!(calls[0].function.name, "get_weather");
// FAILS: actual name is "vvget_weather" — includes "vv" prefix from surrounding text
```

### Minimal reproduction:

```rust
// Even a single extra character leaks into the function name
let input = r#"m]get_weather(location="NYC")"#;
let (calls, _) = try_tool_call_parse_pythonic(input, None).unwrap();
// calls[0].function.name == "mget_weather" — wrong!
```

## Root Cause

The pythonic parser scans backward from `(` to find the function name start. It likely uses a character class (e.g., `[a-zA-Z0-9_]`) to determine which characters belong to the function name. Characters like `v`, `m`, etc. that immediately precede `get_weather` without a separator are incorrectly included because they match the identifier character class.

The parser should require that the function name starts at a word boundary — i.e., the character before the function name should NOT be an identifier character.

## Crash Artifacts

- `crash-3a33dc87a512da1a1877cdd0de29326ff237cd76` — input `[vvvvvvvvv[v[vv]` prefix → `"vvget_weather"`
- `crash-e4b819f2e32c87ff59f0d9a85f9b762f1f0eaaa5` — input `[ [m]]]]` prefix → `"mget_weather"`

## Suggested Fix

When scanning backward from `(` to find the function name, stop at the first non-identifier character AND verify the character before the function name start is not an identifier character. This ensures we extract only complete identifiers, not fragments of surrounding text.

Found by: `fuzz_parser_semantic` fuzzer.
