### [BUG]: `compute_block_hash_for_seq` panics on `kv_block_size=0`

### What This Bug Is (Plain English)

Dynamo's KV router manages cached data for AI model inference. It splits token sequences into fixed-size blocks and computes a hash for each block. The block size is a parameter — how many tokens go in each chunk.

The problem: if the block size is zero, the code tries to split tokens into chunks of size zero, which is like asking "how many groups of zero can I make?" — it's nonsensical and crashes immediately. Any KV cache request that arrives with a zero block size takes down the router process.

### Describe the Bug

`compute_block_hash_for_seq()` in `lib/kv-router/src/protocols.rs` calls `tokens.chunks_exact(kv_block_size as usize)` without checking for zero. When `kv_block_size=0`, this panics in the standard library with `chunk_size must be non-zero`.

Any KV cache request with `kv_block_size=0` crashes the router process.

### Steps to Reproduce

```rust
use dynamo_kv_router::compute_block_hash_for_seq;

// This panics with "chunk_size must be non-zero":
let _ = compute_block_hash_for_seq(&[1, 2, 3, 4], 0, None, None);
```

Alternatively, found via `cargo-fuzz`:

```bash
cd lib/kv-router
cargo +nightly fuzz run fuzz_block_hash_computation -- -max_total_time=60
```

Minimal crashing input: 5 bytes of zeros (`kv_block_size` decoded as `u32 = 0`).

### Expected Behavior

`compute_block_hash_for_seq` should return an empty `Vec` (or an `Err`) when `kv_block_size=0`, not panic.

### Actual Behavior

```
thread 'main' panicked at 'chunk_size must be non-zero'
```

Stack trace points to `tokens.chunks_exact(0)` inside `compute_block_hash_for_seq`.

### Suggested Fix

Add a zero-check guard at the top of the function:

```rust
pub fn compute_block_hash_for_seq(
    tokens: &[u32],
    kv_block_size: u32,
    // ...
) -> Vec<LocalBlockHash> {
    if kv_block_size == 0 {
        return vec![];
    }
    // ... existing code
}
```

### Environment

- dynamo: main branch
- Crate: `dynamo-kv-router`
- File: `lib/kv-router/src/protocols.rs`

### Additional Context

Found via fuzzing with `cargo-fuzz` / libfuzzer. Crash artifact preserved at `lib/kv-router/fuzz/artifacts/fuzz_block_hash_computation/crash-7722745105e9e02e8f1aaf17f7b3aac5c56cd805`.

Related: #3112 (same class — division by zero in the KV router/planner subsystem).
