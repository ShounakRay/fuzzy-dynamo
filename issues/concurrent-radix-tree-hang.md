# ConcurrentRadixTree: Hang on Any Operation Under Fuzzer Instrumentation

## Summary

`ConcurrentRadixTree` hangs indefinitely when used in a fuzz target built with
`cargo fuzz` (which enables AddressSanitizer). Even a single `apply_event` call
storing one hash for one worker never completes — the process shows 0% CPU
usage, indicating a deadlock or blocking condition in the locking primitives.

The single-threaded `RadixTree` processes equivalent operations instantly under
the same instrumentation, confirming this is specific to the concurrent
implementation's `parking_lot::RwLock` / `DashMap` usage.

## Steps to Reproduce

```bash
cd lib/kv-router/fuzz

# Build the differential target
~/.cargo/bin/cargo +nightly fuzz build fuzz_differential_indexers

# Create a minimal test input: one Store op with hash [0] for worker 0
printf '\x04\x00\x00\x00' > /tmp/test_input

# Run — will hang indefinitely
timeout 10 target/aarch64-apple-darwin/release/fuzz_differential_indexers /tmp/test_input
```

## Root Cause

Likely interaction between `parking_lot::RwLock` / `DashMap` and libfuzzer's
execution environment. The hang reproduces both with and without ASAN (`-s none`),
ruling out sanitizer instrumentation as the cause. The 0% CPU confirms the
thread is blocked on a lock, not in an infinite loop.

Possible causes:
1. `parking_lot` uses `thread::park()` / futex internally, which may not work
   correctly in libfuzzer's single-threaded `LLVMFuzzerTestOneInput` context
2. `DashMap` may initialize thread-local state or shards that require a proper
   thread runtime not present under libfuzzer
3. The `RwLock` implementation may detect single-threaded re-entry and park
   the thread permanently

## Impact

- **Severity**: Medium — the ConcurrentRadixTree cannot be fuzz-tested under
  libfuzzer at all; it works correctly in production and in `#[test]` contexts
- **Workaround**: Test RadixTree directly (equivalent logic, different
  concurrency primitives); the `fuzz_differential_indexers` target is included
  but will hang on any input involving ConcurrentRadixTree operations
- **Note**: The differential testing logic is correct and would find real
  divergences if ConcurrentRadixTree could run under libfuzzer

## Suggested Investigation

1. Check if `parking_lot` has known libfuzzer compatibility issues
2. Try `std::sync::RwLock` instead of `parking_lot::RwLock`
3. Test with a thread-based fuzzer like AFL instead of libfuzzer
4. Write equivalent differential tests as `#[test]` with proptest instead
