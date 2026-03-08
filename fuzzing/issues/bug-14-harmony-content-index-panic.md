### [BUG]: Harmony parser panics on empty `content` vector in analysis channel

### What This Bug Is (Plain English)

The Harmony parser processes messages that come through different "channels" (like analysis and commentary). When it gets an analysis channel message, it grabs the first item from the message's content list using `content[0]`. But it never checks if the list is empty first.

If an analysis message arrives with no content (an empty list), the code tries to access the first element of nothing, and crashes. Ironically, the code for the commentary channel right above it handles this correctly by checking with `.first()` — the analysis branch just forgot to do the same thing.

### Describe the Bug

The harmony tool call parser in `lib/parsers/src/tool_calling/harmony/harmony_parser.rs` (line 123) uses direct indexing `message.content[0]` without checking if the content vector is empty:

```rust
} else if channel == Some("analysis") {
    normal_text.push_str(match &message.content[0] {  // panics if empty
        Text(t) => &t.text,
        _ => "",
    });
}
```

If an analysis channel message has an empty content vector, this panics with an index-out-of-bounds error.

The commentary branch (line 97) in the same file correctly uses `.first()` with a match, but the analysis branch does not.

### Steps to Reproduce

This requires a harmony-formatted response where an analysis channel message has an empty `content` array:

```json
{"channel": "analysis", "content": []}
```

Currently latent because the harmony tokenizer always produces non-empty content, but this is a crash risk if the upstream format changes or if the parser is used with a different tokenizer.

### Expected Behavior

Empty content vectors should be handled gracefully, the same way the commentary branch handles them.

### Actual Behavior

```
thread 'main' panicked at 'index out of bounds: the len is 0 but the index is 0'
```

### Suggested Fix

Use `.first()` instead of `[0]`, matching the pattern used in the commentary branch:

```rust
} else if channel == Some("analysis") {
    if let Some(Text(t)) = message.content.first() {
        normal_text.push_str(&t.text);
    }
}
```

### Environment

- dynamo: main branch
- Crate: `dynamo-parsers`
- File: `lib/parsers/src/tool_calling/harmony/harmony_parser.rs`, line 123
