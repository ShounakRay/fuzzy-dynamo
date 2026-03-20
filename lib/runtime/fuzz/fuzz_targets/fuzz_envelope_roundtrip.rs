#![no_main]
use libfuzzer_sys::fuzz_target;

use dynamo_runtime::transports::event_plane::{EventEnvelope, MsgpackCodec};

// Roundtrip: decode arbitrary bytes as EventEnvelope, re-encode with MsgpackCodec,
// decode again, and assert equality. Catches serialization asymmetries in the
// bytes_serde helper (e.g. if certain MsgPack representations decode successfully
// but re-encode to a different form).
fuzz_target!(|data: &[u8]| {
    let envelope: EventEnvelope = match rmp_serde::from_slice(data) {
        Ok(e) => e,
        Err(_) => return,
    };

    let codec = MsgpackCodec;
    let encoded = codec.encode_envelope(&envelope).expect("re-encode must succeed");
    let decoded = codec.decode_envelope(&encoded).expect("re-decode must succeed");

    assert_eq!(envelope, decoded, "roundtrip mismatch");
});
