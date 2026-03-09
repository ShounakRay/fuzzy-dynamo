#![no_main]
use bytes::Bytes;
use libfuzzer_sys::fuzz_target;

use dynamo_runtime::pipeline::network::codec::TcpRequestMessage;

// Crash oracle: feed arbitrary bytes to TcpRequestMessage::decode.
// Must return Ok or Err, never panic.
fuzz_target!(|data: &[u8]| {
    let bytes = Bytes::copy_from_slice(data);
    let _ = TcpRequestMessage::decode(&bytes);
});
