#![no_main]
use libfuzzer_sys::fuzz_target;

use dynamo_kv_router::zmq_wire::KvEventBatch;

// Crash oracle: raw msgpack bytes → KvEventBatch deserialization.
// Must always return Ok or Err, never panic.
fuzz_target!(|data: &[u8]| {
    let _: Result<KvEventBatch, _> = rmp_serde::from_slice(data);
});
