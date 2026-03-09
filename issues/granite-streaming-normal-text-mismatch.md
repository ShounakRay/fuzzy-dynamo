# Bug 8: Granite Parser: Streaming vs Oneshot Normal Text Mismatch

## Summary

The Granite reasoning parser produces different `normal_text` output between
oneshot (`detect_and_parse_reasoning`) and streaming
(`parse_reasoning_streaming_incremental`) modes. Short inputs (e.g., single
character `"H"`) are silently dropped in streaming mode while oneshot returns
them correctly. This means short model outputs may be lost in streaming inference.

## Severity

**Medium** — Causes data loss in streaming mode for short inputs that don't
contain reasoning markers. Could manifest as missing tokens in streamed
responses, particularly at the start of generation or with small chunk sizes.

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

The streaming parser (`granite_parser.rs:99-107`) buffers input waiting for
potential reasoning tags (e.g., `<|thinking|>`) before emitting normal text.
When the input is shorter than the marker prefix, the parser holds it in a
buffer waiting for more data. If no more data arrives, the buffered text is
never flushed as normal output. The oneshot path processes the entire input
at once and correctly returns it as normal text.

## Crash Artifacts

- `fuzz_differential/crash-b9ede57e0851fac41d4ccdf2a4f9db0ab301a461` — input `"H"` (cs=2)
- `fuzz_differential/crash-5bf04a282290f266bdaa7e8b929cc3a33f4dc141` — input `"H"` (cs=3)

## Suggested Fix

The streaming parser needs a finalization/flush mechanism. When the stream ends,
any buffered text that doesn't match a reasoning marker prefix should be emitted
as normal text. Alternatively, provide an explicit `flush()` or `finish()` method
that callers invoke after the last chunk to drain remaining buffered text.

Found by: `fuzz_differential` fuzzer.
