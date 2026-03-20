#![no_main]
use libfuzzer_sys::fuzz_target;

use dynamo_runtime::transports::event_plane::EventEnvelope;

// Crash oracle: feed arbitrary bytes to MsgPack deserialization of EventEnvelope.
// Must return Ok or Err, never panic.
// Exercises: MsgPack parsing, custom bytes_serde deserializer, String allocation
// for topic field, u64 decoding for publisher_id/sequence/published_at.
fuzz_target!(|data: &[u8]| {
    let _ = rmp_serde::from_slice::<EventEnvelope>(data);
});
