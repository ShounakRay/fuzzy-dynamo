#![no_main]
use libfuzzer_sys::fuzz_target;

use bytes::BytesMut;
use dynamo_llm_fuzz::codec::SseLineCodec;
use tokio_util::codec::Decoder;

// Full pipeline: decode SSE messages then attempt JSON deserialization
// of the data field via decode_data::<serde_json::Value>().
fuzz_target!(|data: &[u8]| {
    let mut codec = SseLineCodec::new();
    let mut buf = BytesMut::from(data);

    loop {
        match codec.decode(&mut buf) {
            Ok(Some(msg)) => {
                if msg.data.is_some() {
                    let _ = msg.decode_data::<serde_json::Value>();
                }
            }
            Ok(None) => break,
            Err(_) => break,
        }
    }

    if let Ok(Some(msg)) = codec.decode_eof(&mut buf) {
        if msg.data.is_some() {
            let _ = msg.decode_data::<serde_json::Value>();
        }
    }
});
