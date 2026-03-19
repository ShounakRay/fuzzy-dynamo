# Discovery: MiniMax Streaming vs One-Shot Reasoning Mismatch

## What's the bug?

In LLM inference serving, model output can be processed in two ways: **one-shot** (the entire response arrives at once) and **streaming** (the response arrives in small chunks). A fundamental requirement is that both modes produce identical results for the same input -- this property is called "streaming equivalence." If the same model output is interpreted differently depending on how it arrives, users get inconsistent behavior between batch and real-time modes.

The MiniMaxAppendThink reasoning parser violates this property. When processing the input `";\n"` in one-shot mode, it returns reasoning text `";"` -- stripping the trailing newline. In streaming mode, it returns `";\n"` -- preserving the newline. The root cause is that the one-shot code path delegates to `BasicReasoningParser`, which calls `.trim()` on the reasoning text as a post-processing step. The streaming code path emits text incrementally as each chunk arrives and has no equivalent trimming step at the end.

While a trailing newline might seem harmless, this class of inconsistency can cause real problems. Downstream systems that hash or compare reasoning output will see different values for the same logical response. Logging and debugging become confusing when the same model response looks different depending on the serving path. And if the mismatch exists for whitespace, it may also exist for more significant characters in edge cases that have not yet been discovered.

## When does this happen in real life?

This bug causes inconsistent behavior between streaming and non-streaming API modes:

- **Same prompt, different results** — a user sending the same request via the streaming API (`stream=true`) gets slightly different reasoning text than via the non-streaming API. Trailing newlines are preserved in streaming but stripped in non-streaming
- **Downstream processing breaks** — if an application parses the reasoning text for structured content (e.g., extracting step-by-step reasoning), the presence or absence of trailing newlines can change how the text is split or matched
- **Flaky tests** — integration tests that compare streaming and non-streaming output will intermittently fail depending on whether the model output ends with whitespace

While this seems minor, inconsistency between modes violates the principle that the API should produce identical results regardless of how the output is delivered. Applications that switch between modes (e.g., streaming for interactive users, non-streaming for batch processing) will see different behavior.

## How we found it

### The fuzzing approach

The fuzz target `fuzz_differential.rs` in `lib/parsers/fuzz` uses a **differential testing** strategy. It takes each fuzz-generated input string and processes it through both the one-shot and streaming paths of the same reasoning parser. The fuzzer controls which parser type is used (via a fuzz-controlled byte) and how the input is chunked for streaming (via a configurable chunk size). After both paths finish, it asserts that the reasoning text and normal text are identical.

Differential testing is powerful because it does not require knowing the "correct" answer -- it only requires that two implementations of the same specification agree with each other.

### What the fuzzer did

The fuzzer generated the short input `";\n"` and selected the MiniMaxAppendThink parser. It processed this input through one-shot mode (getting back `";"`) and through streaming mode (getting back `";\n"`). The assertion `oneshot_result.reasoning_text == stream_reasoning` failed, producing the crash artifact `crash-0a7d38b66474cdd18da409ae817d44c1fb8fba74`.

The fuzzer found several other inputs that trigger the same class of bug: `"\n\n\n\n"` (all-newline input), `"+\n:\n"` (mixed content with trailing newline), and `"\nH"` (leading newline stripped in one-shot but kept in streaming). All share the same root cause: whitespace handling differs between the two code paths.

### Why traditional testing missed this

The trailing newline `"\n"` is invisible in most test output and log messages. Existing unit tests used inputs without leading or trailing whitespace, so the trimming difference was never exposed.

## The fix

Either remove the `.trim()` call from the one-shot path (if streaming behavior is correct) or add a finalization step to the streaming path that trims trailing whitespace to match one-shot behavior.

## Fuzzing technique

**Strategy:** Differential (one-shot vs streaming)
**Target:** `fuzz_differential.rs`
**Crate:** `lib/parsers/fuzz`
**Run command:** `cd lib/parsers/fuzz && ~/.cargo/bin/cargo +nightly fuzz run fuzz_differential -- -max_total_time=60`
