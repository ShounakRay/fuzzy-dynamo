#![no_main]
use libfuzzer_sys::fuzz_target;

use bytes::BytesMut;
use dynamo_llm_fuzz::codec::SseLineCodec;
use tokio_util::codec::Decoder;

// Simulate chunked network delivery: feed bytes in variable-sized chunks.
// Tests partial line handling and buffer accumulation across decode() calls.
fuzz_target!(|data: &[u8]| {
    if data.len() < 2 {
        return;
    }

    let chunk_seed = data[0];
    let payload = &data[1..];

    let mut codec = SseLineCodec::new();
    let mut buf = BytesMut::new();
    let mut pos = 0;

    while pos < payload.len() {
        let chunk_size = ((chunk_seed.wrapping_add(pos as u8) % 16) as usize + 1)
            .min(payload.len() - pos);
        buf.extend_from_slice(&payload[pos..pos + chunk_size]);
        pos += chunk_size;

        loop {
            match codec.decode(&mut buf) {
                Ok(Some(_)) => continue,
                Ok(None) => break,
                Err(_) => return,
            }
        }
    }

    let _ = codec.decode_eof(&mut buf);
});
