# Bug 10: MiniMaxAppendThink parser: streaming vs one-shot reasoning mismatch on trailing newlines

## Summary

The `MiniMaxAppendThink` reasoning parser produces different reasoning text when processing input in one-shot vs streaming mode. Specifically, one-shot mode strips trailing newlines from reasoning text while streaming mode preserves them.

## Severity

**Medium** — Inconsistency between one-shot and streaming mode can cause downstream logic to behave differently depending on how input is received. In LLM inference serving, this means the same model output may be processed differently depending on whether it arrives in one chunk or multiple.

## Steps to Reproduce

```rust
use dynamo_parsers::reasoning::ReasoningParser;
use dynamo_parsers::ParserType;

let input = ";\n";

// One-shot
let mut oneshot = ParserType::MiniMaxAppendThink.get_reasoning_parser();
let oneshot_result = oneshot.detect_and_parse_reasoning(input, &[]);
assert_eq!(oneshot_result.reasoning_text, ";"); // Strips trailing \n

// Streaming (char-by-char)
let mut streaming = ParserType::MiniMaxAppendThink.get_reasoning_parser();
let r1 = streaming.parse_reasoning_streaming_incremental(";", &[]);
let r2 = streaming.parse_reasoning_streaming_incremental("\n", &[]);
let stream_reasoning = format!("{}{}", r1.reasoning_text, r2.reasoning_text);
assert_eq!(stream_reasoning, ";\n"); // Preserves trailing \n

// MISMATCH: one-shot gives ";" but streaming gives ";\n"
```

## Root Cause

The one-shot `detect_and_parse_reasoning` method likely trims or strips trailing whitespace from reasoning text as a post-processing step, while the streaming `parse_reasoning_streaming_incremental` method emits text incrementally without such trimming.

## Additional Crash Inputs

All reproduce the same MiniMaxAppendThink trailing-newline issue:
- `crash-0a7d38b66474cdd18da409ae817d44c1fb8fba74` — input `";\n"`
- `crash-3f3d2d8955322f325af6db2238355fa07007ebd9` — input `"\n\n\n\n"`
- `crash-c94a19860d21a1bf9b45bfa7f279e9c3bce017d2` — input `"+\n:\n"`
- `crash-44436640b12392d87c7b96a66d6afac743db368f` — input `"\nH"` (leading newline stripped in one-shot, kept in streaming)

## Suggested Fix

Either:
1. Make one-shot mode NOT strip trailing newlines (if streaming behavior is correct), or
2. Make streaming mode strip trailing newlines at finalization (if one-shot behavior is correct)

The key requirement is that `detect_and_parse_reasoning(input)` must produce the same output as feeding `input` through `parse_reasoning_streaming_incremental` in arbitrary chunks.

Found by: `fuzz_differential` fuzzer.
