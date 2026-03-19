# [BUG]: Granite streaming parser silently drops short normal text inputs

### Describe the Bug

The Granite reasoning parser in `lib/parsers/src/reasoning/granite_parser.rs` produces different `normal_text` output between oneshot (`detect_and_parse_reasoning`) and streaming (`parse_reasoning_streaming_incremental`) modes. Short inputs (e.g., single character `"H"`) are silently dropped in streaming mode while oneshot returns them correctly. The streaming parser buffers input waiting for potential reasoning tags (e.g., `<|thinking|>`) before emitting normal text. When the input is shorter than the marker prefix, the parser holds it in a buffer waiting for more data. If no more data arrives, the buffered text is never flushed as normal output.

### Steps to Reproduce

Via fuzzing:

```bash
cd lib/parsers/fuzz
~/.cargo/bin/cargo +nightly fuzz run fuzz_differential \
  artifacts/fuzz_differential/crash-b9ede57e0851fac41d4ccdf2a4f9db0ab301a461
```

Minimal Rust code:

```rust
use dynamo_parsers::reasoning::{ReasoningParser, ReasoningParserType};

let mut oneshot = ReasoningParserType::Granite.get_reasoning_parser();
let oneshot_result = oneshot.detect_and_parse_reasoning("H", &[]);
assert_eq!(oneshot_result.normal_text, "H"); // passes

let mut streaming = ReasoningParserType::Granite.get_reasoning_parser();
let r = streaming.parse_reasoning_streaming_incremental("H", &[]);
assert_eq!(r.normal_text, "H"); // FAILS: returns ""
```

### Expected Behavior

Streaming mode should produce the same `normal_text` output as oneshot mode. For input `"H"`, both should return `normal_text = "H"`.

### Actual Behavior

Oneshot returns `normal_text = "H"` correctly. Streaming returns `normal_text = ""` — the input is buffered and never flushed, causing data loss.

### Environment

On commit `57e6a79f5` in `dynamo-runtime` @ `lib/parsers/src/reasoning/granite_parser.rs` (lines 99-107)

### Additional Context

This means short model outputs may be lost in streaming inference, particularly at the start of generation or with small chunk sizes. The streaming parser needs a finalization/flush mechanism: when the stream ends, any buffered text that doesn't match a reasoning marker prefix should be emitted as normal text. Alternatively, an explicit `flush()` or `finish()` method could be provided for callers to invoke after the last chunk.

Found via fuzzing with cargo-fuzz / libfuzzer.

### Screenshots

_No response_
