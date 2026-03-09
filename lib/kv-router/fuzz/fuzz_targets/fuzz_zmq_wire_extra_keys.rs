#![no_main]
use libfuzzer_sys::fuzz_target;

use dynamo_kv_router::zmq_wire::{extra_keys_to_block_mm_infos, parse_mm_hash_from_extra_key};

// Two modes testing helper functions from zmq_wire.
fuzz_target!(|data: &[u8]| {
    if data.is_empty() {
        return;
    }

    let mode = data[0] % 2;
    let rest = &data[1..];

    match mode {
        0 => {
            // parse_mm_hash_from_extra_key oracle:
            // Must return Some iff input is exactly 64 hex chars, None otherwise.
            let Ok(s) = std::str::from_utf8(rest) else {
                return;
            };
            let result = parse_mm_hash_from_extra_key(s);
            let is_64_hex = s.len() == 64 && s.chars().all(|c| c.is_ascii_hexdigit());
            if is_64_hex {
                assert!(result.is_some(), "64 hex chars must return Some");
            } else {
                assert!(result.is_none(), "non-64-hex must return None");
            }
        }
        _ => {
            // extra_keys_to_block_mm_infos oracle:
            // Output length must equal input length when Some.
            if rest.is_empty() {
                let _ = extra_keys_to_block_mm_infos(None);
                return;
            }

            let num_blocks = (rest[0] % 8) as usize;
            let mut pos = 1;

            let mut blocks: Vec<Option<Vec<String>>> = Vec::new();
            for _ in 0..num_blocks {
                if pos >= rest.len() {
                    blocks.push(None);
                    continue;
                }
                let flags = rest[pos];
                pos += 1;

                if flags & 1 == 0 {
                    blocks.push(None);
                } else {
                    let num_keys = ((flags >> 1) % 4) as usize;
                    let mut keys = Vec::new();
                    for _ in 0..num_keys {
                        if pos >= rest.len() {
                            break;
                        }
                        let len = (rest[pos] % 65) as usize;
                        pos += 1;
                        let end = (pos + len).min(rest.len());
                        if pos <= end {
                            if let Ok(s) = std::str::from_utf8(&rest[pos..end]) {
                                keys.push(s.to_string());
                            }
                        }
                        pos = end;
                    }
                    blocks.push(Some(keys));
                }
            }

            let result = extra_keys_to_block_mm_infos(Some(blocks.clone()));
            if let Some(ref infos) = result {
                assert_eq!(
                    infos.len(),
                    blocks.len(),
                    "output length must match input length"
                );
            }
        }
    }
});
