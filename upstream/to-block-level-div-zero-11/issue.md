# [BUG]: RequestExtraInfo::to_block_level panics on zero block_size

### Describe the Bug

`RequestExtraInfo::to_block_level` in `lib/kv-router/src/protocols.rs` panics with division by zero when called with `block_size = 0`. The function has three separate division-by-zero paths, all triggered by `block_size == 0`:

1. Line 375: `total_tokens.div_ceil(block_size)`
2. Line 381: `req_start / block_size`
3. Line 382: `(req_end.saturating_sub(1)) / block_size`

### Steps to Reproduce

```rust
use dynamo_kv_router::protocols::{RequestExtraInfo, RequestMmObjectInfo};

let info = RequestExtraInfo {
    mm_objects: vec![RequestMmObjectInfo { mm_hash: 42, offsets: vec![(0, 1)] }],
};

// Panics: "attempt to divide by zero"
let _ = info.to_block_level(0, 10);
```

### Expected Behavior

Should detect the zero block size and return an empty result or error, not panic.

### Actual Behavior

```
thread panicked at 'attempt to divide by zero'
  at lib/kv-router/src/protocols.rs:375
```

### Environment

On commit `57e6a79f5` in `dynamo-runtime` @ `lib/kv-router/src/protocols.rs`

### Additional Context

Same root cause as the `compute_block_hash_for_seq` and `select_worker` zero block_size panics. If `kv_block_size` is misconfigured as 0, any code path calling `to_block_level` will crash the router. The fix would be to add an early return for zero `block_size` at the top of the function.

Found via fuzzing with cargo-fuzz / libfuzzer.

### Screenshots

_No response_
