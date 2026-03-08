#![no_main]
use libfuzzer_sys::fuzz_target;
use dynamo_kv_router::zmq_wire::{
    parse_mm_hash_from_extra_key, extra_keys_to_block_mm_infos,
    KvEventBatch, RawKvEvent,
};

fuzz_target!(|data: &[u8]| {
    // --- KvEventBatch: full JSON deserialization of batch events ---
    // Exercises RawKvEventVisitor which handles both map and seq formats,
    // backward compat with extra fields, BlockHashValue signed/unsigned, etc.
    let _ = serde_json::from_slice::<KvEventBatch>(data);

    // --- RawKvEvent: single event deserialization ---
    let _ = serde_json::from_slice::<RawKvEvent>(data);

    // --- Original parse_mm_hash_from_extra_key tests ---
    let Ok(s) = std::str::from_utf8(data) else { return };

    // parse_mm_hash_from_extra_key must not panic
    let result = parse_mm_hash_from_extra_key(s);

    // If the string is exactly 64 hex chars, we should get Some
    if s.len() == 64 && s.chars().all(|c| c.is_ascii_hexdigit()) {
        assert!(result.is_some(),
            "Expected Some for 64-char hex string, got None: {:?}", &s[..16]);
    }
    // If it's not 64 chars or has non-hex, should be None
    if s.len() != 64 || !s.chars().all(|c| c.is_ascii_hexdigit()) {
        assert!(result.is_none(),
            "Expected None for non-64-hex string (len={}), got {:?}", s.len(), result);
    }

    // Determinism
    let result2 = parse_mm_hash_from_extra_key(s);
    assert_eq!(result, result2, "parse_mm_hash_from_extra_key not deterministic");

    // extra_keys_to_block_mm_infos with fuzz-generated nested structure
    if data.len() >= 4 {
        let num_blocks = (data[0] % 8) as usize;
        let mut blocks: Vec<Option<Vec<String>>> = Vec::new();
        let mut pos = 1;
        for _ in 0..num_blocks {
            if pos >= data.len() { break; }
            match data[pos] % 3 {
                0 => { blocks.push(None); pos += 1; }
                1 => { blocks.push(Some(vec![])); pos += 1; }
                _ => {
                    pos += 1;
                    let count = if pos < data.len() { (data[pos] % 4) as usize + 1 } else { 1 };
                    pos += 1;
                    let mut keys = Vec::new();
                    for _ in 0..count {
                        if pos + 2 > data.len() { break; }
                        let key_len = (data[pos] % 32) as usize;
                        pos += 1;
                        let end = (pos + key_len).min(data.len());
                        if let Ok(key) = std::str::from_utf8(&data[pos..end]) {
                            keys.push(key.to_string());
                        }
                        pos = end;
                    }
                    blocks.push(Some(keys));
                }
            }
        }

        let _ = extra_keys_to_block_mm_infos(Some(blocks));
    }

    // None case
    let _ = extra_keys_to_block_mm_infos(None);
});
