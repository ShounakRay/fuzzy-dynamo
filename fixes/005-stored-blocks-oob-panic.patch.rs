// Fix for Bug 5: create_stored_blocks / create_stored_block_from_parts OOB panics
// File: lib/kv-router/src/zmq_wire.rs
// Severity: HIGH
//
// Problem: create_stored_blocks indexes token_ids[offset..offset+N] without checking that
//          token_ids is long enough. create_stored_block_from_parts indexes [0] on the result
//          of compute_block_hash_for_seq which returns an empty Vec when token_ids < kv_block_size.
// Fix: In create_stored_blocks, validate total expected tokens <= token_ids.len() before the loop.
//      In create_stored_block_from_parts, validate token_ids.len() >= kv_block_size.

// === ORIGINAL create_stored_blocks (lines 473-500) ===
// pub fn create_stored_blocks(
//     kv_block_size: u32,
//     token_ids: &[u32],
//     num_block_tokens: &[u64],
//     block_hashes: &[u64],
//     lora_name: Option<&str>,
//     warning_count: &Arc<AtomicU32>,
//     block_mm_infos: Option<&[Option<BlockExtraInfo>]>,
// ) -> Vec<KvCacheStoredBlockData> {
//     let mut blocks: Vec<KvCacheStoredBlockData> = Vec::new();
//
//     let mut token_offset: usize = 0;
//     for (block_idx, (num_tokens_it, block_hash_it)) in ...

// === FIXED create_stored_blocks ===
pub fn create_stored_blocks(
    kv_block_size: u32,
    token_ids: &[u32],
    num_block_tokens: &[u64],
    block_hashes: &[u64],
    lora_name: Option<&str>,
    warning_count: &Arc<AtomicU32>,
    block_mm_infos: Option<&[Option<BlockExtraInfo>]>,
) -> Vec<KvCacheStoredBlockData> {
    // Validate total token count matches claimed blocks
    let expected_tokens: usize = num_block_tokens.iter().map(|&n| n as usize).sum();
    if token_ids.len() < expected_tokens {
        tracing::warn!(
            "token_ids length ({}) less than sum of num_block_tokens ({}), skipping",
            token_ids.len(),
            expected_tokens
        );
        return Vec::new();
    }

    let mut blocks: Vec<KvCacheStoredBlockData> = Vec::new();

    let mut token_offset: usize = 0;
    for (block_idx, (num_tokens_it, block_hash_it)) in
        num_block_tokens.iter().zip(block_hashes.iter()).enumerate()
    {
        // ... rest unchanged from original ...
    }
    blocks
}

// === ORIGINAL create_stored_block_from_parts (lines 444-457) ===
// pub fn create_stored_block_from_parts(
//     kv_block_size: u32,
//     block_hash: u64,
//     token_ids: &[u32],
//     lora_name: Option<&str>,
//     mm_extra_info: Option<BlockExtraInfo>,
// ) -> KvCacheStoredBlockData {
//     let block_mm_infos = mm_extra_info.as_ref().map(|info| vec![Some(info.clone())]);
//     let tokens_hash = compute_block_hash_for_seq(
//         token_ids,
//         kv_block_size,
//         block_mm_infos.as_deref(),
//         lora_name,
//     )[0];   // panics when result is empty

// === FIXED create_stored_block_from_parts ===
pub fn create_stored_block_from_parts(
    kv_block_size: u32,
    block_hash: u64,
    token_ids: &[u32],
    lora_name: Option<&str>,
    mm_extra_info: Option<BlockExtraInfo>,
) -> KvCacheStoredBlockData {
    assert!(
        token_ids.len() >= kv_block_size as usize,
        "token_ids too short ({}) for kv_block_size ({})",
        token_ids.len(),
        kv_block_size
    );
    let block_mm_infos = mm_extra_info.as_ref().map(|info| vec![Some(info.clone())]);
    let tokens_hash = compute_block_hash_for_seq(
        token_ids,
        kv_block_size,
        block_mm_infos.as_deref(),
        lora_name,
    )[0];
    // ... rest unchanged ...
}

// === TEST ===
#[test]
fn test_create_stored_blocks_short_token_ids_returns_empty() {
    use std::sync::Arc;
    use std::sync::atomic::AtomicU32;

    let warning_count = Arc::new(AtomicU32::new(0));
    // Claims 2 blocks of size 4 (8 tokens needed), but only provides 4 tokens
    let result = create_stored_blocks(
        4,
        &[1, 2, 3, 4],
        &[4, 4],
        &[100, 200],
        None,
        &warning_count,
        None,
    );
    assert!(result.is_empty(), "Should return empty vec when token_ids too short");
}

#[test]
#[should_panic(expected = "token_ids too short")]
fn test_create_stored_block_from_parts_short_token_ids_panics() {
    // kv_block_size=4 but only 2 tokens
    let _ = create_stored_block_from_parts(4, 42, &[1, 2], None, None);
}
