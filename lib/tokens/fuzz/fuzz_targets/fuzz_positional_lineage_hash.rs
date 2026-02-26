#![no_main]
use libfuzzer_sys::fuzz_target;

use dynamo_tokens::PositionalLineageHash;

fuzz_target!(|data: &[u8]| {
    if data.len() < 20 {
        return;
    }

    let current_hash = u64::from_le_bytes(data[0..8].try_into().unwrap());
    let parent_hash_raw = u64::from_le_bytes(data[8..16].try_into().unwrap());
    let position_raw = u32::from_le_bytes(data[16..20].try_into().unwrap());

    // Use parent_hash = None if MSB is set
    let parent_hash = if parent_hash_raw & (1 << 63) != 0 {
        None
    } else {
        Some(parent_hash_raw)
    };

    // Position >= 2^24 is intentionally rejected with panic by the API.
    // Filter it out since catch_unwind doesn't work under ASAN.
    let position = (position_raw as u64) % (1u64 << 24);

    let plh = PositionalLineageHash::new(current_hash, parent_hash, position);

    // Round-trip: position must match
    let retrieved_pos = plh.position();
    assert_eq!(
        retrieved_pos, position,
        "position round-trip failed: stored {} but got {}",
        position, retrieved_pos
    );

    // Mode must be consistent with position
    let mode = plh.mode();
    match mode {
        0 => assert!(position < 256, "mode 0 but position {}", position),
        1 => assert!(position < 65536, "mode 1 but position {}", position),
        2 => assert!(position < 16_777_216, "mode 2 but position {}", position),
        _ => panic!("invalid lineage mode {}", mode),
    }

    // Mode boundary testing
    // Position 255 should be mode 0, position 256 should be mode 1
    if position == 255 {
        assert_eq!(mode, 0, "position 255 should use mode 0");
    }
    if position == 256 {
        assert_eq!(mode, 1, "position 256 should use mode 1");
    }
    if position == 65535 {
        assert_eq!(mode, 1, "position 65535 should use mode 1");
    }
    if position == 65536 {
        assert_eq!(mode, 2, "position 65536 should use mode 2");
    }

    // Accessors should not panic
    let _current_frag = plh.current_hash_fragment();
    let _parent_frag = plh.parent_hash_fragment();
    let _val = plh.as_u128();

    // Determinism: identical inputs produce identical hashes
    let plh2 = PositionalLineageHash::new(current_hash, parent_hash, position);
    assert_eq!(plh, plh2, "identical inputs produced different lineage hashes");
});
