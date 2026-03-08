#![no_main]
use libfuzzer_sys::fuzz_target;
use dynamo_tokens::{PositionalSequenceHash, PositionalLineageHash};

/// Consolidated positional hash fuzzer.
///
/// Merges: fuzz_positional_sequence_hash, fuzz_positional_lineage_hash,
///         fuzz_positional_hash (new)
///
/// Tests round-trip, mode selection, boundary values, and all accessors
/// for both PositionalSequenceHash and PositionalLineageHash.
fuzz_target!(|data: &[u8]| {
    if data.len() < 20 { return; }

    let seq_hash = u64::from_le_bytes(data[0..8].try_into().unwrap());
    let raw_position = u64::from_le_bytes(data[8..16].try_into().unwrap());
    let local_block_hash = u64::from_le_bytes(data[12..20].try_into().unwrap());

    // === PositionalSequenceHash ===
    let position = raw_position % (1u64 << 31);
    let psh = PositionalSequenceHash::new(seq_hash, position, local_block_hash);

    assert_eq!(psh.sequence_hash(), seq_hash, "seq_hash round-trip failed");
    assert_eq!(psh.position(), position, "position round-trip failed");

    match psh.mode() {
        0 => assert!(position < 256),
        1 => assert!(position < 65536),
        2 => assert!(position < 16_777_216),
        3 => assert!(position < 2_147_483_648),
        m => panic!("invalid mode {m}"),
    }

    let _ = psh.local_block_hash();
    let _ = psh.as_u128();

    let psh2 = PositionalSequenceHash::new(seq_hash, position, local_block_hash);
    assert_eq!(psh.as_u128(), psh2.as_u128(), "PSH determinism failed");

    // === PositionalLineageHash ===
    let lineage_position = raw_position % (1u64 << 24);
    let parent_hash = if data[0] & 1 == 0 { Some(seq_hash) } else { None };
    let plh = PositionalLineageHash::new(seq_hash, parent_hash, lineage_position);

    assert_eq!(plh.position(), lineage_position, "lineage position round-trip failed");

    match plh.mode() {
        0 => assert!(lineage_position < 256),
        1 => assert!(lineage_position < 65536),
        2 => assert!(lineage_position < 16_777_216),
        m => panic!("invalid lineage mode {m}"),
    }

    if lineage_position == 255 { assert_eq!(plh.mode(), 0); }
    if lineage_position == 256 { assert_eq!(plh.mode(), 1); }
    if lineage_position == 65535 { assert_eq!(plh.mode(), 1); }
    if lineage_position == 65536 { assert_eq!(plh.mode(), 2); }

    let _ = plh.current_hash_fragment();
    let _ = plh.parent_hash_fragment();
    let _ = plh.as_u128();

    let plh2 = PositionalLineageHash::new(seq_hash, parent_hash, lineage_position);
    assert_eq!(plh.as_u128(), plh2.as_u128(), "PLH determinism failed");

    // === Boundary values ===
    for &pos in &[0u64, 1, 254, 255, 256, 65534, 65535, 65536, (1 << 24) - 1] {
        let h = PositionalSequenceHash::new(seq_hash, pos, local_block_hash);
        assert_eq!(h.position(), pos, "Boundary PSH position {pos} failed");

        if pos < (1u64 << 24) {
            let l = PositionalLineageHash::new(seq_hash, parent_hash, pos);
            assert_eq!(l.position(), pos, "Boundary PLH position {pos} failed");
        }
    }
});
