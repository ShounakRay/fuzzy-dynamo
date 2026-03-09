# GLM-4.7 parser panics on multibyte UTF-8 function names with leading whitespace

## Summary

The GLM-4.7 tool call parser (`try_tool_call_parse_glm47`) panics with "byte index is not a char boundary" when parsing tool calls where the content between `<tool_call>` tags has leading whitespace and the function name contains multibyte UTF-8 characters.

## Severity

**High** — Any model output containing multibyte characters (e.g., CJK, Cyrillic, emoji) in or near the function name position with any leading/trailing whitespace will crash the inference server. This is a denial-of-service vector triggered by model output.

## Steps to Reproduce

```rust
use dynamo_parsers::config::Glm47ParserConfig;
use dynamo_parsers::xml::try_tool_call_parse_glm47;

let config = Glm47ParserConfig::default();

// Cyrillic character 'ш' (2 bytes UTF-8) with leading spaces
let input = "<tool_call>  .ш\x18\n<arg_key>location</arg_key><arg_value>NYC</arg_value></tool_call>";

// PANICS: "byte index 4 is not a char boundary; it is inside 'ш' (bytes 3..5)"
let _ = try_tool_call_parse_glm47(input, &config, None);
```

### Minimal reproduction:

```rust
// Any multibyte char + leading whitespace triggers it
let input = "<tool_call> 获取<arg_key>k</arg_key><arg_value>v</arg_value></tool_call>";
let _ = try_tool_call_parse_glm47(input, &config, None);
// PANICS at glm47_parser.rs:216
```

## Root Cause

In `glm47_parser.rs:203-216`:

```rust
let function_name = if let Some(pos) = content.find(arg_key_start.as_str()) {
    content[..pos].trim().to_string()  // trim removes leading/trailing whitespace
} else {
    content.trim().to_string()
};

// ...

let args_section = &content[function_name.len()..];  // BUG: byte offset mismatch
```

The problem:
1. `content` = `"  .ш\x18\n<arg_key>..."` (7 bytes before `<arg_key>`)
2. `content[..7].trim()` = `".ш\x18"` → `function_name` is 4 bytes
3. `content[4..]` tries to index at byte 4, which is inside the 2-byte UTF-8 character `ш` (bytes 3-4)
4. **PANIC**: Rust string slicing requires char-aligned byte boundaries

The fundamental issue is that `function_name.len()` (byte length of the *trimmed* name) doesn't correspond to a valid position in the *untrimmed* `content` string. Trimming removes leading whitespace, shifting the byte offset.

## Crash Artifacts

- `fuzz/artifacts/fuzz_glm47_utf8/crash-ed5713d8cb0206d339613ca7de5428b9856ad393`
- Input bytes: `[46, 209, 136, 24, 10]` (Base64: `LtGIGAo=`)

## Suggested Fix

Use the original `pos` variable (byte position of `<arg_key>` in `content`) instead of `function_name.len()`:

```rust
let args_section = if let Some(pos) = content.find(arg_key_start.as_str()) {
    &content[pos..]
} else {
    ""
};
```

Or equivalently, save `pos` from the function name extraction and reuse it.

Found by: `fuzz_glm47_utf8` fuzzer.
