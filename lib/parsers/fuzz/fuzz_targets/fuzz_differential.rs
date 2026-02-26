#![no_main]
use libfuzzer_sys::fuzz_target;
use dynamo_parsers::reasoning::{ReasoningParser, ReasoningParserType};

// Differential fuzz harness: compare streaming vs one-shot reasoning parsing.
//
// Feed the same input to detect_and_parse_reasoning (one-shot) and
// parse_reasoning_streaming_incremental (chunked), then assert they produce
// identical reasoning_text and normal_text.
//
// The streaming path has complex stateful buffer management in
// BasicReasoningParser that can diverge from the simpler one-shot path
// on edge cases (partial tags, UTF-8 boundaries, force_reasoning).
fuzz_target!(|data: &[u8]| {
    if data.len() < 3 { return; }
    let Ok(s) = std::str::from_utf8(&data[2..]) else { return };
    if s.is_empty() { return; }

    // Use first byte to select parser type
    let types = [
        ReasoningParserType::DeepseekR1,
        ReasoningParserType::Basic,
        ReasoningParserType::GptOss,
        ReasoningParserType::Qwen,
        ReasoningParserType::NemotronDeci,
        ReasoningParserType::Kimi,
        ReasoningParserType::KimiK25,
        ReasoningParserType::Step3,
        ReasoningParserType::Mistral,
        ReasoningParserType::Granite,
        ReasoningParserType::MiniMaxAppendThink,
    ];
    let parser_type = types[data[0] as usize % types.len()];

    // One-shot parse
    let mut oneshot_parser = parser_type.get_reasoning_parser();
    let oneshot_result = oneshot_parser.detect_and_parse_reasoning(s, &[]);

    // Streaming parse: use second byte to control chunk sizing strategy
    let mut streaming_parser = parser_type.get_reasoning_parser();
    let mut streaming_reasoning = String::new();
    let mut streaming_normal = String::new();

    let bytes = s.as_bytes();
    let mut pos = 0;
    let chunk_strategy = data[1] % 4;

    while pos < bytes.len() {
        let chunk_len = match chunk_strategy {
            0 => 1, // byte-by-byte (most aggressive)
            1 => {
                // random-ish sizes 1-16
                let seed = pos.wrapping_mul(31).wrapping_add(data[1] as usize);
                (seed % 16).max(1)
            }
            2 => {
                // medium chunks 4-32
                let seed = pos.wrapping_mul(37).wrapping_add(data[1] as usize);
                ((seed % 29) + 4).min(bytes.len() - pos)
            }
            _ => bytes.len() - pos, // whole input at once (should trivially match)
        };

        let mut end = (pos + chunk_len).min(bytes.len());

        // Extend chunk to include complete UTF-8 characters.
        // Unlike the crash-only streaming harness which can skip bytes,
        // the differential harness must feed the *exact same* bytes to
        // both paths — so we extend (never skip) to hit a char boundary.
        while end < bytes.len() && (bytes[end] & 0xC0) == 0x80 {
            // bytes[end] is a continuation byte — extend to complete the character
            end += 1;
        }

        if end == pos {
            // Shouldn't happen with valid UTF-8 input, but guard anyway
            break;
        }

        let chunk = unsafe { std::str::from_utf8_unchecked(&bytes[pos..end]) };
        let result = streaming_parser.parse_reasoning_streaming_incremental(chunk, &[]);
        streaming_reasoning.push_str(&result.reasoning_text);
        streaming_normal.push_str(&result.normal_text);
        pos = end;
    }

    // Assert equivalence between one-shot and streaming results.
    // Both paths should produce the same split of reasoning vs normal text.
    assert_eq!(
        oneshot_result.reasoning_text, streaming_reasoning,
        "Reasoning text mismatch for {:?} (chunk_strategy={}).\n\
         Input ({} bytes): {:?}\n\
         One-shot reasoning: {:?}\n\
         Streaming reasoning: {:?}",
        parser_type, chunk_strategy,
        s.len(), truncate(s, 200),
        truncate(&oneshot_result.reasoning_text, 200),
        truncate(&streaming_reasoning, 200),
    );

    assert_eq!(
        oneshot_result.normal_text, streaming_normal,
        "Normal text mismatch for {:?} (chunk_strategy={}).\n\
         Input ({} bytes): {:?}\n\
         One-shot normal: {:?}\n\
         Streaming normal: {:?}",
        parser_type, chunk_strategy,
        s.len(), truncate(s, 200),
        truncate(&oneshot_result.normal_text, 200),
        truncate(&streaming_normal, 200),
    );
});

fn truncate(s: &str, max_len: usize) -> &str {
    if s.len() <= max_len {
        s
    } else {
        &s[..s.floor_char_boundary(max_len)]
    }
}
