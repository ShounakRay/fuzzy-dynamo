# [BUG]: Pythonic tool call parser absorbs prefix characters into function name

### Describe the Bug

`try_tool_call_parse_pythonic` in `lib/parsers/src/pythonic_parser.rs` incorrectly includes characters from surrounding text in the extracted function name. The parser regex at line 22 uses pattern `[a-zA-Z]+\w*\(` which does not enforce a word boundary before the function name. Characters like `v`, `m`, etc. that immediately precede the real function name match the `[a-zA-Z]` class and get absorbed because there's no assertion that the preceding character is not an identifier character.

### Steps to Reproduce

```rust
use dynamo_parsers::try_tool_call_parse_pythonic;

// Tool call embedded in surrounding text
let input = r#"[vvvvvvvvv[v[vv]get_weather(location="NYC")"#;

let (calls, _) = try_tool_call_parse_pythonic(input, None).unwrap();
assert_eq!(calls[0].function.name, "get_weather");
// FAILS: actual name is "vvget_weather" — includes "vv" prefix from surrounding text
```

Even a single extra character leaks into the function name:

```rust
let input = r#"m]get_weather(location="NYC")"#;
let (calls, _) = try_tool_call_parse_pythonic(input, None).unwrap();
// calls[0].function.name == "mget_weather" — wrong!
```

### Expected Behavior

The extracted function name should be `"get_weather"`, not `"vvget_weather"` or `"mget_weather"`. Characters from surrounding text should not leak into the function name.

### Actual Behavior

The regex matches greedily from the first alphabetic character it finds before the real function name, absorbing adjacent identifier characters as part of the name. This causes tool calls to be dispatched to the wrong function.

### Environment

On commit `57e6a79f5` in `dynamo-runtime` @ `lib/parsers/src/pythonic_parser.rs` (line 22)

### Additional Context

Incorrect function name extraction means tool calls are dispatched to the wrong function. In LLM inference serving, this causes tool call failures or, worse, calling the wrong tool with valid arguments. A fix would be to add a word boundary or lookbehind assertion to the regex so the function name must start at a word boundary, e.g., `\b[a-zA-Z]+\w*\(` or `(?<![a-zA-Z0-9_])[a-zA-Z]+\w*\(`.

Found via fuzzing with cargo-fuzz / libfuzzer.

### Screenshots

_No response_
