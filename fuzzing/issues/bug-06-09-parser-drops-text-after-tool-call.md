### [BUG]: Pythonic and base JSON parsers silently drop text after tool calls

### Describe the Bug

Two tool call parsers only extract text **before** the first tool call, silently dropping any text that appears after:

**1. Pythonic parser** (`lib/parsers/src/tool_calling/pythonic/pythonic_parser.rs`, lines 182-187):

```rust
let normal_text = stripped
    .split(&matches[0])
    .next()    // only takes the FIRST part
    .unwrap()
    .trim()
    .to_string();
```

`split().next()` returns only text before the first match. Any text after the tool call is lost.

**2. Base JSON parser** (`lib/parsers/src/tool_calling/json/base_json_parser.rs`, lines 115-122):

```rust
fn try_parse_normal_text(input: &str, start_token: &str) -> String {
    if let Some(idx) = input.find(start_token) {
        return input[..idx].trim().to_string();
    }
    String::new()
}
```

Only text before the start token is returned via `input[..idx]`. Text after the tool call end token is lost.

### Steps to Reproduce

**Pythonic:**
```rust
use dynamo_parsers::try_tool_call_parse_pythonic;

let input = "Here is the call: get_weather(location=\"NYC\")\nDone!";
let (calls, normal_text) = try_tool_call_parse_pythonic(input, None).unwrap();
// normal_text = Some("Here is the call:") — "Done!" is lost
```

**Base JSON:**
```rust
use dynamo_parsers::try_tool_call_parse_json;

let input = "Let me help. <tool_call>{\"name\":\"f\",\"arguments\":{}}</tool_call> Here's the result.";
// normal_text only contains "Let me help." — "Here's the result." is lost
```

### Expected Behavior

Normal text both before and after tool calls should be preserved and returned.

### Actual Behavior

Text after tool calls is silently dropped. No error or warning.

### Suggested Fix

**Pythonic**: Collect all parts of the split and concatenate:
```rust
let parts: Vec<&str> = stripped.split(&matches[0]).collect();
let normal_text = parts.join("").trim().to_string();
```

**Base JSON**: Also extract text after the last end token and concatenate it.

### Environment

- dynamo: main branch
- Crate: `dynamo-parsers`
- Files:
  - `lib/parsers/src/tool_calling/pythonic/pythonic_parser.rs`, lines 182-187
  - `lib/parsers/src/tool_calling/json/base_json_parser.rs`, lines 115-122
