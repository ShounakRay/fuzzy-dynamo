### [BUG]: DeepSeek V3 parser destroys newlines in tool call arguments during JSON normalization

### What This Bug Is (Plain English)

When an AI model returns a tool call with a code snippet as an argument (like "run this Python code"), the code naturally contains newlines. The DeepSeek V3 parser has a fallback path that tries to fix malformed JSON by joining all the lines together with spaces. This destroys every newline in the process.

So if the model says "run this code: `def hello():\n    print('hi')`", the parser turns it into `def hello(): print('hi')` — which is broken Python. The code argument gets mangled, and the tool receives something that doesn't work.

### Describe the Bug

The DeepSeek V3 parser in `lib/parsers/src/tool_calling/json/deepseek_v3_parser.rs` has a JSON normalization fallback (lines 115-119) that joins all lines with spaces, destroying intentional newlines in string values:

```rust
let normalized = args_str
    .lines()
    .map(|line| line.trim_start())
    .collect::<Vec<_>>()
    .join(" ");
```

This triggers when the initial `serde_json::from_str` fails (e.g., the model output has leading-whitespace indentation). The fallback strips all newlines, corrupting any string value that contains intentional newlines (code, multi-line text, etc.).

### Steps to Reproduce

```rust
// Tool call with code argument containing newlines:
let input = concat!(
    "<｜tool▁calls▁begin｜>",
    "<｜tool▁call▁begin｜>function<｜tool▁sep｜>run_code\n",
    "```json\n",
    "  {\n",
    "    \"code\": \"def f():\\n    pass\"\n",
    "  }\n",
    "```",
    "<｜tool▁call▁end｜>",
    "<｜tool▁calls▁end｜>"
);

// After normalization, the arguments string has newlines replaced with spaces:
// "def f():\\n    pass" becomes "def f(): pass" or similar corruption
```

### Expected Behavior

Newlines inside JSON string values should be preserved during normalization. Only structural whitespace (indentation) should be normalized.

### Actual Behavior

All newlines in the arguments string are replaced with spaces, corrupting string values that contain intentional newlines.

### Suggested Fix

Only strip leading whitespace for indentation normalization, but join with `"\n"` instead of `" "`:

```rust
let normalized = args_str
    .lines()
    .map(|line| line.trim_start())
    .collect::<Vec<_>>()
    .join("\n");
```

Or better yet, only normalize structural whitespace outside of JSON string values.

### Additional Context

**Note**: This normalization is a deliberate recovery path for malformed JSON (e.g., model output with Python-style indentation). The newline destruction is a trade-off — joining with `"\n"` instead of `" "` would preserve string newlines but could break the JSON fix for certain malformed inputs. Whether this warrants changing depends on how frequently models emit code-containing arguments in malformed JSON format.

### Environment

- dynamo: main branch
- Crate: `dynamo-parsers`
- File: `lib/parsers/src/tool_calling/json/deepseek_v3_parser.rs`, lines 115-119
