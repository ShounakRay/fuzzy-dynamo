#![no_main]
use bytes::Bytes;
use libfuzzer_sys::fuzz_target;

use dynamo_runtime::transports::event_plane::{Frame, MsgpackCodec};

// Full pipeline: Frame::decode -> extract payload -> MsgpackCodec::decode_envelope.
// This is the exact path every ZMQ/NATS subscriber uses when receiving events.
// Tests frame header parsing + MsgPack deserialization in combination.
fuzz_target!(|data: &[u8]| {
    let buf = Bytes::copy_from_slice(data);
    if let Ok(frame) = Frame::decode(buf) {
        let codec = MsgpackCodec;
        let _ = codec.decode_envelope(&frame.payload);
    }
});
