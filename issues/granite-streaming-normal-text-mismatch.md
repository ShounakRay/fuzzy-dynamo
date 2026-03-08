# Granite Parser: Streaming vs Oneshot Normal Text Mismatch

## Summary

The Granite reasoning parser produces different `normal_text` output between
oneshot (`detect_and_parse_reasoning`) and streaming
(`parse_reasoning_streaming_incremental`) modes. For the single-character input
`"H"`, oneshot returns `normal_text = "H"` while streaming returns
`normal_text = ""`.

## Steps to Reproduce

### Via fuzzing

```bash
cd lib/parsers/fuzz
~/.cargo/bin/cargo +nightly fuzz run fuzz_differential \
  artifacts/fuzz_differential/crash-b9ede57e0851fac41d4ccdf2a4f9db0ab301a461
```

### Minimal Rust code

```rust
use dynamo_parsers::reasoning::{ReasoningParser, ReasoningParserType};

let mut oneshot = ReasoningParserType::Granite.get_reasoning_parser();
let oneshot_result = oneshot.detect_and_parse_reasoning("H", &[]);
assert_eq!(oneshot_result.normal_text, "H"); // passes

let mut streaming = ReasoningParserType::Granite.get_reasoning_parser();
let r = streaming.parse_reasoning_streaming_incremental("H", &[]);
assert_eq!(r.normal_text, "H"); // FAILS: returns ""
```

## Root Cause

The streaming parser likely buffers input waiting for potential reasoning tags
before emitting normal text. When the input is very short (1 character), the
buffered text is never flushed because no "end of stream" signal is sent after
the single chunk. The oneshot path processes the entire input at once and
correctly returns it as normal text.

## Impact

- **Severity**: Medium — causes data loss in streaming mode for short inputs
  that don't contain reasoning markers
- **Affected parser**: `ReasoningParserType::Granite`
- **Workaround**: None currently; callers relying on streaming mode may miss
  trailing normal text

## Suggested Fix

Ensure the streaming parser flushes its internal buffer when no reasoning
markers are detected in the input prefix. Alternatively, provide an explicit
`flush()` or `finish()` method that callers invoke after the last chunk to
drain remaining buffered text.
