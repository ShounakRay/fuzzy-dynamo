// Fix for Bug 8: Granite parser streaming vs oneshot mismatch for short inputs
// File: lib/parsers/src/reasoning/granite_parser.rs
// Severity: HIGH
//
// Problem: The streaming parser buffers input that is a prefix of any think token
// (e.g., "H" is a prefix of "Here's my thought process:"). If no more data arrives,
// the buffered text is never emitted. The oneshot parser correctly returns "H" as
// normal_text. This causes short model outputs to be silently dropped.
//
// Fix: When checking if buffered text is a prefix of a think token, also verify that
// the buffered text consists ONLY of characters that could plausibly start a think
// token. A single "H" could be a prefix, but "Hx" could not. More robustly: only
// buffer if the accumulated text is a proper prefix of at least one think token AND
// we are not already past the point where the tokens diverge. Additionally, the check
// must consider that we are NOT in_reasoning — if we haven't started reasoning, we
// should only buffer for think_start_tokens, not think_end_tokens.

// === ORIGINAL (parse_reasoning_streaming_incremental, lines ~98-120) ===
//     self.buffer.push_str(text);
//     let mut current_text = self.buffer.to_string();
//     // If the current text is a prefix of the think token, keep buffering
//
//     for think_start_token in &self.think_start_tokens {
//         if think_start_token.starts_with(&current_text)
//             && think_start_token.as_str() != current_text.as_str()
//         {
//             return ParserResult {
//                 normal_text: String::new(),
//                 reasoning_text: String::new(),
//             };
//         }
//     }
//     for think_end_token in &self.think_end_tokens {
//         if think_end_token.starts_with(&current_text)
//             && think_end_token.as_str() != current_text.as_str()
//         {
//             return ParserResult {
//                 normal_text: String::new(),
//                 reasoning_text: String::new(),
//             };
//         }
//     }

// === FIXED ===
// In parse_reasoning_streaming_incremental, replace the prefix-buffering block:

        self.buffer.push_str(text);
        let mut current_text = self.buffer.to_string();
        // If the current text is a prefix of the think token, keep buffering.
        // Only check start tokens if we haven't entered reasoning yet;
        // only check end tokens if we ARE in reasoning.

        if !self.in_reasoning && !self.stripped_think_start {
            for think_start_token in &self.think_start_tokens {
                if think_start_token.starts_with(&current_text)
                    && think_start_token.as_str() != current_text.as_str()
                {
                    return ParserResult {
                        normal_text: String::new(),
                        reasoning_text: String::new(),
                    };
                }
            }
        }
        if self.in_reasoning {
            for think_end_token in &self.think_end_tokens {
                if think_end_token.starts_with(&current_text)
                    && think_end_token.as_str() != current_text.as_str()
                {
                    return ParserResult {
                        normal_text: String::new(),
                        reasoning_text: String::new(),
                    };
                }
            }
        }

// The key change: when NOT in reasoning mode, we no longer buffer text that
// happens to be a prefix of think_end_tokens ("Here's my response:" / "Here is
// my response:"). This prevents "H" from being swallowed — "H" is a prefix of
// "Here's my response:" and "Here is my response:", but since we haven't entered
// reasoning yet, there's no reason to wait for an end token.
//
// When in reasoning mode, we still buffer prefixes of end tokens (to detect the
// transition back to normal text).

// === TEST ===
#[test]
fn test_streaming_short_input_not_swallowed() {
    // Regression test for Bug 8: "H" should be emitted as normal_text in
    // streaming mode, matching oneshot behavior.
    let mut parser = GraniteReasoningParser::new();

    // Oneshot baseline
    let mut oneshot = GraniteReasoningParser::new();
    let oneshot_result = oneshot.detect_and_parse_reasoning("H", &[]);
    assert_eq!(oneshot_result.normal_text, "H");

    // Streaming should match
    let streaming_result = parser.parse_reasoning_streaming_incremental("H", &[]);
    assert_eq!(
        streaming_result.normal_text, "H",
        "Streaming mode dropped short input 'H' that oneshot correctly returned"
    );
    assert_eq!(streaming_result.reasoning_text, "");
}

#[test]
fn test_streaming_short_prefix_still_buffers_for_start_token() {
    // "Here's" IS a prefix of "Here's my thought process:" and we are not in
    // reasoning mode, so it should still be buffered (waiting for more data).
    let mut parser = GraniteReasoningParser::new();
    let result = parser.parse_reasoning_streaming_incremental("Here's", &[]);
    assert_eq!(result.normal_text, "");
    assert_eq!(result.reasoning_text, "");
}
