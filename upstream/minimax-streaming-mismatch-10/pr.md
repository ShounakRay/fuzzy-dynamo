# fix: align MiniMaxAppendThink one-shot and streaming reasoning text output

#### Overview:

[ref: TBD -- file issue first]

Override `detect_and_parse_reasoning` in `MiniMaxAppendThinkParser` to preserve trailing and leading whitespace in reasoning text, matching the streaming path. The one-shot path previously delegated to `BasicReasoningParser` which calls `.trim()`, while streaming emits text incrementally without trimming, causing the same input to produce different output depending on the code path. Found via fuzzing with cargo-fuzz / libfuzzer.

#### Details:

Instead of returning the trimmed result from `BasicReasoningParser`, reconstruct the untrimmed reasoning and normal text by splitting the original input on `</think>`. If no `</think>` tag is present, all text is treated as reasoning. The streaming `parse_reasoning_streaming_incremental` method is unchanged.

#### Where should the reviewer start?

`lib/parsers/src/reasoning/minimax_append_think_parser.rs`

#### Related Issues: (use one of the action keywords Closes / Fixes / Resolves / Relates to)

- Fixes GitHub issue: TBD
