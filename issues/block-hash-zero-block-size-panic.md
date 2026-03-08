# compute_block_hash_for_seq panics on zero kv_block_size

## Summary

`compute_block_hash_for_seq` panics with "chunk size must be non-zero" when called with `kv_block_size = 0`. The function uses `tokens.chunks_exact(kv_block_size as usize)` without validating that the block size is positive.

## Severity

**Medium** — `kv_block_size` comes from worker configuration, not directly from untrusted user input. However, a misconfigured worker or a malicious event could trigger this panic in the router, crashing the service.

## Steps to Reproduce

```rust
use dynamo_kv_router::compute_block_hash_for_seq;

// Panics: "chunk size must be non-zero"
let _ = compute_block_hash_for_seq(&[1, 2, 3, 4], 0, None, None);
```

Crash artifact: `fuzz/artifacts/fuzz_block_hash_computation/crash-a10909c2cdcaf5adb7e6b092a4faba558b62bd96`

## Root Cause

In `protocols.rs:44-45`:
```rust
tokens
    .chunks_exact(kv_block_size as usize)  // panics when kv_block_size == 0
```

`chunks_exact(0)` is a documented panic in the standard library.

## Suggested Fix

Add an early return for zero block size:

```rust
pub fn compute_block_hash_for_seq(
    tokens: &[u32],
    kv_block_size: u32,
    block_mm_infos: Option<&[Option<BlockExtraInfo>]>,
    lora_name: Option<&str>,
) -> Vec<LocalBlockHash> {
    if kv_block_size == 0 {
        return Vec::new();
    }
    // ... rest unchanged
}
```

Alternatively, validate `kv_block_size > 0` at the configuration/deserialization boundary and make the function `debug_assert!(kv_block_size > 0)`.

Found by: `fuzz_block_hash_computation` fuzzer.
