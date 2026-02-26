#![no_main]
use libfuzzer_sys::fuzz_target;
use dynamo_parsers::reasoning::ReasoningParser;
use dynamo_parsers_fuzz::select_parser_type;

// Fuzz streaming/incremental reasoning parsers.
// Splits input into random-sized chunks and feeds them sequentially,
// exercising the stateful incremental parsing path.
fuzz_target!(|data: &[u8]| {
    if data.len() < 2 { return; }
    let Ok(s) = std::str::from_utf8(&data[1..]) else { return };

    let parser_type = select_parser_type(data[0]);
    let mut parser = parser_type.get_reasoning_parser();

    // Split string into chunks at arbitrary positions using remaining bytes
    // to simulate streaming token-by-token input
    let mut pos = 0;
    let bytes = s.as_bytes();
    let mut chunk_seed = 1usize;
    while pos < bytes.len() {
        chunk_seed = chunk_seed.wrapping_mul(31).wrapping_add(7);
        let chunk_len = (chunk_seed % 16).max(1).min(bytes.len() - pos);
        // Find next valid UTF-8 boundary
        let end = (pos + chunk_len).min(bytes.len());
        let end = match std::str::from_utf8(&bytes[pos..end]) {
            Ok(_) => end,
            Err(e) => pos + e.valid_up_to(),
        };
        if end == pos { pos += 1; continue; }
        let chunk = unsafe { std::str::from_utf8_unchecked(&bytes[pos..end]) };
        let _ = parser.parse_reasoning_streaming_incremental(chunk, &[]);
        pos = end;
    }
});
