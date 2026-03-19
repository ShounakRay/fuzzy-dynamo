# Discovery: Worker Selector Panics on Zero Block Size

## What's the bug?

This bug is actually two bugs in one, both triggered by the same input: `block_size = 0`. They illustrate two different flavors of "division by zero" that behave very differently.

The first is **integer division by zero**. The worker selector computes how many KV cache blocks a request needs: `isl.div_ceil(block_size)`. This is integer math -- dividing the input sequence length by the block size, rounding up. When `block_size` is 0, Rust panics immediately with "attempt to divide by zero." The program crashes. This is the loud, obvious failure.

The second is **floating-point division by zero**, which is sneakier. Later in the same function, the code computes `prefill_tokens / (block_size as f64)`. Floating-point math follows a standard called IEEE 754, which says that dividing by zero does *not* crash -- instead, it produces a special value called "infinity." Infinity is technically a valid floating-point number, but it poisons every calculation it touches. When infinity enters the softmax function (used to convert scores into probabilities), the result is NaN ("Not a Number") -- another special value that means "this computation went wrong." NaN propagates silently through all subsequent math, producing meaningless scheduling decisions without any error message.

The integer panic crashes the server immediately. The float infinity would silently corrupt scheduling decisions, potentially sending requests to the wrong workers. In a production inference server, the first bug causes downtime; the second would cause degraded performance that is extremely hard to diagnose.

## When does this happen in real life?

This bug triggers when the KV cache scheduler tries to select a worker for a request, but the configured `block_size` is 0. The scenarios are similar to Bug 1 (block hash zero panic):

- **Misconfigured deployment** — a model deployed with `kv_block_size: 0` in its configuration will crash the scheduler on the first incoming request
- **Dynamic reconfiguration** — if block size is read from a configuration service and temporarily returns 0 (e.g., during a rollout or config propagation delay), the scheduler crashes

The crash has two effects: (1) the immediate integer division-by-zero panic kills the router, and (2) even if the panic were caught, the float division by zero produces infinity values that propagate through the softmax scoring, causing the scheduler to make nonsensical worker selections (NaN scores). So even a "graceful" handling of the panic wouldn't fix the underlying logic error.

## How we found it

### The fuzzing approach

We wrote a property-based fuzzer (`fuzz_worker_selector_div.rs`) that uses Rust's `Arbitrary` derive to generate structured scheduling scenarios: block sizes, worker counts, ISL token counts, temperature values, overlap scores, and data-parallel configurations. It constructs a realistic `SchedulingRequest` and calls `select_worker`. The target validates that the selected worker actually exists in the worker map.

### What the fuzzer did

The fuzzer generated a `FuzzInput` with `block_size = 0`, along with valid worker configurations and overlap scores. The target passed `block_size = 0` to `selector.select_worker()`. At selector.rs line 113, the function computed `isl.div_ceil(0)` and Rust panicked with "attempt to divide by zero."

After discovering the integer panic, the fuzz target was updated to filter `block_size = 0` as a known bug, allowing it to continue exploring deeper code paths and finding other bugs in the scheduling logic.

### Why traditional testing missed this

The worker selector is tested with realistic configurations where block size is always a positive power of two (like 16, 32, or 64). Zero is not a meaningful block size for any GPU, so it is never used in tests. But configuration values can be wrong -- a missing field defaulting to zero, a typo in a config file, or a newly registered worker with uninitialized state.

## The fix

Add validation at the top of `select_worker`: if `block_size == 0`, return an error immediately. This prevents both the integer panic and the silent float corruption in a single check.

## Fuzzing technique

**Strategy:** Property-based with known-bug filtering
**Target:** `fuzz_worker_selector_div.rs`
**Crate:** `lib/kv-router/fuzz`
**Run command:** `cd lib/kv-router/fuzz && ~/.cargo/bin/cargo +nightly fuzz run fuzz_worker_selector_div -- -max_total_time=60`
