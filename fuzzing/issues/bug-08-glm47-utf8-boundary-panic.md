### [BUG]: GLM-4.7 parser panics on non-ASCII input due to trim offset miscalculation

### Describe the Bug

The GLM-4.7 tool call parser in `lib/parsers/src/tool_calling/xml/glm47_parser.rs` (lines 203-216) extracts the function name by trimming whitespace, then uses the trimmed name's byte length to slice back into the original content string:

```rust
let function_name = if let Some(pos) = content.find(arg_key_start.as_str()) {
    content[..pos].trim().to_string()
} else {
    content.trim().to_string()
};
// ...
let args_section = &content[function_name.len()..];
```

When `content` has leading whitespace, `trim()` removes it but the slice offset calculation uses `len()` of the trimmed name, which doesn't account for the removed whitespace prefix. With multibyte UTF-8 characters, this causes slicing at a non-char-boundary, panicking.

### Steps to Reproduce

```rust
// Content with leading whitespace and multibyte characters:
let content = "  café\n{}";
// After trim: "café" (len = 5 bytes due to UTF-8 é)
// content[5..] slices into the middle of the original string at the wrong offset
// because the 2-byte whitespace prefix was removed by trim but not accounted for
```

The exact trigger requires a GLM-4.7 formatted tool call where the function name section has leading whitespace and contains multibyte UTF-8 characters.

### Expected Behavior

The parser should correctly handle whitespace and multibyte characters without panicking.

### Actual Behavior

```
thread 'main' panicked at 'byte index N is not a char boundary'
```

### Suggested Fix

Use `content.find(&function_name)` to get the correct byte offset of the trimmed name within the original content, or track the trim offset separately:

```rust
let trimmed_start = content.len() - content.trim_start().len();
let args_section = &content[trimmed_start + function_name.len()..];
```

### Environment

- dynamo: main branch
- Crate: `dynamo-parsers`
- File: `lib/parsers/src/tool_calling/xml/glm47_parser.rs`, lines 203-216
