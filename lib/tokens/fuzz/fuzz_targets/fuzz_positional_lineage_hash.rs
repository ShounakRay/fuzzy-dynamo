#![no_main]
use libfuzzer_sys::fuzz_target;

use dynamo_tokens::PositionalLineageHash;

fuzz_target!(|data: &[u8]| {
    if data.len() < 20 {
        return;
    }

    let current_hash = u64::from_le_bytes(data[0..8].try_into().unwrap());
    let parent_hash_raw = u64::from_le_bytes(data[8..16].try_into().unwrap());
    let parent_hash = if parent_hash_raw & (1 << 63) != 0 { None } else { Some(parent_hash_raw) };
    // Position >= 2^24 panics by design; filter since catch_unwind doesn't work under ASAN
    let position = (u32::from_le_bytes(data[16..20].try_into().unwrap()) as u64) % (1u64 << 24);

    let plh = PositionalLineageHash::new(current_hash, parent_hash, position);

    assert_eq!(plh.position(), position, "position round-trip failed");

    match plh.mode() {
        0 => assert!(position < 256),
        1 => assert!(position < 65536),
        2 => assert!(position < 16_777_216),
        m => panic!("invalid lineage mode {m}"),
    }

    if position == 255 { assert_eq!(plh.mode(), 0); }
    if position == 256 { assert_eq!(plh.mode(), 1); }
    if position == 65535 { assert_eq!(plh.mode(), 1); }
    if position == 65536 { assert_eq!(plh.mode(), 2); }

    let _ = plh.current_hash_fragment();
    let _ = plh.parent_hash_fragment();
    let _ = plh.as_u128();

    assert_eq!(plh, PositionalLineageHash::new(current_hash, parent_hash, position), "determinism failed");
});
