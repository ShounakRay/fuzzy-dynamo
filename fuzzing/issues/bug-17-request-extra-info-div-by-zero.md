### [BUG]: `RequestExtraInfo::to_block_level` panics with division by zero when `block_size=0`

### What This Bug Is (Plain English)

The KV router needs to figure out which cached blocks correspond to a request's tokens. It does this by dividing token positions by the block size to find which block they fall into. But if the block size is zero, dividing by zero crashes the program instantly.

This is the same class of bug as Bug 15 (zero block size in `compute_block_hash_for_seq`) — the KV router subsystem doesn't validate the block size parameter before doing math with it. Any request with a zero block size crashes the router.

### Describe the Bug

`RequestExtraInfo::to_block_level()` in `lib/kv-router/src/protocols.rs` divides by `block_size` in three places without checking for zero:

- Line 365: `total_tokens.div_ceil(block_size)`
- Line 371: `req_start / block_size`
- Line 372: `(req_end.saturating_sub(1)) / block_size`

Any call with `block_size=0` panics with `attempt to divide by zero`.

### Steps to Reproduce

```rust
use dynamo_kv_router::protocols::{RequestExtraInfo, RequestMmObjectInfo};

let info = RequestExtraInfo {
    mm_objects: vec![RequestMmObjectInfo { mm_hash: 42, offsets: vec![(0, 1)] }],
};

// This panics with "attempt to divide by zero":
let _ = info.to_block_level(0, 10);
```

Alternatively, found via `cargo-fuzz`:

```bash
cd lib/kv-router
cargo +nightly fuzz run fuzz_request_extra_info -- -max_total_time=60
```

Two distinct minimal inputs trigger the crash:
- `[0, 0, 0, 0, 0, 10]` — `block_size=0`, `total_tokens=0`, with mm_objects
- `[0, 0, 10, 186, 10, 10]` — `block_size=0`, `total_tokens=2746`

### Expected Behavior

`to_block_level` should return an empty `Vec` (or an `Err`) when `block_size=0`, not panic.

### Actual Behavior

```
thread 'main' panicked at 'attempt to divide by zero'
  at lib/kv-router/src/protocols.rs:365
```

### Suggested Fix

Add a zero-check guard at the top of `to_block_level`:

```rust
pub fn to_block_level(
    &self,
    block_size: usize,
    total_tokens: usize,
) -> Vec<Option<BlockExtraInfo>> {
    if block_size == 0 {
        return vec![];
    }
    // ... existing code
}
```

### Environment

- dynamo: main branch
- Crate: `dynamo-kv-router`
- File: `lib/kv-router/src/protocols.rs`, lines 360-380

### Additional Context

Found via fuzzing with `cargo-fuzz` / libfuzzer. Two crash artifacts preserved:
- `fuzz/artifacts/fuzz_request_extra_info/crash-d67282f0e9533909e48d16aa0ccb3e408c15f3f2`
- `fuzz/artifacts/fuzz_request_extra_info/crash-0cc8a1cc896ff94ec03af52f14c14be935affafe`

Related: #3112 (same class — division by zero in the KV router subsystem).
