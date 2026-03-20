# fix: only buffer relevant token prefixes in Granite streaming parser

#### Overview:

[ref: TBD — file issue first]

Guard the prefix-buffering logic in `parse_reasoning_streaming_incremental` so that think-start tokens are only checked when not yet in reasoning mode, and think-end tokens are only checked when in reasoning mode. Previously, text like `"H"` was buffered indefinitely because it matched a prefix of both start and end tokens regardless of parser state, causing short normal-text outputs to be silently dropped. Found via fuzzing with cargo-fuzz / libfuzzer.

#### Details:

Wrap the existing `think_start_tokens` prefix check in `if !self.in_reasoning && !self.stripped_think_start` and the `think_end_tokens` prefix check in `if self.in_reasoning`. When not in reasoning mode, there is no reason to buffer text that is a prefix of an end token (e.g., `"Here's my response:"`), so `"H"` is correctly emitted as `normal_text` instead of being held in the buffer. When in reasoning mode, end-token prefix buffering is still active to detect the transition back to normal text.

#### Where should the reviewer start?

`lib/parsers/src/reasoning/granite_parser.rs` — the `parse_reasoning_streaming_incremental` method

#### Related Issues: (use one of the action keywords Closes / Fixes / Resolves / Relates to)

- Fixes GitHub issue: TBD
