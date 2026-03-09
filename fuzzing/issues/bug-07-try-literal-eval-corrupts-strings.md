### [BUG]: `try_literal_eval` corrupts string values containing `True`, `False`, or `None` as substrings

### Describe the Bug

The `try_literal_eval` function in `lib/parsers/src/tool_calling/xml/parser.rs` (lines 491-495) performs global string replacements to convert Python literals to JSON:

```rust
let normalized = s
    .replace('\'', "\"")
    .replace("True", "true")
    .replace("False", "false")
    .replace("None", "null");
```

These replacements are not context-aware — they modify text inside JSON string values, not just bare Python literals. This corrupts any string containing these keywords as substrings:

- `"TrueNorth"` → `"trueNorth"`
- `"Falsehood"` → `"falsehood"`
- `"NonEmpty"` → `"nullEmpty"`
- `"Valentine"` → `"Valentinull"` (contains `None` backwards — wait, no, but `"ArcherNone"` → `"Archernull"`)

### Steps to Reproduce

```rust
// XML tool call with a string value containing "True" as substring:
let input = r#"<tool_call>{"name":"search","arguments":{"query":"TrueNorth navigation"}}</tool_call>"#;
let (calls, _) = try_tool_call_parse_xml(input, &XmlParserConfig::default(), None).unwrap();
// calls[0].function.arguments contains "trueNorth navigation" instead of "TrueNorth navigation"
```

### Expected Behavior

String values inside JSON should not be modified. Only bare Python literals (`True`, `False`, `None`) outside of quoted strings should be converted.

### Actual Behavior

All occurrences of `True`, `False`, and `None` are replaced globally, including inside quoted JSON string values.

### Suggested Fix

Use word-boundary-aware replacement (e.g., regex `\bTrue\b` → `true`), or better yet, parse the JSON structure first and only convert bare Python literals that appear as values (not inside strings).

### Environment

- dynamo: main branch
- Crate: `dynamo-parsers`
- File: `lib/parsers/src/tool_calling/xml/parser.rs`, lines 491-495
