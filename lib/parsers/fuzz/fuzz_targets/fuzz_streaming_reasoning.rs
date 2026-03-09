#![no_main]
use libfuzzer_sys::fuzz_target;
use dynamo_parsers::reasoning::ReasoningParser;
use dynamo_parsers_fuzz::select_parser_type;

fuzz_target!(|data: &[u8]| {
    if data.len() < 2 { return; }
    let Ok(s) = std::str::from_utf8(&data[1..]) else { return };

    let mut parser = select_parser_type(data[0]).get_reasoning_parser();
    let bytes = s.as_bytes();
    let mut pos = 0;
    let mut seed = 1usize;

    while pos < bytes.len() {
        seed = seed.wrapping_mul(31).wrapping_add(7);
        let chunk_len = (seed % 16).max(1).min(bytes.len() - pos);
        let end = match std::str::from_utf8(&bytes[pos..(pos + chunk_len).min(bytes.len())]) {
            Ok(_) => (pos + chunk_len).min(bytes.len()),
            Err(e) => pos + e.valid_up_to(),
        };
        if end == pos { pos += 1; continue; }
        let chunk = unsafe { std::str::from_utf8_unchecked(&bytes[pos..end]) };
        let _ = parser.parse_reasoning_streaming_incremental(chunk, &[]);
        pos = end;
    }
});
