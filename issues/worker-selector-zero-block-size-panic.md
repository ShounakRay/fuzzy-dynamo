# Bug 12: DefaultWorkerSelector::select_worker panics on zero block_size

## Summary

`DefaultWorkerSelector::select_worker` panics with division by zero when called with `block_size = 0`. The function performs `isl.div_ceil(block_size as usize)` at `selector.rs:113` without validating that `block_size > 0`.

## Severity

**Medium** — Same class of bug as `compute_block_hash_for_seq` and `to_block_level` zero block_size panics. If `kv_block_size` is misconfigured as 0, the router will crash on the first scheduling request.

## Steps to Reproduce

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

## Root Cause

In `scheduling/selector.rs:113`:
```rust
let request_blocks = isl.div_ceil(block_size as usize);  // panics when block_size == 0
```

Additionally at line 141:
```rust
let potential_prefill_block = (prefill_token as f64) / (block_size as f64);
```
This would produce `Inf` when `block_size == 0` (float division by zero doesn't panic but produces incorrect logits, potentially causing NaN propagation in softmax).

## Crash Artifacts

- `fuzz/artifacts/fuzz_worker_selector_div/crash-dd9e1428fb007da4b77e0b7a811cb2147464a6aa`

## Suggested Fix

Add an early validation at the top of `select_worker`:

```rust
if block_size == 0 {
    return Err(KvSchedulerError::NoEndpoints);  // or a new InvalidConfig variant
}
```

Found by: `fuzz_worker_selector_div` fuzzer.
