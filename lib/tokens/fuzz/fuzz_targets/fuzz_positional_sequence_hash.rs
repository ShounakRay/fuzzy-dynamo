#![no_main]
use libfuzzer_sys::fuzz_target;

use dynamo_tokens::PositionalSequenceHash;

fuzz_target!(|data: &[u8]| {
    if data.len() < 20 {
        return;
    }

    let seq_hash = u64::from_le_bytes(data[0..8].try_into().unwrap());
    let position = (u32::from_le_bytes(data[8..12].try_into().unwrap()) as u64) % (1u64 << 31);
    let block_hash = u64::from_le_bytes(data[12..20].try_into().unwrap());

    let psh = PositionalSequenceHash::new(seq_hash, position, block_hash);

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

    assert_eq!(psh, PositionalSequenceHash::new(seq_hash, position, block_hash), "determinism failed");
});
