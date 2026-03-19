# Discovery: RadixTree Underreports Overlap Scores

> **Status: FIXED** -- Resolved upstream in PRs #5973 and #6122.

## What's the bug?

A **radix tree** (also called a prefix tree or trie) is a tree data structure that stores sequences by sharing common prefixes. If you store the sequences [A, B, C] and [A, B, D], the shared prefix [A, B] is stored once in the tree, with two branches for C and D. This saves memory and makes prefix lookups fast.

In Dynamo's KV-cache router, the radix tree tracks which workers have cached which sequences of key-value blocks. When a new inference request arrives, the router needs to figure out which worker already has the most relevant blocks cached -- this is the "overlap score." A higher score means more cache hits and faster inference. The router queries the tree with the new request's block sequence and asks: for each worker, how many blocks along this sequence do they already have cached?

The bug was in the `find_matches` traversal. When walking down the tree to count matches, the code only counted one match per tree node visited, rather than tracking the total **depth** of matched blocks along the path. In a compressed radix tree, a single node can represent multiple blocks (that is the whole point of compression). So visiting one node at the root level and finding a match was reported as score 1, even when the path actually contained 3 matching blocks spanning multiple levels of the original sequence.

The practical impact is that the router would undervalue workers that have good cache coverage. Instead of preferring a worker with 3 out of 3 blocks cached (score 3), it would see that worker as having only 1 block cached (score 1) -- no better than a worker with minimal coverage. This leads to unnecessary recomputation of KV cache, wasting GPU memory and increasing inference latency.

## When does this happen in real life?

This bug (now fixed) caused suboptimal KV cache routing:

- **Requests routed to wrong workers** — the RadixTree reported lower overlap scores than the actual cache coverage, so the router didn't prefer workers that already had the relevant KV cache blocks. Requests were spread across workers instead of being directed to workers with existing cache
- **Unnecessary prefill computation** — when a request goes to a worker without cached blocks, the worker must recompute the KV cache from scratch (prefill). With correct scores, the request would have gone to a worker that already had the cache, saving GPU time
- **Higher latency, lower throughput** — the aggregate effect is slower first-token latency (more prefill) and lower overall throughput (GPU cycles wasted on redundant computation)

This bug was fixed upstream in PRs #5973 and #6122.

## How we found it

### The fuzzing approach

The fuzz target `fuzz_triple_differential.rs` in `lib/kv-router/fuzz` uses a **triple differential** strategy. It maintains three independent implementations of the same indexing interface -- `RadixTree`, `ConcurrentRadixTree`, and `PositionalIndexer` -- and applies the same sequence of Store, Remove, Clear, and Query operations to all three. After each Query, it asserts that all three implementations return identical overlap scores. The `PositionalIndexer` is a simpler position-based implementation that indexes each block by its position in the sequence, making it a reliable reference oracle.

The fuzzer generates a stream of bytes, which are parsed into a sequence of up to 16 operations. Block hashes are constrained to the range 0-7 and worker IDs to 0-1, keeping the state space small enough for the fuzzer to explore thoroughly while still exercising meaningful tree structures.

### What the fuzzer did

The fuzzer generated a sequence of Store operations that built up cached block sequences on worker 0. Then it issued a Query for the sequence [6, 0, 5]. The `PositionalIndexer` correctly reported score 3 (all three blocks matched), but the `RadixTree` reported score 1. The assertion `r_radix.scores == r_positional.scores` failed, producing the crash artifact `crash-f79bf57988d52dd105b554a1f070a6c593284171`.

Three independent crash inputs all exhibited the same pattern: 3-block queries where the `RadixTree` reported score 1 while `PositionalIndexer` reported score 3. The consistency across different inputs confirmed this was a systematic traversal bug, not a one-off edge case.

### Why traditional testing missed this

The bug only manifests after specific sequences of Store operations that create particular tree structures. Hand-written tests used simple, short sequences where the tree happened to have one block per node, making the counting error invisible.

## The fix

The `find_matches` traversal was rewritten to properly track `matched_depth` along tree paths, accumulating the total number of matching blocks rather than counting tree nodes. Fixed upstream in PRs #5973 and #6122.

## Fuzzing technique

**Strategy:** Triple differential
**Target:** `fuzz_triple_differential.rs`
**Crate:** `lib/kv-router/fuzz`
**Run command:** `cd lib/kv-router/fuzz && ~/.cargo/bin/cargo +nightly fuzz run fuzz_triple_differential -- -max_total_time=60`
