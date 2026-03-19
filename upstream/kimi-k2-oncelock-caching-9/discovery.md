# Discovery: KimiK2 OnceLock Caches Regex from First Config

## What's the bug?

When a program needs to use the same value over and over, a common optimization is to compute it once and reuse it. In Rust, `OnceLock` is a standard tool for this: it initializes a value on the first access and returns that cached value on every subsequent access. This is perfect for things like mathematical constants or static configuration that never changes.

The KimiK2 tool call parser uses `OnceLock` to cache a compiled regular expression (regex) -- a pattern used to find and extract tool calls from model output. The problem is that the regex is built from configuration tokens that vary between models. Different KimiK2-based models use different marker tokens like `<|tool_call_begin|>` or `[CALL_START]` to denote where tool calls start and end. The regex needs to match these specific tokens.

Because `OnceLock` initializes exactly once per process, whichever model configuration happens to be used first "wins." Its tokens get baked into the regex permanently. Every subsequent model with different tokens silently gets the wrong regex -- the parser returns zero tool calls with no error or warning. In a multi-model serving scenario, this means only the first model loaded would parse tool calls correctly. All others would silently fail.

This is particularly dangerous because there is no crash or error message. The parser just quietly returns empty results, making it look like the model simply did not produce any tool calls. An engineer debugging this in production would have a very hard time tracing the problem back to initialization order.

## When does this happen in real life?

This bug affects multi-model serving deployments:

- **Serving multiple KimiK2 variants** — if an inference server hosts both a standard Kimi-K2 model and a fine-tuned variant that uses different token markers (e.g., different `<tool_call_begin>` tokens), only the first model loaded will parse tool calls correctly. The second model's tool calls will silently fail to parse — returning zero tool calls instead of the expected results
- **Model hot-swapping** — if an operator swaps one KimiK2 model for another with different config tokens without restarting the server process, the cached regex from the old model persists, breaking the new model's tool call parsing
- **A/B testing** — running two model variants side-by-side in the same process means one always gets the wrong regex

The failure is silent — no crash, no error log. Tool calls from the second model simply aren't detected. Users see the model "ignoring" their tool-use requests, which is extremely confusing to debug.

## How we found it

### The fuzzing approach

The fuzz target `fuzz_parser_semantic.rs` in `lib/parsers/fuzz` uses a **semantic round-trip oracle** strategy. Instead of just throwing random bytes at parsers and hoping for crashes, it embeds known-valid tool calls inside fuzz-controlled surrounding text. The fuzzer generates a random string, splits it at a random position into a prefix and suffix, and wraps a well-formed tool call in between. Then it verifies that the parser correctly extracts the function name and arguments.

The target covers five parser types (XML, Pythonic, DSML, Basic JSON, DeepSeek V3), selecting which one to test based on a fuzz-controlled byte. For the KimiK2 case specifically, testing with different parser configurations exposes whether the cached regex is rebuilt.

### What the fuzzer did

The semantic test first ran with a default KimiK2 configuration, which cached the regex in the static `OnceLock`. The regex was compiled with the default tokens like `<|tool_call_begin|>`. When a subsequent test case used a different configuration with different marker tokens, the `OnceLock` returned the already-cached regex from the first configuration. The new tokens did not match the old regex pattern, so the parser returned zero tool calls -- but the oracle expected at least one, triggering an assertion failure.

### Why traditional testing missed this

Unit tests always use the same default configuration in isolation. No existing test exercised a multi-config scenario where different configurations are used within the same process lifetime.

## The fix

Replace `OnceLock` with either no caching (regex compilation is cheap for small patterns) or a config-aware cache like a `HashMap` keyed by the configuration tokens, so each unique configuration gets its own compiled regex.

## Fuzzing technique

**Strategy:** Semantic round-trip oracle
**Target:** `fuzz_parser_semantic.rs`
**Crate:** `lib/parsers/fuzz`
**Run command:** `cd lib/parsers/fuzz && ~/.cargo/bin/cargo +nightly fuzz run fuzz_parser_semantic -- -max_total_time=60`
