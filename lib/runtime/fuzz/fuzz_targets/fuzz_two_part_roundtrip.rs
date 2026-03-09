#![no_main]
use bytes::Bytes;
use libfuzzer_sys::fuzz_target;
use dynamo_runtime::pipeline::network::codec::{TwoPartCodec, TwoPartMessage};

fuzz_target!(|data: &[u8]| {
    if data.len() < 2 { return; }

    let split = data[0] as usize % data.len().max(1);
    let header_bytes = &data[1..=split.min(data.len() - 1)];
    let data_bytes = &data[split.min(data.len() - 1) + 1..];

    let header = Bytes::copy_from_slice(header_bytes);
    let body = Bytes::copy_from_slice(data_bytes);
    let codec = TwoPartCodec::new(None);

    let encoded = match codec.encode_message(TwoPartMessage::new(header.clone(), body.clone())) {
        Ok(bytes) => bytes,
        Err(_) => return,
    };
    let decoded = codec.decode_message(encoded)
        .expect("round-trip: encode succeeded but decode failed");
    let (dec_header, dec_data) = decoded.into_parts();
    assert_eq!(dec_header, header, "header mismatch");
    assert_eq!(dec_data, body, "data mismatch");

    // Header-only and data-only variants
    if let Ok(enc) = codec.encode_message(TwoPartMessage::from_header(Bytes::copy_from_slice(header_bytes))) {
        assert!(codec.decode_message(enc).is_ok(), "header-only round-trip failed");
    }
    if let Ok(enc) = codec.encode_message(TwoPartMessage::from_data(Bytes::copy_from_slice(data_bytes))) {
        assert!(codec.decode_message(enc).is_ok(), "data-only round-trip failed");
    }
});
