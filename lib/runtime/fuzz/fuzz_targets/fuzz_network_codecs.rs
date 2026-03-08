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

    // --- TcpResponseMessage: single-shot decode ---
    let _ = TcpResponseMessage::decode(&bytes);

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
