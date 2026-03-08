#![no_main]
use bytes::Bytes;
use libfuzzer_sys::fuzz_target;

use dynamo_runtime::pipeline::network::codec::{TwoPartCodec, TcpRequestMessage};
use dynamo_runtime::transports::event_plane::{Frame, FrameHeader};

/// Consolidated crash oracle for all network codecs.
///
/// Merges: fuzz_two_part_decode, fuzz_tcp_decode, fuzz_frame_decode
///
/// All decoders must return Ok or Err on arbitrary input, never panic.
fuzz_target!(|data: &[u8]| {
    let bytes = Bytes::copy_from_slice(data);

    // --- TwoPartCodec: no limit, 1KB limit, 1-byte limit ---
    let codec = TwoPartCodec::new(None);
    let _ = codec.decode_message(bytes.clone());

    let codec_limited = TwoPartCodec::new(Some(1024));
    let _ = codec_limited.decode_message(bytes.clone());

    let codec_tiny = TwoPartCodec::new(Some(1));
    let _ = codec_tiny.decode_message(bytes.clone());

    // --- TcpRequestMessage ---
    let _ = TcpRequestMessage::decode(&bytes);

    // --- Frame and FrameHeader ---
    let _ = Frame::decode(bytes);

    let mut cursor = &data[..];
    let _ = FrameHeader::decode(&mut cursor);

    let mut cursor2 = &data[..];
    if let Ok(header) = FrameHeader::decode(&mut cursor2) {
        let _size = header.frame_size();
    }
});
