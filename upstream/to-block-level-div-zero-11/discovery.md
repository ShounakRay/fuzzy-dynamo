# Discovery: to_block_level Panics on Zero Block Size

## What's the bug?

Modern AI inference servers handle more than just text -- they also process images, audio, and other media (called "multimodal" inputs). When these inputs flow through the KV cache, the system needs to convert token-level metadata (which tracks where each media object starts and ends in the token stream) into block-level metadata (which tracks which cache blocks contain parts of each media object). This conversion is what `to_block_level` does.

The conversion involves division. If a media object spans tokens 0 through 15 and the block size is 4, then it spans blocks 0 through 3 (computed as 0/4 through 15/4). Division by the block size appears three times in this function:

1. `total_tokens.div_ceil(block_size)` -- computing how many blocks exist in total
2. `req_start / block_size` -- converting a token start position to a block index
3. `(req_end - 1) / block_size` -- converting a token end position to a block index

When `block_size` is 0, all three divisions panic. This is the same class of bug as the block hash and worker selector panics -- zero block size causes division by zero. But here the impact is tripled: three separate crash sites in a single function, any one of which will take down the router.

Division by zero is an error that computers cannot recover from in integer arithmetic. Unlike floating-point math (where dividing by zero produces infinity), integer division by zero has no meaningful answer. Rust enforces this by panicking rather than producing undefined behavior. The panic is "correct" in the sense that it prevents worse outcomes, but in a server that should stay running, it is still a denial-of-service bug.

## When does this happen in real life?

This bug triggers when multimodal metadata (e.g., image embeddings in a vision-language model) needs to be converted from token-level to block-level granularity, and the block size is 0. In practice:

- **Multimodal model misconfiguration** — vision-language models like LLaVA or Qwen-VL that process images alongside text use multimodal metadata to track which token positions correspond to image embeddings. If the KV cache block size is misconfigured as 0, this conversion function panics
- **Requests without KV cache** — if a request arrives before the KV cache system is fully initialized (block size not yet propagated from worker configuration), the default value of 0 triggers the panic

This specifically affects multimodal inference workloads. Text-only requests don't call `to_block_level` and are unaffected. But in a mixed deployment serving both text and vision requests, only the vision requests crash the router.

## How we found it

### The fuzzing approach

We wrote a property-based fuzzer (`fuzz_request_extra_info.rs`) that generates block sizes, total token counts, and multimodal metadata from raw bytes. It constructs `RequestExtraInfo` objects with varying numbers of multimodal objects and offset ranges, then calls `to_block_level` and validates the output: checking that the returned block count matches expectations and that all offsets within each block are within bounds. The target explicitly routes `block_size = 0` to a direct call to let the fuzzer confirm the panic.

### What the fuzzer did

The fuzzer generated input bytes where the first two bytes decoded to `block_size = 0`. The target constructed a `RequestExtraInfo` with one multimodal object having offsets `(0, 1)` and called `info.to_block_level(0, 10)`. At protocols.rs line 375, the function computed `total_tokens.div_ceil(0)` and Rust panicked with "attempt to divide by zero."

### Why traditional testing missed this

Tests for multimodal metadata conversion use the same block sizes as the rest of the KV cache tests -- positive values that reflect actual GPU memory layouts. The `to_block_level` function is a utility function deeper in the call stack, so it receives `block_size` from several layers above. Nobody added a guard because the value is "supposed to" be validated elsewhere. Fuzzing does not care about "supposed to" -- it tests what actually happens.

## The fix

Add an early return at the top of `to_block_level`: if `block_size == 0`, return an empty vector. This is the same pattern as the fix for `compute_block_hash_for_seq` and `select_worker`. Ideally, `block_size` should also be validated once at the configuration boundary to prevent zero values from propagating through the entire system.

## Fuzzing technique

**Strategy:** Property-based with bounds checking
**Target:** `fuzz_request_extra_info.rs`
**Crate:** `lib/kv-router/fuzz`
**Run command:** `cd lib/kv-router/fuzz && ~/.cargo/bin/cargo +nightly fuzz run fuzz_request_extra_info -- -max_total_time=60`
