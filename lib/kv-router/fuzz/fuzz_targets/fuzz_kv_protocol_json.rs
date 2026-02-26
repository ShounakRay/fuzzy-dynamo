#![no_main]
use libfuzzer_sys::fuzz_target;

use dynamo_kv_router::protocols::{
    ActiveLoad, ActiveSequenceEvent, KvCacheEvents, PrefillEvent, RouterEvent, RouterRequest,
    RouterResponse,
};

// Fuzz all KV protocol JSON deserialization.
// Must return Ok or Err, never panic.
fuzz_target!(|data: &[u8]| {
    let Ok(s) = std::str::from_utf8(data) else {
        return;
    };

    // All these must not panic — only Ok or Err
    let _: Result<RouterRequest, _> = serde_json::from_str(s);
    let _: Result<RouterResponse, _> = serde_json::from_str(s);
    let _: Result<KvCacheEvents, _> = serde_json::from_str(s);
    let _: Result<RouterEvent, _> = serde_json::from_str(s);
    let _: Result<PrefillEvent, _> = serde_json::from_str(s);
    let _: Result<ActiveSequenceEvent, _> = serde_json::from_str(s);
    let _: Result<ActiveLoad, _> = serde_json::from_str(s);
});
