#![no_main]
use libfuzzer_sys::fuzz_target;

use bytes::BytesMut;
use dynamo_llm_fuzz::codec::SseLineCodec;
use tokio_util::codec::Decoder;

// Crash oracle: feed raw bytes to SSE decoder.
// Must always return Ok or Err, never panic.
fuzz_target!(|data: &[u8]| {
    let mut codec = SseLineCodec::new();
    let mut buf = BytesMut::from(data);

    loop {
        match codec.decode(&mut buf) {
            Ok(Some(_)) => continue,
            Ok(None) => break,
            Err(_) => return,
        }
    }

    // Flush remaining buffered data on EOF
    let _ = codec.decode_eof(&mut buf);
});
