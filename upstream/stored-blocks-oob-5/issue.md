# [BUG]: create_stored_blocks and create_stored_block_from_parts OOB panics on mismatched sizes

### Describe the Bug

Two related panics in `lib/kv-router/src/zmq_wire.rs` when the `token_ids` array is shorter than the block count implies. A malicious or buggy worker can send a KV cache event where `block_hashes` claims N blocks but `token_ids` contains fewer than `N * kv_block_size` tokens, crashing the router.

**Bug 1 — `create_stored_blocks` (line 479):** The slice `&token_ids[token_offset..(token_offset + *num_tokens_it as usize)]` panics with "index out of bounds" when `token_ids` has fewer elements than required. The check at line 468 (`num_tokens_it != kv_block_size`) only validates that each block claims the right size — it does NOT validate that `token_ids` has enough tokens to back those claims.

**Bug 2 — `create_stored_block_from_parts` (line 431-436):** When `token_ids.len() < kv_block_size`, `chunks_exact(kv_block_size)` produces zero chunks, `compute_block_hash_for_seq` returns an empty `Vec`, and indexing `[0]` panics with "index out of bounds".

Both bugs are reachable through `convert_event` (zmq_wire.rs:340), which processes ZMQ events from workers. A `BlockStored` event contains `block_hashes` and `token_ids` arrays that can have mismatched sizes. The `convert_event` path constructs `num_block_tokens` as `vec![block_size as u64; block_hashes.len()]` (line 377), then passes the raw `token_ids` without validating they contain enough elements.

### Steps to Reproduce

Bug 1:

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

Bug 2:

```rust
use dynamo_kv_router::zmq_wire::create_stored_block_from_parts;

// kv_block_size=4 but only 2 tokens
create_stored_block_from_parts(4, 42, &[1, 2], None, None);
// Panics: "index out of bounds: the len is 0 but the index is 0"
```

### Expected Behavior

Should validate that `token_ids` has enough elements before indexing, and return an error or empty result instead of panicking.

### Actual Behavior

Bug 1:
```
thread panicked at 'index out of bounds: the len is 4 but the index is 8'
  at lib/kv-router/src/zmq_wire.rs:479
```

Bug 2:
```
thread panicked at 'index out of bounds: the len is 0 but the index is 0'
  at lib/kv-router/src/zmq_wire.rs:436
```

### Environment

On commit `57e6a79f5` in `dynamo-runtime` @ `lib/kv-router/src/zmq_wire.rs`

### Additional Context

These functions process data from worker ZMQ events. A single malformed event from any worker crashes the entire router process. The fix would be to add bounds validation in `create_stored_blocks` before the loop, checking that `token_ids.len()` is at least the sum of `num_block_tokens`, and to validate `token_ids.len() >= kv_block_size` in `create_stored_block_from_parts`.

Found via fuzzing with cargo-fuzz / libfuzzer.

### Screenshots

_No response_
