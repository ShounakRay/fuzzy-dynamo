# [BUG]: GLM-4.7 parser panics on multibyte UTF-8 function names with leading whitespace

### Describe the Bug

`try_tool_call_parse_glm47` in `lib/parsers/src/tool_calling/xml/glm47_parser.rs` panics with "byte index is not a char boundary" when parsing tool calls where the content between `<tool_call>` tags has leading whitespace and the function name contains multibyte UTF-8 characters (e.g., CJK, Cyrillic, emoji).

The root cause is at line 216: after trimming whitespace from the function name, the code uses `function_name.len()` (the byte length of the *trimmed* name) as a byte offset into the *untrimmed* `content` string. Trimming removes leading whitespace, so the byte offset no longer corresponds to a char boundary. For example, `content[..pos].trim()` produces a 4-byte string, but byte 4 in the original `content` falls inside a 2-byte Cyrillic character, causing the panic.

Any model output containing multibyte characters near the function name position with any leading/trailing whitespace will crash the inference server. This is a denial-of-service vector triggered by model output.

### Steps to Reproduce

You can save the following under: `lib/parsers/tests/thebug.rs`

```rust
use dynamo_parsers::config::Glm47ParserConfig;
use dynamo_parsers::xml::try_tool_call_parse_glm47;

let config = Glm47ParserConfig::default();

// Cyrillic character 'ш' (2 bytes UTF-8) with leading spaces
let input = "<tool_call>  .ш\x18\n<arg_key>location</arg_key><arg_value>NYC</arg_value></tool_call>";

// PANICS: "byte index 4 is not a char boundary; it is inside 'ш' (bytes 3..5)"
let _ = try_tool_call_parse_glm47(input, &config, None);
```

Minimal reproduction:

```rust
// Any multibyte char + leading whitespace triggers it
let input = "<tool_call> 获取<arg_key>k</arg_key><arg_value>v</arg_value></tool_call>";
let _ = try_tool_call_parse_glm47(input, &config, None);
// PANICS at glm47_parser.rs:216
```

### Expected Behavior

Should handle multibyte UTF-8 characters correctly without panicking, by using char-boundary-safe indexing into the content string.

### Actual Behavior

```
thread panicked at 'byte index 4 is not a char boundary; it is inside 'ш' (bytes 3..5)'
  at lib/parsers/src/tool_calling/xml/glm47_parser.rs:216
```

### Environment

On commit `57e6a79f5` in `dynamo-runtime` @ `lib/parsers/src/tool_calling/xml/glm47_parser.rs`

### Additional Context

The fix would be to use the original `pos` variable (byte position of `<arg_key>` in `content`) instead of `function_name.len()` when slicing `args_section`, e.g. `let args_section = &content[pos..];`.

Found via fuzzing with cargo-fuzz / libfuzzer.

### Screenshots

_No response_
