### [BUG]: `detect_tool_call_start_xml` and `detect_tool_call_start_glm47` panic on multibyte UTF-8 start tokens

### Describe the Bug

Both `detect_tool_call_start_xml` (`lib/parsers/src/tool_calling/xml/parser.rs`, line 42) and `detect_tool_call_start_glm47` (`lib/parsers/src/tool_calling/xml/glm47_parser.rs`, line 30) call `chunk_ends_with_token_prefix()` for partial streaming match detection.

The `chunk_ends_with_token_prefix` function uses byte-based slicing internally. When the start token contains multibyte UTF-8 characters (common in CJK-based model formats like DeepSeek's `<｜tool▁call｜>`), the slice may land in the middle of a character, panicking with `byte index N is not a char boundary`.

### Steps to Reproduce

```rust
use dynamo_parsers::tool_calling::xml::detect_tool_call_start_xml;
use dynamo_parsers::XmlParserConfig;

// Config with multibyte UTF-8 start token
let config = XmlParserConfig {
    tool_call_start_token: "＜tool＞".to_string(), // fullwidth angle brackets (3 bytes each)
    tool_call_end_token: "＜/tool＞".to_string(),
    ..Default::default()
};

// Chunk whose suffix partially overlaps the token at a non-char-boundary
// The exact trigger depends on chunk size and token byte widths
let chunk = "some text＜"; // ends with first char of start token
let _ = detect_tool_call_start_xml(chunk, &config); // may panic
```

Currently latent because the default XML config uses ASCII tokens (`<tool_call>`), but would crash if a custom config used Unicode start tokens — which several model formats do.

### Expected Behavior

`detect_tool_call_start_xml` and `detect_tool_call_start_glm47` should handle multibyte UTF-8 tokens correctly without panicking.

### Actual Behavior

```
thread 'main' panicked at 'byte index N is not a char boundary'
```

### Suggested Fix

Use character-based iteration instead of byte slicing in `chunk_ends_with_token_prefix()`, or validate char boundaries before slicing.

### Environment

- dynamo: main branch
- Crate: `dynamo-parsers`
- Files:
  - `lib/parsers/src/tool_calling/xml/parser.rs`, line 42
  - `lib/parsers/src/tool_calling/xml/glm47_parser.rs`, line 30
