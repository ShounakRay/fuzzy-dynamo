#![no_main]
use bytes::Bytes;
use libfuzzer_sys::fuzz_target;

use dynamo_runtime::transports::event_plane::{Frame, FrameHeader};

// Crash oracle: feed arbitrary bytes to Frame::decode and FrameHeader::decode.
// Must return Ok or Err, never panic.
// payload_len = u32::MAX should not OOM.
fuzz_target!(|data: &[u8]| {
    // Test Frame::decode
    let buf = Bytes::copy_from_slice(data);
    let _ = Frame::decode(buf.clone());

    // Test FrameHeader::decode separately
    let mut cursor = &data[..];
    let _ = FrameHeader::decode(&mut cursor);

    // If we got a valid header, verify frame_size doesn't overflow
    let mut cursor2 = &data[..];
    if let Ok(header) = FrameHeader::decode(&mut cursor2) {
        let _size = header.frame_size();
    }
});
