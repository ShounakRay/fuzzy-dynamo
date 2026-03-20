# [BUG]: DefaultWorkerSelector::select_worker panics on zero block_size

### Describe the Bug

`DefaultWorkerSelector::select_worker` in `lib/kv-router/src/scheduling/selector.rs` panics with division by zero when called with `block_size = 0`. The function performs `isl.div_ceil(block_size as usize)` at line 113 without validating that `block_size > 0`.

Additionally, at line 141, `(prefill_token as f64) / (block_size as f64)` would produce `Inf` when `block_size == 0` (float division by zero doesn't panic but produces incorrect logits, potentially causing NaN propagation in softmax).

### Steps to Reproduce

```rust
use dynamo_kv_router::selector::{DefaultWorkerSelector, WorkerSelector};
use dynamo_kv_router::SchedulingRequest;
use dynamo_kv_router::protocols::{OverlapScores, WorkerConfigLike};
use std::collections::HashMap;

struct TestConfig;
impl WorkerConfigLike for TestConfig {
    fn data_parallel_start_rank(&self) -> u32 { 0 }
    fn data_parallel_size(&self) -> u32 { 1 }
    fn max_num_batched_tokens(&self) -> Option<u64> { None }
    fn total_kv_blocks(&self) -> Option<u64> { None }
}

let selector = DefaultWorkerSelector::default();
let mut workers = HashMap::new();
workers.insert(0u64, TestConfig);

let request = SchedulingRequest {
    isl_tokens: 100,
    overlaps: OverlapScores::new(),
    // ... other fields default
};

// Panics: "attempt to divide by zero" at selector.rs:113
let _ = selector.select_worker(&workers, &request, 0);
```

### Expected Behavior

Should detect the zero block size and return an error, not panic.

### Actual Behavior

```
thread panicked at 'attempt to divide by zero'
  at lib/kv-router/src/scheduling/selector.rs:113
```

### Environment

On commit `57e6a79f5` in `dynamo-runtime` @ `lib/kv-router/src/scheduling/selector.rs`

### Additional Context

Same class of bug as the `compute_block_hash_for_seq` and `to_block_level` zero block_size panics. If `kv_block_size` is misconfigured as 0, the router will crash on the first scheduling request. The fix would be to add an early validation at the top of `select_worker` returning an error when `block_size == 0`.

Found via fuzzing with cargo-fuzz / libfuzzer.

### Screenshots

_No response_
