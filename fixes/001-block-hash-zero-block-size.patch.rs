// Fix for Bug 1: compute_block_hash_for_seq panics on zero kv_block_size
// File: lib/kv-router/src/protocols.rs
// Severity: HIGH
//
// Problem: tokens.chunks_exact(kv_block_size as usize) panics with "chunk size must be non-zero"
//          when kv_block_size is 0. A misconfigured worker or malformed ZMQ event triggers this.
// Fix: Early return with empty Vec when kv_block_size == 0.

// === ORIGINAL (lines 37-50) ===
// pub fn compute_block_hash_for_seq(
//     tokens: &[u32],
//     kv_block_size: u32,
//     block_mm_infos: Option<&[Option<BlockExtraInfo>]>,
//     lora_name: Option<&str>,
// ) -> Vec<LocalBlockHash> {
//     let seed = match lora_name.filter(|n| !n.is_empty()) {
//         Some(name) => XXH3_SEED.wrapping_add(xxh3::xxh3_64(name.as_bytes())),
//         None => XXH3_SEED,
//     };
//     tokens
//         .chunks_exact(kv_block_size as usize)  // panics when kv_block_size == 0

// === FIXED ===
pub fn compute_block_hash_for_seq(
    tokens: &[u32],
    kv_block_size: u32,
    block_mm_infos: Option<&[Option<BlockExtraInfo>]>,
    lora_name: Option<&str>,
) -> Vec<LocalBlockHash> {
    if kv_block_size == 0 {
        return Vec::new();
    }
    let seed = match lora_name.filter(|n| !n.is_empty()) {
        Some(name) => XXH3_SEED.wrapping_add(xxh3::xxh3_64(name.as_bytes())),
        None => XXH3_SEED,
    };
    tokens
        .chunks_exact(kv_block_size as usize)
        // ... rest unchanged ...
}

// === TEST ===
#[test]
fn test_compute_block_hash_zero_block_size() {
    // Must not panic, should return empty vec
    let result = compute_block_hash_for_seq(&[1, 2, 3, 4], 0, None, None);
    assert!(result.is_empty());
}

#[test]
fn test_compute_block_hash_normal_operation() {
    // Sanity check: normal operation still works
    let result = compute_block_hash_for_seq(&[1, 2, 3, 4], 2, None, None);
    assert_eq!(result.len(), 2);
}
