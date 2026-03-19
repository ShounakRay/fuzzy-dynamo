# Discovery: ConcurrentRadixTree Deadlock on Duplicate Block Hashes

## What's the bug?

A **deadlock** is one of the most feared bugs in concurrent programming. It occurs when a thread is waiting for something that will never happen -- like standing in front of a locked door, holding the only key, but the key is for the wrong side. The program does not crash, does not print an error, and does not use any CPU. It simply stops forever.

In Rust, shared data structures that need to support multiple threads often use `RwLock` (read-write lock). This lock allows either many simultaneous readers OR one exclusive writer -- but not both at the same time. A critical rule is that if a thread already holds a write lock on a particular `RwLock`, it must never try to acquire a read lock on that same `RwLock`. The write lock it holds prevents any reads (including its own), so it would wait forever for itself to finish writing.

The `ConcurrentRadixTree` in Dynamo's KV-cache router manages a tree of cached key-value blocks. When a worker reports that it has stored new blocks, the tree's `apply_stored` function walks through the blocks and inserts them using hand-over-hand locking: it holds a write lock on the parent node while reading children to find the right insertion point. The bug occurs when a store event contains **duplicate block hashes** -- two or more blocks with the same `ExternalSequenceBlockHash`. The first block creates a new node and registers it in a lookup table. The second block finds that node in the lookup table and, through the lookup indirection, inserts it as a child of itself. When the traversal then tries to read-lock this node (which it already write-locked as the current node), it deadlocks.

In production, this means a single malformed or duplicate-containing event from a worker could hang the entire KV-cache routing thread, effectively taking down the inference server's ability to route requests to cached workers. The server would appear healthy (no crash, no error log) but would stop processing new routing decisions.

## When does this happen in real life?

This bug causes the KV cache router to silently hang forever:

- **Worker sends duplicate block hashes** — this can happen when a worker's KV cache reports two blocks that hash to the same value (hash collision), or when a bug in the worker's cache management produces duplicate entries in a single store event
- **Block hash collisions** — the hashing function that produces `ExternalSequenceBlockHash` could produce collisions for different sequences, especially with short sequences or specific token patterns. While unlikely for any single event, over millions of events it becomes statistically probable
- **The failure is invisible** — the router thread hangs at 0% CPU with no error message, no crash log, and no metric spike. From the outside, the router simply stops making routing decisions. New requests pile up, latencies spike, and eventually the system appears "stuck" but healthy

This is one of the hardest bug categories to diagnose in production. There's no stack trace, no panic message, and traditional monitoring (CPU usage, error rates) shows nothing abnormal. Only thread-level debugging or deadlock detection tooling would identify the root cause.

## How we found it

### The fuzzing approach

The fuzz target `fuzz_concurrent_stress.rs` in `lib/kv-router/fuzz` uses a **multi-threaded stress testing** strategy with deadlock detection. It spawns 2 to 5 threads (controlled by the first fuzz byte), each operating on its own range of worker IDs. Each thread gets a slice of the fuzz input, which it parses into a sequence of Store, Remove, Clear, and Query operations. All threads share the same `ConcurrentRadixTree` instance via `Arc` (Rust's atomic reference-counted pointer). The main thread joins all spawned threads -- if any thread hangs due to a deadlock, the join times out and the test detects the failure.

### What the fuzzer did

The fuzzer generated input bytes that, when parsed into operations, produced a Store event where multiple blocks had the same hash value (e.g., all three blocks had `ExternalSequenceBlockHash(0)`). When this event was applied to the tree, the first block created node B1 and registered it in the worker lookup table under hash 0. The second block looked up hash 0 and found B1, then inserted B1 as a child of itself -- creating a self-referential loop. The third block acquired a write lock on B1 as the "current" node, then tried to read-lock B1 via the children list (since B1 was its own child). The write lock prevented the read lock, and the thread hung at 0% CPU indefinitely. The main thread's `join()` call detected the hang.

### Why traditional testing missed this

Deadlocks only manifest under specific data conditions (duplicate hashes in a single event) that are uncommon in normal operation. Standard tests use well-formed events with unique hashes. Duplicate block hashes can occur with malicious or buggy workers but were never considered in test scenarios.

## The fix

Add a self-reference check before acquiring the read lock (e.g., `Arc::ptr_eq(existing, &current)`) to detect when a node would lock itself, or deduplicate block hashes at the event ingestion boundary before they reach the tree.

## Fuzzing technique

**Strategy:** Multi-threaded stress testing with deadlock detection
**Target:** `fuzz_concurrent_stress.rs`
**Crate:** `lib/kv-router/fuzz`
**Run command:** `cd lib/kv-router/fuzz && ~/.cargo/bin/cargo +nightly fuzz run fuzz_concurrent_stress -- -max_total_time=60`
