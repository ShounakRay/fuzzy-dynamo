### [BUG]: `strip_quotes` panics on single-character quote input

### What This Bug Is (Plain English)

There's a helper function that strips matching quotes from strings — turning `"hello"` into `hello`. It checks if the string starts and ends with a quote, then slices off the first and last characters.

The problem: if the string is *just* a single quote character (`"`), it both starts and ends with a quote (it's the same character). The function tries to slice from position 1 to position 0, which is impossible, and crashes. This can happen when a model outputs a malformed tool call argument that's just a lone quote character.

### Describe the Bug

The `strip_quotes` function in `lib/parsers/src/tool_calling/xml/parser.rs` (lines 19-28) panics when the input is a single quote character (`"` or `'`):

```rust
fn strip_quotes(s: &str) -> &str {
    let trimmed = s.trim();
    if (trimmed.starts_with('"') && trimmed.ends_with('"'))
        || (trimmed.starts_with('\'') && trimmed.ends_with('\''))
    {
        &trimmed[1..trimmed.len() - 1]  // panics when len() == 1
    } else {
        trimmed
    }
}
```

When `trimmed` is a single quote character (length 1), both `starts_with('"')` and `ends_with('"')` return true (same character), but the slice `[1..0]` panics because the begin index exceeds the end index.

### Steps to Reproduce

```rust
// This panics with "range end index 0 is less than range start index 1":
let _ = strip_quotes("\"");
let _ = strip_quotes("'");
```

This can be triggered in production when an XML tool call parser encounters a parameter value that is just a single quote character, which is plausible with malformed model output.

### Expected Behavior

`strip_quotes("\"")` should return `""` (empty string) or the original `"\""`.

### Actual Behavior

```
thread 'main' panicked at 'range end index 0 is less than range start index 1'
```

### Suggested Fix

Add a length check before slicing:

```rust
if trimmed.len() >= 2
    && ((trimmed.starts_with('"') && trimmed.ends_with('"'))
        || (trimmed.starts_with('\'') && trimmed.ends_with('\'')))
{
    &trimmed[1..trimmed.len() - 1]
} else {
    trimmed
}
```

### Environment

- dynamo: main branch
- Crate: `dynamo-parsers`
- File: `lib/parsers/src/tool_calling/xml/parser.rs`, lines 19-28
