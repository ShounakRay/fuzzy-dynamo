#![no_main]
use libfuzzer_sys::fuzz_target;

use dynamo_kv_router::zmq_wire::{BlockHashValue, RawKvEvent};

// Serialize-Deserialize-Serialize roundtrip.
// Constructs valid RawKvEvent from structured fuzz bytes, serializes to msgpack,
// deserializes back, re-serializes, and asserts byte equality.
fuzz_target!(|data: &[u8]| {
    if data.is_empty() {
        return;
    }

    let variant = data[0] % 3;
    let rest = &data[1..];

    let event = match variant {
        0 => {
            // BlockStored
            if rest.len() < 8 {
                return;
            }
            let num_hashes = (rest[0] % 4) as usize + 1;
            let block_size = u16::from_le_bytes([rest[1], rest[2]]) as usize;
            if block_size == 0 {
                return;
            }
            let flags = rest[3];
            let has_parent = flags & 1 != 0;
            let has_medium = flags & 2 != 0;
            let has_lora = flags & 4 != 0;

            let mut pos = 4;

            let block_hashes: Vec<BlockHashValue> = (0..num_hashes)
                .map(|i| {
                    if pos + 8 <= rest.len() {
                        let v = i64::from_le_bytes(rest[pos..pos + 8].try_into().unwrap());
                        pos += 8;
                        BlockHashValue::Signed(v)
                    } else {
                        BlockHashValue::Signed(i as i64)
                    }
                })
                .collect();

            let parent_block_hash = if has_parent && pos + 8 <= rest.len() {
                let v = i64::from_le_bytes(rest[pos..pos + 8].try_into().unwrap());
                pos += 8;
                Some(BlockHashValue::Signed(v))
            } else if has_parent {
                Some(BlockHashValue::Signed(99))
            } else {
                None
            };

            let num_tokens = num_hashes * block_size;
            // Cap tokens to prevent OOM
            if num_tokens > 4096 {
                return;
            }
            let token_ids: Vec<u32> = (0..num_tokens)
                .map(|i| {
                    if pos + 4 <= rest.len() {
                        let v = u32::from_le_bytes(rest[pos..pos + 4].try_into().unwrap());
                        pos += 4;
                        v
                    } else {
                        i as u32
                    }
                })
                .collect();

            let medium = if has_medium {
                Some("gpu".to_string())
            } else {
                None
            };
            let lora_name = if has_lora {
                Some("adapter-a".to_string())
            } else {
                None
            };

            RawKvEvent::BlockStored {
                block_hashes,
                parent_block_hash,
                token_ids,
                block_size,
                medium,
                lora_name,
                block_mm_infos: None,
            }
        }
        1 => {
            // BlockRemoved
            if rest.len() < 2 {
                return;
            }
            let num_hashes = (rest[0] % 4) as usize + 1;
            let has_medium = rest[1] & 1 != 0;
            let mut pos = 2;

            let block_hashes: Vec<BlockHashValue> = (0..num_hashes)
                .map(|i| {
                    if pos + 8 <= rest.len() {
                        let v = u64::from_le_bytes(rest[pos..pos + 8].try_into().unwrap());
                        pos += 8;
                        BlockHashValue::Unsigned(v)
                    } else {
                        BlockHashValue::Unsigned(i as u64)
                    }
                })
                .collect();

            let medium = if has_medium {
                Some("gpu".to_string())
            } else {
                None
            };

            RawKvEvent::BlockRemoved {
                block_hashes,
                medium,
            }
        }
        _ => RawKvEvent::AllBlocksCleared,
    };

    // S-D-S: Serialize → Deserialize → Serialize
    let bytes1 = match rmp_serde::to_vec_named(&event) {
        Ok(b) => b,
        Err(_) => return,
    };

    let decoded: RawKvEvent = match rmp_serde::from_slice(&bytes1) {
        Ok(e) => e,
        Err(_) => return,
    };

    let bytes2 = match rmp_serde::to_vec_named(&decoded) {
        Ok(b) => b,
        Err(_) => return,
    };

    assert_eq!(
        bytes1, bytes2,
        "S-D-S roundtrip must produce identical bytes"
    );
});
