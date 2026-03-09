# create_stored_blocks / create_stored_block_from_parts OOB panics on mismatched sizes

## Summary

Two related panics in `zmq_wire.rs` when the `token_ids` array is shorter than the block count implies. A malicious or buggy worker can send a KV cache event where `block_hashes` claims N blocks but `token_ids` contains fewer than `N * kv_block_size` tokens, crashing the router.

## Severity

**High** — These functions process data from worker ZMQ events. A single malformed event from any worker crashes the entire router process. The `convert_event` function calls `create_stored_blocks` directly from deserialized `RawKvEvent::BlockStored` data with no bounds validation.

## Bug 1: `create_stored_blocks` — array slice OOB

In `zmq_wire.rs:479`:
```rust
let tokens = &token_ids[token_offset..(token_offset + *num_tokens_it as usize)];
```

When `token_ids` has fewer elements than `token_offset + num_tokens_it`, this panics with "index out of bounds". The check at line 468 (`num_tokens_it != kv_block_size`) only validates that each block claims the right size — it does NOT validate that `token_ids` has enough tokens to back those claims.

### Steps to Reproduce

```rust
use std::sync::Arc;
use std::sync::atomic::AtomicU32;
use dynamo_kv_router::zmq_wire::create_stored_blocks;

let warning_count = Arc::new(AtomicU32::new(0));
// Claims 2 blocks of size 4, but only provides 4 tokens (not 8)
create_stored_blocks(
    4,                    // kv_block_size
    &[1, 2, 3, 4],       // token_ids: only 4 tokens
    &[4, 4],             // num_block_tokens: claims 2 blocks
    &[100, 200],         // block_hashes
    None,
    &warning_count,
    None,
);
// Panics: "index out of bounds: the len is 4 but the index is 8"
```

## Bug 2: `create_stored_block_from_parts` — index on empty Vec

In `zmq_wire.rs:431-436`:
```rust
let tokens_hash = compute_block_hash_for_seq(
    token_ids,
    kv_block_size,
    block_mm_infos.as_deref(),
    lora_name,
)[0];  // panics when result is empty
```

When `token_ids.len() < kv_block_size`, `chunks_exact(kv_block_size)` produces zero chunks, `compute_block_hash_for_seq` returns an empty `Vec`, and `[0]` panics with "index out of bounds".

### Steps to Reproduce

```rust
use dynamo_kv_router::zmq_wire::create_stored_block_from_parts;

// kv_block_size=4 but only 2 tokens
create_stored_block_from_parts(4, 42, &[1, 2], None, None);
// Panics: "index out of bounds: the len is 0 but the index is 0"
```

## Attack Vector

Both bugs are reachable through `convert_event` (zmq_wire.rs:340), which is called when processing ZMQ events from workers. A `BlockStored` event contains `block_hashes` and `token_ids` arrays that can have mismatched sizes. The `convert_event` path constructs `num_block_tokens` as `vec![block_size as u64; block_hashes.len()]` (line 377), then passes the raw `token_ids` without validating they contain enough elements.

## Suggested Fix

Add bounds validation in `create_stored_blocks` before the loop:

```rust
pub fn create_stored_blocks(
    kv_block_size: u32,
    token_ids: &[u32],
    num_block_tokens: &[u64],
    block_hashes: &[u64],
    // ...
) -> Vec<KvCacheStoredBlockData> {
    // Validate total token count matches claimed blocks
    let expected_tokens: usize = num_block_tokens.iter()
        .map(|&n| n as usize)
        .sum();
    if token_ids.len() < expected_tokens {
        tracing::warn!(
            "token_ids length ({}) less than sum of num_block_tokens ({}), skipping",
            token_ids.len(), expected_tokens
        );
        return Vec::new();
    }
    // ... rest unchanged
}
```

For `create_stored_block_from_parts`, validate that `token_ids.len() >= kv_block_size`:

```rust
pub fn create_stored_block_from_parts(
    kv_block_size: u32,
    // ...
) -> KvCacheStoredBlockData {
    assert!(token_ids.len() >= kv_block_size as usize,
        "token_ids too short ({}) for kv_block_size ({})", token_ids.len(), kv_block_size);
    // ... rest unchanged
}
```

Crash artifact: `fuzz/artifacts/fuzz_stored_blocks/crash-1c8cdf5bf03e00120b47ac9add8f5f873d374c45`

Found by: `fuzz_stored_blocks` fuzzer.
