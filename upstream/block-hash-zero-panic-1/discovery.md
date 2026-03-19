# Discovery: compute_block_hash_for_seq Panics on Zero Block Size

## What's the bug?

Division by zero is one of the most fundamental errors in computing. When you divide a number by zero, the result is mathematically undefined -- there is no number that, multiplied by zero, gives you the original value back. Different programming languages handle this differently: some return infinity, some return an error, and some crash. Rust's integer division panics (crashes) on division by zero.

The `compute_block_hash_for_seq` function in the KV router computes hash values for sequences of tokens, grouping them into blocks of a fixed size. It uses Rust's `chunks_exact(kv_block_size)` method, which divides an array into equal-sized pieces. Under the hood, `chunks_exact` divides the array length by the chunk size. When `kv_block_size` is 0, this is a division by zero, and Rust panics with "chunk size must be non-zero."

This might seem like a trivial edge case -- why would anyone set block size to zero? In a production system, configuration values come from many sources: config files, environment variables, worker registration messages. A misconfigured worker reporting `kv_block_size = 0`, or a default value that was never properly initialized, would crash the router on the very first hash computation. The crash takes down the entire routing layer, not just the one misconfigured worker.

## When does this happen in real life?

This bug triggers when a worker reports a `kv_block_size` of 0 to the router. In practice:

- **Misconfigured worker** — if a new model is deployed with an incorrect KV cache configuration (block size set to 0 by accident or through a missing config default), the first cache event from that worker crashes the router
- **Deserialization default** — if the block size field is missing from a ZMQ message and the deserialization code defaults to 0 rather than failing, the router silently receives an invalid configuration
- **Testing environments** where placeholder configurations are used without proper validation

The crash happens in the router process, not the worker. A single misconfigured worker can crash the router that serves all workers, creating a cascade failure.

## How we found it

### The fuzzing approach

We wrote a property-based fuzzer (`fuzz_block_hash_computation.rs`) that parses raw bytes into a block size (u32) and a token array. The fuzzer tests several properties: determinism (same input always produces the same hash), LoRA name equivalence (empty LoRA string equals None), and multimodal metadata handling. Critically, the fuzz target explicitly tests the `kv_block_size = 0` case by routing it to a direct call, letting the fuzzer discover the panic.

### What the fuzzer did

The fuzzer generated input bytes where the first 4 bytes decoded to `kv_block_size = 0`. The target called `compute_block_hash_for_seq(&[1, 2, 3, 4], 0, None, None)`. Inside the function, `tokens.chunks_exact(0)` panicked immediately with "chunk size must be non-zero" at protocols.rs line 44.

### Why traditional testing missed this

Unit tests for hash computation use realistic block sizes like 16 or 64 -- values that mirror actual GPU memory configurations. Zero is not a valid block size in any real deployment, so nobody wrote a test for it. But fuzzing does not know what is "realistic" -- it tries everything, including zero, and that is exactly why it finds bugs that humans miss.

## The fix

Add an early return at the top of `compute_block_hash_for_seq`: if `kv_block_size == 0`, return an empty vector (no blocks can be formed from a zero-sized block). Alternatively, validate `kv_block_size > 0` at the configuration boundary when workers first register.

## Fuzzing technique

**Strategy:** Property-based (determinism + edge cases)
**Target:** `fuzz_block_hash_computation.rs`
**Crate:** `lib/kv-router/fuzz`
**Run command:** `cd lib/kv-router/fuzz && ~/.cargo/bin/cargo +nightly fuzz run fuzz_block_hash_computation -- -max_total_time=60`
