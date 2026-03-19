// Fix for Bug 10: MiniMaxAppendThink streaming vs one-shot reasoning mismatch on trailing newlines
// File: lib/parsers/src/reasoning/minimax_append_think_parser.rs
// Severity: MEDIUM
//
// Problem: One-shot detect_and_parse_reasoning delegates to BasicReasoningParser which
//   calls .trim() on reasoning_text (base_parser.rs:153), stripping trailing newlines.
//   Streaming mode emits text incrementally without trimming, causing a mismatch.
// Fix: Override detect_and_parse_reasoning in MiniMaxAppendThinkParser to NOT trim
//   the reasoning text, matching the streaming behavior. The trim in BasicReasoningParser
//   is intentional for other parsers but wrong for MiniMax differential consistency.

// === ORIGINAL (lines 53-57) ===
// impl ReasoningParser for MiniMaxAppendThinkParser {
//     fn detect_and_parse_reasoning(&mut self, text: &str, token_ids: &[u32]) -> ParserResult {
//         // Prepend <think> and delegate to the inner parser
//         let augmented = format!("<think>{}", text);
//         self.inner.detect_and_parse_reasoning(&augmented, token_ids)
//     }

// === FIXED ===
impl ReasoningParser for MiniMaxAppendThinkParser {
    fn detect_and_parse_reasoning(&mut self, text: &str, token_ids: &[u32]) -> ParserResult {
        // Prepend <think> and delegate to the inner parser
        let augmented = format!("<think>{}", text);
        let mut result = self.inner.detect_and_parse_reasoning(&augmented, token_ids);

        // BasicReasoningParser.detect_and_parse_reasoning trims reasoning_text and
        // normal_text. Streaming mode does NOT trim. To maintain one-shot/streaming
        // equivalence, reconstruct the untrimmed reasoning from the original input.
        //
        // If there's no </think> in the input, all text is reasoning (untrimmed).
        // If there is a </think>, split on it: reasoning = before, normal = after.
        if let Some(end_pos) = text.find("</think>") {
            result.reasoning_text = text[..end_pos].to_string();
            result.normal_text = text[end_pos + "</think>".len()..].to_string();
        } else {
            result.reasoning_text = text.to_string();
            result.normal_text = String::new();
        }

        result
    }

    // parse_reasoning_streaming_incremental unchanged
    fn parse_reasoning_streaming_incremental(
        &mut self,
        text: &str,
        token_ids: &[u32],
    ) -> ParserResult {
        if self.is_first_chunk {
            self.is_first_chunk = false;
            let augmented = format!("<think>{}", text);
            self.inner
                .parse_reasoning_streaming_incremental(&augmented, token_ids)
        } else {
            self.inner
                .parse_reasoning_streaming_incremental(text, token_ids)
        }
    }
}

// === TEST ===
#[test]
fn test_minimax_oneshot_streaming_trailing_newline_consistency() {
    // One-shot
    let mut oneshot = MiniMaxAppendThinkParser::new();
    let oneshot_result = oneshot.detect_and_parse_reasoning(";\n", &[]);

    // Streaming (char-by-char)
    let mut streaming = MiniMaxAppendThinkParser::new();
    let r1 = streaming.parse_reasoning_streaming_incremental(";", &[]);
    let r2 = streaming.parse_reasoning_streaming_incremental("\n", &[]);
    let stream_reasoning = format!("{}{}", r1.reasoning_text, r2.reasoning_text);

    // Both must produce identical reasoning text
    assert_eq!(oneshot_result.reasoning_text, stream_reasoning,
        "one-shot and streaming must agree: oneshot={:?}, streaming={:?}",
        oneshot_result.reasoning_text, stream_reasoning);
}

#[test]
fn test_minimax_oneshot_streaming_leading_newline_consistency() {
    let mut oneshot = MiniMaxAppendThinkParser::new();
    let oneshot_result = oneshot.detect_and_parse_reasoning("\nH", &[]);

    let mut streaming = MiniMaxAppendThinkParser::new();
    let r1 = streaming.parse_reasoning_streaming_incremental("\n", &[]);
    let r2 = streaming.parse_reasoning_streaming_incremental("H", &[]);
    let stream_reasoning = format!("{}{}", r1.reasoning_text, r2.reasoning_text);

    assert_eq!(oneshot_result.reasoning_text, stream_reasoning);
}

#[test]
fn test_minimax_oneshot_with_end_token_no_trim() {
    let mut parser = MiniMaxAppendThinkParser::new();
    let result = parser.detect_and_parse_reasoning("reasoning\n</think>response\n", &[]);
    // reasoning should NOT be trimmed
    assert_eq!(result.reasoning_text, "reasoning\n");
    assert_eq!(result.normal_text, "response\n");
}
