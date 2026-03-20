# Discovery: Stored Blocks Out-of-Bounds Panic on Mismatched Sizes

## What's the bug?

When a program needs to read from an array (or "slice" in Rust), it uses an index -- a number that says "give me the item at position N." If N is larger than the array's length, you are reading past the end of the array, into memory that belongs to something else. In C, this silently reads garbage data (or worse, lets an attacker read secrets from memory). Rust prevents this: any out-of-bounds access immediately crashes the program with a panic.

The KV router in NVIDIA Dynamo manages a cache of key-value blocks that GPU workers use during inference. When a worker finishes computing a block, it sends a message over ZMQ (a network messaging library) saying "I stored N blocks, here are their hashes, and here are the token IDs." The router's `create_stored_blocks` function trusts that the token array is long enough for the claimed block count. If a message says "I have 2 blocks of size 4" but only provides 4 tokens instead of 8, the function's loop tries to read tokens 4 through 7, which do not exist. Rust panics.

There is a second variant in `create_stored_block_from_parts`: if the token array is shorter than one block size, `chunks_exact()` produces zero chunks, and then the code indexes into an empty result at position [0], which also panics.

Both functions process data arriving over the network from worker processes. A single malformed message -- whether from a buggy worker or an attacker -- crashes the entire router process, taking down inference for all users.

## When does this happen in real life?

This bug triggers when the KV cache router receives a malformed block storage event from a worker over ZMQ (a network messaging protocol). In practice:

- **A worker has a bug** that causes it to report more blocks than it actually has tokens for — for example, claiming 4 blocks of 16 tokens each (64 tokens needed) but only sending 32 tokens of actual data
- **Network corruption** alters the block count or token array in transit, creating a mismatch between the header metadata and the payload
- **A malicious actor** sends crafted ZMQ messages to the router to crash it (denial of service) — this only requires knowing the ZMQ endpoint address

The router process crashes immediately, taking down KV cache routing for all models served by that router instance. Since this is triggered by network input from workers, it can happen without any user interaction.

## How we found it

### The fuzzing approach

We wrote a property-based fuzzer (`fuzz_stored_blocks.rs`) that generates random configurations: a KV block size (0-32), a number of blocks (1-7), a token count (0-63), and token data parsed from raw bytes. It synthesizes block hashes and calls both `create_stored_blocks` and `create_stored_block_from_parts`. After discovery, the fuzz target was updated to filter known-bug inputs (mismatched sizes and block_size=0) so it could continue exploring deeper code paths without ASAN aborting on the known crash.

### What the fuzzer did

The fuzzer generated a configuration with 2 blocks of size 4 but only 4 tokens (half of the 8 needed). The `create_stored_blocks` function entered its loop, processed the first block using tokens 0-3 successfully, then tried to slice tokens 4-7. The array only had 4 elements (indices 0-3), so Rust panicked with "index out of bounds: the len is 4 but the index is 8" at zmq_wire.rs line 479.

### Why traditional testing missed this

Tests for this function use correctly-formed inputs where the token array always has exactly the right number of elements. The mismatch scenario only happens when data arrives from an external source (a ZMQ message from a worker) where the sizes are not guaranteed to be consistent -- something unit tests do not simulate.

## The fix

Add bounds validation at the top of `create_stored_blocks`, checking that `token_ids.len()` is at least the sum of `num_block_tokens` before entering the loop. Similarly, validate `token_ids.len() >= kv_block_size` in `create_stored_block_from_parts`. Return an error or empty result instead of panicking.

## Fuzzing technique

**Strategy:** Property-based with known-bug filtering
**Target:** `fuzz_stored_blocks.rs`
**Crate:** `lib/kv-router/fuzz`
**Run command:** `cd lib/kv-router/fuzz && ~/.cargo/bin/cargo +nightly fuzz run fuzz_stored_blocks -- -max_total_time=60`
