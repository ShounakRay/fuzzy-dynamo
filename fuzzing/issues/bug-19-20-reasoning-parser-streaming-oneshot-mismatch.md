### [BUG]: Reasoning parsers produce different output in streaming vs one-shot mode

### Describe the Bug

The reasoning parser's one-shot path (`detect_and_parse_reasoning`) and streaming path (`parse_reasoning_streaming_incremental`) produce different results for the same input. Two distinct issues:

**1. `.trim()` asymmetry (MiniMaxAppendThink and all parsers using `BasicReasoningParser`)**

The one-shot path applies `.trim()` to both outputs (`lib/parsers/src/reasoning/base_parser.rs`, lines 139-140):

```rust
let reasoning_text = reasoning_parts.join("").trim().to_string();
let normal_text = normal_parts.join("").trim().to_string();
```

The streaming path returns raw accumulated text without trimming (lines 253-254). This means whitespace-only reasoning like `"\n\n"` produces `""` in one-shot but `"\n\n"` in streaming.

**2. Prefix-matching content loss (Mistral, and potentially DeepseekR1, Step3, KimiK25)**

When `force_reasoning=true`, the streaming path buffers content that looks like a prefix of the think-start token (`base_parser.rs`, lines 185-191):

```rust
if !self.stripped_think_start
    && self._in_reasoning
    && !current_text.is_empty()
    && self.think_start_token.starts_with(current_text.as_str())
{
    break; // buffer the content, wait for more data
}
```

If a model emits `"["` as a standalone token, the streaming path sees that `"[THINK]".starts_with("[")` is true and buffers `"["` indefinitely. If the next token doesn't complete the tag, the buffered content is silently lost. The one-shot path has no such buffering — it correctly treats `"["` as reasoning content.

### Steps to Reproduce

Found via differential fuzzing:

```bash
cd lib/parsers
cargo +nightly fuzz run fuzz_differential -- -max_total_time=60
```

**Reproducer for trim asymmetry** (MiniMaxAppendThink, input `"\n\n"`):
```rust
use dynamo_parsers::reasoning::{ReasoningParser, ReasoningParserType};

let mut oneshot = ReasoningParserType::MiniMaxAppendThink.get_reasoning_parser();
let result = oneshot.detect_and_parse_reasoning("\n\n", &[]);
assert_eq!(result.reasoning_text, ""); // trimmed

let mut streaming = ReasoningParserType::MiniMaxAppendThink.get_reasoning_parser();
let result = streaming.parse_reasoning_streaming_incremental("\n\n", &[]);
assert_eq!(result.reasoning_text, "\n\n"); // NOT trimmed — mismatch
```

**Reproducer for prefix content loss** (Mistral, input `"["`):
```rust
let mut oneshot = ReasoningParserType::Mistral.get_reasoning_parser();
let result = oneshot.detect_and_parse_reasoning("[", &[]);
assert_eq!(result.reasoning_text, "["); // correct

let mut streaming = ReasoningParserType::Mistral.get_reasoning_parser();
let result = streaming.parse_reasoning_streaming_incremental("[", &[]);
assert_eq!(result.reasoning_text, ""); // content silently lost — mismatch
```

### Expected Behavior

One-shot and streaming parsing of the same input should produce identical `reasoning_text` and `normal_text`.

### Actual Behavior

1. **Trim**: One-shot trims whitespace, streaming doesn't → different results for whitespace-only reasoning
2. **Prefix**: Streaming buffers and loses content that matches a prefix of the think-start token → real content silently dropped

### Suggested Fix

**For trim asymmetry**: Remove `.trim()` from the one-shot path (lines 139-140 in `base_parser.rs`). This preserves raw content and matches streaming behavior. Alternatively, add trimming to the streaming path's final output, but that changes streaming behavior mid-stream.

**For prefix content loss**: When `force_reasoning=true`, the start token should be considered already consumed. Either:
- Skip the prefix-buffering check entirely when `force_reasoning=true`
- Set `stripped_think_start = true` at construction time when `force_reasoning=true`
- Require a minimum overlap threshold (like the `ol >= 2` check used elsewhere in the same function)

### Environment

- dynamo: main branch
- Crate: `dynamo-parsers`
- Files: `lib/parsers/src/reasoning/base_parser.rs` (lines 139-140, 185-191, 253-254)

### Additional Context

Found via differential fuzzing comparing streaming vs one-shot output for all 11 reasoning parser types. The fuzzer found both issues within seconds using 1-4 byte inputs.

Crash artifacts:
- `fuzz/artifacts/fuzz_differential/crash-3f3d2d8955322f325af6db2238355fa07007ebd9` (trim asymmetry)
- `fuzz/artifacts/fuzz_differential/crash-792ce6d99566f570120f2897290fc1e3d06f413d` (prefix content loss)

Related: #3393 (acknowledges parser "loss of tokens").
