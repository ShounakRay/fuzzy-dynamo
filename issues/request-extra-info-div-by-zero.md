# RequestExtraInfo::to_block_level panics on zero block_size

## Summary

`RequestExtraInfo::to_block_level` panics with division by zero when called with `block_size = 0`. The function uses `total_tokens.div_ceil(block_size)` and `req_start / block_size` without validating that `block_size > 0`.

## Severity

**Medium** — Same root cause as the `compute_block_hash_for_seq` zero block_size panic. If `kv_block_size` is misconfigured as 0, any code path calling `to_block_level` will crash the router.

## Steps to Reproduce

```rust
use dynamo_kv_router::protocols::{RequestExtraInfo, RequestMmObjectInfo};

let info = RequestExtraInfo {
    mm_objects: vec![RequestMmObjectInfo { mm_hash: 42, offsets: vec![(0, 1)] }],
};

// Panics: "attempt to divide by zero"
let _ = info.to_block_level(0, 10);
```

## Root Cause

In `protocols.rs:375`:
```rust
let num_blocks = total_tokens.div_ceil(block_size);  // panics when block_size == 0
```

And at lines 381-382:
```rust
let start_block = req_start / block_size;             // also panics
let end_block = (req_end.saturating_sub(1)) / block_size;  // also panics
```

Three separate division-by-zero paths in the same function, all triggered by `block_size == 0`.

## Suggested Fix

Add an early return for zero block_size:

```rust
pub fn to_block_level(
    &self,
    block_size: usize,
    total_tokens: usize,
) -> Vec<Option<BlockExtraInfo>> {
    if block_size == 0 || total_tokens == 0 {
        return Vec::new();
    }
    // ... rest unchanged
}
```

Crash artifacts:
- `fuzz/artifacts/fuzz_request_extra_info/crash-7722745105e9e02e8f1aaf17f7b3aac5c56cd805`
- `fuzz/artifacts/fuzz_request_extra_info/crash-0cc8a1cc896ff94ec03af52f14c14be935affafe`
- `fuzz/artifacts/fuzz_request_extra_info/crash-1d4e4c1fb19f5198eb8f134a51dd64b4b143160d`

Found by: `fuzz_request_extra_info` fuzzer.
