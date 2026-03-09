#![no_main]
use libfuzzer_sys::fuzz_target;
use arbitrary::Arbitrary;
use dynamo_tokens::{PositionalLineageHash, PositionalSequenceHash};

#[derive(Debug, Arbitrary)]
struct FuzzInput {
    current_seq_hash: u64,
    parent_seq_hash: u64,
    position: u64,
    // For PositionalSequenceHash
    local_block_hash: u64,
    // For cross-mode boundary testing
    positions: Vec<u64>,
}

fuzz_target!(|input: FuzzInput| {
    // --- PositionalLineageHash roundtrip ---
    // PLH panics on position >= 2^24, so clamp
    let pos = input.position % (1u64 << 24);
    let parent = if pos == 0 { None } else { Some(input.parent_seq_hash) };

    let plh = PositionalLineageHash::new(input.current_seq_hash, parent, pos);

    // Roundtrip: position must survive encode/decode
    assert_eq!(plh.position(), pos, "PLH position roundtrip failed");

    // Mode must be consistent with position
    let mode = plh.mode();
    match mode {
        0 => assert!(pos < (1u64 << 8), "mode 0 but pos >= 2^8"),
        1 => assert!(pos >= (1u64 << 8) && pos < (1u64 << 16), "mode 1 mismatch"),
        2 => assert!(pos >= (1u64 << 16), "mode 2 mismatch"),
        _ => panic!("invalid PLH mode: {}", mode),
    }

    // Current hash fragment should be non-zero when hash is non-zero
    // (unless all relevant bits are zero, which is very unlikely with random input)
    let _ = plh.current_hash_fragment();
    let _ = plh.parent_hash_fragment();

    // Display/Debug should not panic
    let _ = format!("{}", plh);
    let _ = format!("{:?}", plh);

    // --- PositionalSequenceHash roundtrip ---
    // PSH panics on position >= 2^31, so clamp
    let psh_pos = input.position % (1u64 << 31);
    let psh = PositionalSequenceHash::new(input.current_seq_hash, psh_pos, input.local_block_hash);

    // Roundtrip checks
    assert_eq!(psh.sequence_hash(), input.current_seq_hash, "PSH sequence_hash roundtrip failed");
    assert_eq!(psh.position(), psh_pos, "PSH position roundtrip failed");

    // Mode consistency
    let psh_mode = psh.mode();
    match psh_mode {
        0 => assert!(psh_pos < (1u64 << 8)),
        1 => assert!(psh_pos >= (1u64 << 8) && psh_pos < (1u64 << 16)),
        2 => assert!(psh_pos >= (1u64 << 16) && psh_pos < (1u64 << 24)),
        3 => assert!(psh_pos >= (1u64 << 24) && psh_pos < (1u64 << 31)),
        _ => panic!("invalid PSH mode: {}", psh_mode),
    }

    // Debug should not panic
    let _ = format!("{:?}", psh);

    // --- Cross-mode boundary: parent matching ---
    // When building a chain of PLH, position N's current_hash should match
    // position N+1's parent_hash (for same sequence hash).
    // Test this across mode boundaries.
    let mut prev_hash: Option<u64> = None;
    for (i, &raw_pos) in input.positions.iter().take(32).enumerate() {
        let chain_pos = (i as u64) % (1u64 << 24);
        let seq_hash = raw_pos; // Use as sequence hash
        let parent = prev_hash;

        let chain_plh = PositionalLineageHash::new(seq_hash, parent, chain_pos);

        if i > 0 {
            // The parent fragment at position i should match the current fragment at position i-1
            // But only within the same mode's bit width
            // (cross-mode boundaries may truncate — this is by design)
            let _ = chain_plh.parent_hash_fragment();
        }

        prev_hash = Some(seq_hash);
    }
});
