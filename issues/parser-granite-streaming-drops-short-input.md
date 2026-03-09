# Granite parser: streaming mode drops single-character input

## Summary

The `Granite` reasoning parser drops all normal text when processing short inputs (e.g., single character `"H"`) in streaming mode, while one-shot mode correctly returns the text. This means short model outputs may be silently lost in streaming inference.

## Severity

**High** — Complete data loss. Short model outputs (single characters, short tokens) are silently dropped in streaming mode. This could manifest as missing tokens in streamed responses, particularly at the start of generation or with small chunk sizes.

## Steps to Reproduce

```rust
use dynamo_parsers::reasoning::ReasoningParser;
use dynamo_parsers::ParserType;

let input = "H";

// One-shot: correct
let mut oneshot = ParserType::Granite.get_reasoning_parser();
let oneshot_result = oneshot.detect_and_parse_reasoning(input, &[]);
assert_eq!(oneshot_result.normal_text, "H"); // Correct

// Streaming: WRONG — drops the text entirely
let mut streaming = ParserType::Granite.get_reasoning_parser();
let r = streaming.parse_reasoning_streaming_incremental("H", &[]);
assert_eq!(r.normal_text, ""); // Bug: should be "H"
```

## Root Cause

The Granite streaming parser likely buffers input looking for reasoning markers (e.g., `<|thinking|>`). When the input is shorter than the marker prefix, the parser holds it in a buffer waiting for more data. If no more data arrives, the buffered text is never flushed as normal output.

## Additional Crash Inputs

- `crash-5bf04a282290f266bdaa7e8b929cc3a33f4dc141` — input `"H"` (Granite, cs=3)
- `crash-b9ede57e0851fac41d4ccdf2a4f9db0ab301a461` — input `"H"` (Granite, cs=2)

## Suggested Fix

The streaming parser needs a finalization/flush mechanism. When the stream ends, any buffered text that doesn't match a reasoning marker prefix should be emitted as normal text.

Alternatively, if the parser is meant to be incremental only (always expecting more data), then the one-shot mode should match this behavior by using the same buffering logic.

Found by: `fuzz_differential` fuzzer.
