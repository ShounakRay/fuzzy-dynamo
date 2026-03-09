# Pythonic tool call parser absorbs prefix characters into function name

## Summary

`try_tool_call_parse_pythonic` incorrectly includes characters from surrounding
text in the extracted function name. When a tool call like
`get_weather(location="NYC")` is embedded in arbitrary text, the parser absorbs
adjacent identifier characters as part of the function name.

## Severity

**High** — Incorrect function name extraction means tool calls are dispatched to
the wrong function. In LLM inference serving, this causes tool call failures or,
worse, calling the wrong tool with valid arguments.

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

The pythonic parser regex at `pythonic_parser.rs:22` uses pattern `[a-zA-Z]+\w*\(`
which does not enforce a word boundary before the function name. Characters like
`v`, `m`, etc. that immediately precede the real function name match the
`[a-zA-Z]` class and get absorbed because there's no assertion that the
preceding character is NOT an identifier character.

## Crash Artifacts

- `crash-3a33dc87a512da1a1877cdd0de29326ff237cd76` — input `[vvvvvvvvv[v[vv]` prefix → `"vvget_weather"`
- `crash-e4b819f2e32c87ff59f0d9a85f9b762f1f0eaaa5` — input `[ [m]]]]` prefix → `"mget_weather"`

## Suggested Fix

Add a word boundary or lookbehind assertion to the regex so the function name
must start at a word boundary — i.e., the character before the function name
should NOT be an identifier character (`\b` or `(?<![a-zA-Z0-9_])`).

Found by: `fuzz_parser_semantic` fuzzer.
