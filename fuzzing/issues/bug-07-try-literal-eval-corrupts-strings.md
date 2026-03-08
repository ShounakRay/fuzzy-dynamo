### [BUG]: `try_literal_eval` corrupts string values containing `True`, `False`, or `None` as substrings

### What This Bug Is (Plain English)

Some AI models output Python-style values like `True`, `False`, and `None` instead of JSON's `true`, `false`, and `null`. The parser has a converter that does a find-and-replace to fix this. But the replacement is too aggressive — it replaces those words *everywhere*, including inside text that just happens to contain them.

So if a tool call argument is `"Search for TrueNorth Navigation company"`, the parser corrupts it to `"Search for trueNorth Navigation company"`. The word `"None"` inside `"NonEmpty"` becomes `"nullEmpty"`. User data gets silently mangled.

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
