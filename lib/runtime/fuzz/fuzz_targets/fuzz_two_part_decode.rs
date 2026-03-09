#![no_main]
use bytes::Bytes;
use libfuzzer_sys::fuzz_target;

use dynamo_runtime::pipeline::network::codec::TwoPartCodec;

// Crash oracle: feed arbitrary bytes to decode_message.
// Must return Ok or Err, never panic.
fuzz_target!(|data: &[u8]| {
    let codec = TwoPartCodec::new(None);
    let _ = codec.decode_message(Bytes::copy_from_slice(data));

    // Also test with a size limit
    let codec_limited = TwoPartCodec::new(Some(1024));
    let _ = codec_limited.decode_message(Bytes::copy_from_slice(data));

    // Test with very small size limit
    let codec_tiny = TwoPartCodec::new(Some(1));
    let _ = codec_tiny.decode_message(Bytes::copy_from_slice(data));
});
