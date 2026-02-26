#![no_main]
use libfuzzer_sys::fuzz_target;

use dynamo_tokens::PositionalSequenceHash;

fuzz_target!(|data: &[u8]| {
    if data.len() < 20 {
        return;
    }

    // Parse: 8 bytes seq_hash, 4 bytes position, 8 bytes block_hash
    let seq_hash = u64::from_le_bytes(data[0..8].try_into().unwrap());
    let position_raw = u32::from_le_bytes(data[8..12].try_into().unwrap());
    let block_hash = u64::from_le_bytes(data[12..20].try_into().unwrap());

    // Position must be < 2^31 for PositionalSequenceHash
    let position = (position_raw as u64) % (1u64 << 31);

    let psh = PositionalSequenceHash::new(seq_hash, position, block_hash);

    // Round-trip: accessors must return consistent values
    let retrieved_seq = psh.sequence_hash();
    assert_eq!(
        retrieved_seq, seq_hash,
        "sequence_hash round-trip failed: stored {} but got {}",
        seq_hash, retrieved_seq
    );

    let retrieved_pos = psh.position();
    assert_eq!(
        retrieved_pos, position,
        "position round-trip failed: stored {} but got {}",
        position, retrieved_pos
    );

    // Mode must be consistent with position range
    let mode = psh.mode();
    match mode {
        0 => assert!(position < 256, "mode 0 but position {}", position),
        1 => assert!(position < 65536, "mode 1 but position {}", position),
        2 => assert!(position < 16_777_216, "mode 2 but position {}", position),
        3 => assert!(
            position < 2_147_483_648,
            "mode 3 but position {}",
            position
        ),
        _ => panic!("invalid mode {}", mode),
    }

    // local_block_hash should not panic
    let _lbh = psh.local_block_hash();

    // as_u128 should not panic
    let _val = psh.as_u128();

    // Two identical constructions must produce identical results
    let psh2 = PositionalSequenceHash::new(seq_hash, position, block_hash);
    assert_eq!(psh, psh2, "identical inputs produced different hashes");
});
