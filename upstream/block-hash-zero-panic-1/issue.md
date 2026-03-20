# [BUG]: compute_block_hash_for_seq panics on zero kv_block_size

### Describe the Bug

`compute_block_hash_for_seq` in `lib/kv-router/src/protocols.rs` panics with "chunk size must be non-zero" when called with `kv_block_size = 0`. The function uses `tokens.chunks_exact(kv_block_size as usize)` (line 44-45) without validating that the block size is positive. `chunks_exact(0)` is a documented panic in the Rust standard library.

### Steps to Reproduce

```rust
use dynamo_kv_router::compute_block_hash_for_seq;

// Panics: "chunk size must be non-zero"
let _ = compute_block_hash_for_seq(&[1, 2, 3, 4], 0, None, None);
```

### Expected Behavior

Should detect the zero block size and return an empty result or error, not panic.

### Actual Behavior

```
thread panicked at 'chunk size must be non-zero'
  at lib/kv-router/src/protocols.rs:44
```

### Environment

On commit `57e6a79f5` in `dynamo-runtime` @ `lib/kv-router/src/protocols.rs`

### Additional Context

`kv_block_size` comes from worker configuration, not directly from untrusted user input. However, a misconfigured worker or a malicious event could trigger this panic in the router, crashing the service. The fix would be to add an early return for zero block size, or validate `kv_block_size > 0` at the configuration/deserialization boundary.

Found via fuzzing with cargo-fuzz / libfuzzer.

### Screenshots

_No response_
