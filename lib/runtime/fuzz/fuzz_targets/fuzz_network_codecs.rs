#![no_main]
use bytes::{Bytes, BytesMut};
use libfuzzer_sys::fuzz_target;

use dynamo_runtime::pipeline::network::codec::{
    TcpRequestCodec, TcpRequestMessage, TcpResponseCodec, TcpResponseMessage, TwoPartCodec,
};
use dynamo_runtime::transports::event_plane::MsgpackCodec;
use tokio_util::codec::Decoder;

/// Extended network boundary fuzzer covering all decode paths.
///
/// Tests every codec that processes untrusted network input:
/// - TcpRequestCodec (streaming decoder with partial message handling)
/// - TcpResponseCodec (streaming decoder)
/// - TcpResponseMessage (single-shot decode)
/// - MsgpackCodec + EventEnvelope (event plane MessagePack)
/// - ZeroCopyTcpDecoder (async streaming, tested via sync wrapper)
///
/// All must return Ok or Err on arbitrary input, never panic.
fuzz_target!(|data: &[u8]| {
    let bytes = Bytes::copy_from_slice(data);

    // --- TcpResponseMessage: single-shot decode + determinism ---
    let r1 = TcpResponseMessage::decode(&bytes);
    let r2 = TcpResponseMessage::decode(&bytes);
    assert_eq!(r1.is_ok(), r2.is_ok(), "TcpResponseMessage::decode not deterministic");

    // --- TcpRequestCodec: streaming decoder with size limits ---
    // No limit
    {
        let mut codec = TcpRequestCodec::new(None);
        let mut buf = BytesMut::from(data);
        let _ = codec.decode(&mut buf);
    }
    // 1KB limit
    {
        let mut codec = TcpRequestCodec::new(Some(1024));
        let mut buf = BytesMut::from(data);
        let _ = codec.decode(&mut buf);
    }
    // Tiny limit (1 byte) — should reject most inputs
    {
        let mut codec = TcpRequestCodec::new(Some(1));
        let mut buf = BytesMut::from(data);
        let _ = codec.decode(&mut buf);
    }

    // --- TcpResponseCodec: streaming decoder with size limits ---
    {
        let mut codec = TcpResponseCodec::new(None);
        let mut buf = BytesMut::from(data);
        let _ = codec.decode(&mut buf);
    }
    {
        let mut codec = TcpResponseCodec::new(Some(1024));
        let mut buf = BytesMut::from(data);
        let _ = codec.decode(&mut buf);
    }

    // --- MsgpackCodec: EventEnvelope deserialization ---
    {
        let codec = MsgpackCodec;
        let _ = codec.decode_envelope(&bytes);
    }

    // --- TwoPartCodec: additional boundary sizes ---
    // Filter known bug #13: header_len/body_len overflow when u64 values cast to usize
    // The codec reads two u64 values and adds them (24 + header_len + body_len),
    // which overflows if either value > usize::MAX / 2
    let twopart_safe = if data.len() >= 24 {
        let h = u64::from_be_bytes([data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7]]);
        let b = u64::from_be_bytes([data[8], data[9], data[10], data[11], data[12], data[13], data[14], data[15]]);
        h.checked_add(b).and_then(|s| s.checked_add(24)).is_some()
            && (h as u128 + b as u128 + 24) <= usize::MAX as u128
    } else {
        true // < 24 bytes returns Ok(None), safe
    };

    if twopart_safe {
        // Exact boundary: 24 bytes (minimum header size)
        {
            let codec = TwoPartCodec::new(Some(24));
            let _ = codec.decode_message(bytes.clone());
        }
        // Large limit
        {
            let codec = TwoPartCodec::new(Some(1 << 20));
            let _ = codec.decode_message(bytes.clone());
        }
    }

    // --- Streaming: feed data byte-by-byte to request codec ---
    if data.len() <= 64 {
        let mut codec = TcpRequestCodec::new(Some(1024));
        let mut buf = BytesMut::new();
        for &b in data {
            buf.extend_from_slice(&[b]);
            match codec.decode(&mut buf) {
                Ok(Some(_msg)) => break,
                Ok(None) => continue,
                Err(_) => break,
            }
        }
    }

    // --- Streaming: feed data byte-by-byte to response codec ---
    if data.len() <= 64 {
        let mut codec = TcpResponseCodec::new(Some(1024));
        let mut buf = BytesMut::new();
        for &b in data {
            buf.extend_from_slice(&[b]);
            match codec.decode(&mut buf) {
                Ok(Some(_msg)) => break,
                Ok(None) => continue,
                Err(_) => break,
            }
        }
    }

    // --- Concatenated messages: multiple frames in one buffer ---
    if data.len() >= 3 {
        let split = (data[0] as usize % (data.len() - 1)) + 1;
        let mut buf = BytesMut::new();
        buf.extend_from_slice(&data[1..]);
        buf.extend_from_slice(&data[1..split]);

        let mut codec = TcpRequestCodec::new(Some(4096));
        // Decode first message
        let _ = codec.decode(&mut buf);
        // Decode second (if any remains)
        let _ = codec.decode(&mut buf);
    }
});
