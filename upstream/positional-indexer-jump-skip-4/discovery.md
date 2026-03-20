# Discovery: PositionalIndexer Jump Optimization Skips Removed Blocks

## What's the bug?

NVIDIA Dynamo's KV cache router maintains an index of which model layers (called "blocks") are cached on which GPU worker. When a new inference request arrives, the router queries this index to find the worker that already has the most relevant blocks cached, so it can reuse them instead of recomputing. The `PositionalIndexer` is one of three implementations of this index.

To speed up queries, the PositionalIndexer uses a "jump optimization" -- instead of checking every single position in a sequence, it checks every 32nd position. If a worker is present at position 0 and also at position 32, the optimizer assumes the worker is present at every position in between. Think of it like checking attendance in a classroom by only looking at every third row: if row 1 and row 4 both have a student, you assume rows 2 and 3 do too. This shortcut works perfectly as long as students only arrive and never leave. But if someone in row 2 quietly walks out, the jump check still sees rows 1 and 4 occupied and incorrectly reports full attendance.

That is exactly what happens here. When a block is removed from the middle of a cached sequence (say, due to memory pressure on a GPU), the blocks after it become "orphans" -- they still exist in the flat index even though their predecessor is gone. The jump optimization leaps over the gap and reports an inflated match score. In production, this means the router sends requests to a worker that appears to have a full cache hit but actually has a gap, forcing expensive recomputation and degrading inference latency.

The other two index implementations (RadixTree and ConcurrentRadixTree) use a tree structure where removing a parent node automatically disconnects all children, so they never produce orphan blocks and always return correct scores.

## When does this happen in real life?

This bug affects KV cache routing accuracy after blocks are evicted. In a production inference server:

- **KV cache eviction** is routine — when GPU memory fills up, the least-recently-used KV cache blocks are evicted to make room for new requests. Each eviction triggers a "remove" event in the router's indexer
- **After eviction, new requests are misrouted** — the router thinks a worker still has a full sequence cached (score 3) when in fact a middle block was evicted (real score should be 1). The request is sent to a worker that has to recompute the missing blocks, wasting GPU time
- **The impact scales with traffic** — under high load, evictions happen more frequently, and each misrouted request means unnecessary prefill computation. This silently degrades throughput without any errors or warnings

Operators would see higher-than-expected prefill latency and lower throughput, but no obvious cause — the routing decisions look normal in logs, they're just wrong.

## How we found it

### The fuzzing approach

The fuzz target `fuzz_triple_differential` in `lib/kv-router/fuzz` uses a technique called *differential fuzzing*. Instead of checking one implementation against a specification, it runs the same operations against all three index implementations and asserts that their results match. This is powerful because you do not need to know the correct answer in advance -- you just need two implementations that should agree.

The target parses raw bytes into a sequence of up to 16 operations: Store (add blocks to a worker), Remove (delete a specific block), Clear (wipe a worker), and Query (look up match scores). Each operation is encoded in 2-5 bytes, so the fuzzer can explore a huge space of operation sequences with small inputs. All three implementations -- RadixTree, ConcurrentRadixTree, and PositionalIndexer -- receive identical operations, and the target asserts that query results match across all three.

### What the fuzzer did

The fuzzer generated a short byte sequence that decoded into three operations:

1. **Store** blocks `[6, 2, 5]` on worker 1 at positions 0, 1, and 2.
2. **Remove** the block at position 1 (the middle block, with hash 2).
3. **Query** for sequence `[6, 2, 5]`.

RadixTree and ConcurrentRadixTree both returned a score of 1: only position 0 matched, because removing the block at position 1 broke the prefix chain. But PositionalIndexer returned a score of 3 (full match). Its jump optimization jumped from position 0 directly to position 2 -- since the query only had 3 elements and the jump size is 32, `next_pos = min(0 + 32, 2) = 2`. Both endpoints looked active, so it assumed everything in between was fine. The assertion `r_radix.scores == r_positional.scores` failed, and the fuzzer saved the crashing input as `crash-33160f5734e5d06e6af9cabf3030ca138df58123`.

### Why traditional testing missed this

Existing unit tests only tested append-only sequences -- blocks were stored and queried, but never removed between operations. The specific combination of store, remove, and query that breaks the jump invariant was never exercised.

## The fix

The fix is to either cascade removals (when removing a block at position P, also remove all blocks at positions greater than P for that worker) or to fall back to a linear scan after any remove operation has occurred, bypassing the jump optimization.

## Fuzzing technique

**Strategy:** Triple differential (RadixTree vs ConcurrentRadixTree vs PositionalIndexer)
**Target:** `fuzz_triple_differential.rs`
**Crate:** `lib/kv-router/fuzz`
**Run command:** `cd lib/kv-router/fuzz && ~/.cargo/bin/cargo +nightly fuzz run fuzz_triple_differential -- -max_total_time=60`
