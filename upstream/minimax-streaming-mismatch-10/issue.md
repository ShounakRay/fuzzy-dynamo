# [BUG]: MiniMaxAppendThink parser streaming vs one-shot reasoning mismatch on trailing newlines

### Describe the Bug

The `MiniMaxAppendThink` reasoning parser produces different reasoning text when processing input in one-shot vs streaming mode. Specifically, one-shot mode strips trailing newlines from reasoning text while streaming mode preserves them. In LLM inference serving, this means the same model output may be processed differently depending on whether it arrives in one chunk or multiple.

The one-shot `detect_and_parse_reasoning` method trims trailing whitespace from reasoning text as a post-processing step, while the streaming `parse_reasoning_streaming_incremental` method emits text incrementally without such trimming.

### Steps to Reproduce

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

### Expected Behavior

One-shot and streaming modes should produce identical reasoning text for the same input. `detect_and_parse_reasoning(input)` must produce the same output as feeding `input` through `parse_reasoning_streaming_incremental` in arbitrary chunks.

### Actual Behavior

One-shot mode returns `";"` (trailing newline stripped) while streaming mode returns `";\n"` (trailing newline preserved).

### Environment

On commit `57e6a79f5` in `dynamo-runtime` @ `lib/parsers/src/reasoning/`

### Additional Context

Multiple fuzz crash inputs reproduce the same issue:
- `crash-0a7d38b66474cdd18da409ae817d44c1fb8fba74` — input `";\n"`
- `crash-3f3d2d8955322f325af6db2238355fa07007ebd9` — input `"\n\n\n\n"`
- `crash-c94a19860d21a1bf9b45bfa7f279e9c3bce017d2` — input `"+\n:\n"`
- `crash-44436640b12392d87c7b96a66d6afac743db368f` — input `"\nH"` (leading newline stripped in one-shot, kept in streaming)

A fix would be to either make one-shot mode not strip trailing newlines (if streaming behavior is correct), or make streaming mode strip trailing newlines at finalization (if one-shot behavior is correct).

Found via fuzzing with cargo-fuzz / libfuzzer.

### Screenshots

_No response_
