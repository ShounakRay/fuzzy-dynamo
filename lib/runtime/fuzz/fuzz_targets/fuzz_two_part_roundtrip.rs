#![no_main]
use bytes::Bytes;
use libfuzzer_sys::fuzz_target;

use dynamo_runtime::pipeline::network::codec::{TwoPartCodec, TwoPartMessage};

// Round-trip oracle: encode(msg) -> bytes -> decode(bytes) == msg
fuzz_target!(|data: &[u8]| {
    if data.len() < 2 {
        return;
    }

    // Split fuzz input into header and data portions
    let split = data[0] as usize % data.len().max(1);
    let header_bytes = &data[1..=split.min(data.len() - 1)];
    let data_bytes = &data[split.min(data.len() - 1) + 1..];

    let header = Bytes::copy_from_slice(header_bytes);
    let body = Bytes::copy_from_slice(data_bytes);

    let msg = TwoPartMessage::new(header.clone(), body.clone());
    let codec = TwoPartCodec::new(None);

    // Encode
    let encoded = match codec.encode_message(msg) {
        Ok(bytes) => bytes,
        Err(_) => return,
    };

    // Decode
    let decoded = match codec.decode_message(encoded) {
        Ok(msg) => msg,
        Err(e) => {
            panic!(
                "round-trip failed: encode succeeded but decode failed: {:?}",
                e
            );
        }
    };

    // Verify round-trip
    let (dec_header, dec_data) = decoded.into_parts();
    assert_eq!(
        dec_header, header,
        "header mismatch in round-trip"
    );
    assert_eq!(
        dec_data, body,
        "data mismatch in round-trip"
    );

    // Also test header-only and data-only messages
    let header_only = TwoPartMessage::from_header(Bytes::copy_from_slice(header_bytes));
    if let Ok(encoded) = codec.encode_message(header_only) {
        let decoded = codec.decode_message(encoded);
        assert!(decoded.is_ok(), "header-only round-trip decode failed");
    }

    let data_only = TwoPartMessage::from_data(Bytes::copy_from_slice(data_bytes));
    if let Ok(encoded) = codec.encode_message(data_only) {
        let decoded = codec.decode_message(encoded);
        assert!(decoded.is_ok(), "data-only round-trip decode failed");
    }
});
