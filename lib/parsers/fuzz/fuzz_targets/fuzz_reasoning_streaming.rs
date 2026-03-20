#![no_main]
use libfuzzer_sys::fuzz_target;
use arbitrary::Arbitrary;
use dynamo_parsers::reasoning::ReasoningParser;
use dynamo_parsers_fuzz::select_parser_type;

#[derive(Debug, Arbitrary)]
struct FuzzInput {
    parser_type: u8,
    chunk_sizes: Vec<u8>,
    text: String,
}

fuzz_target!(|input: FuzzInput| {
    if input.text.is_empty() { return; }

    let mut parser = select_parser_type(input.parser_type).get_reasoning_parser();
    let bytes = input.text.as_bytes();
    let mut pos = 0;
    let mut chunk_idx = 0;

    while pos < bytes.len() {
        let raw_size = if chunk_idx < input.chunk_sizes.len() {
            input.chunk_sizes[chunk_idx]
        } else {
            32
        };
        chunk_idx += 1;
        let chunk_len = ((raw_size as usize) % 32).max(1);
        let mut end = (pos + chunk_len).min(bytes.len());

        // Respect UTF-8 boundaries
        while end < bytes.len() && (bytes[end] & 0xC0) == 0x80 {
            end += 1;
        }
        if end == pos {
            pos += 1;
            while pos < bytes.len() && (bytes[pos] & 0xC0) == 0x80 {
                pos += 1;
            }
            continue;
        }

        let chunk = std::str::from_utf8(&bytes[pos..end]).unwrap_or_else(|e| {
            let valid_end = pos + e.valid_up_to();
            if valid_end == pos {
                pos += 1;
                return "";
            }
            unsafe { std::str::from_utf8_unchecked(&bytes[pos..valid_end]) }
        });
        if !chunk.is_empty() {
            let _ = parser.parse_reasoning_streaming_incremental(chunk, &[]);
        }
        pos = end;
    }
});
