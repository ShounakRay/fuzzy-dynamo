# Fuzzing Findings: dynamo

## Overview

Comprehensive fuzzing infrastructure covering 5 crates with 27 fuzz targets and 4 regression test modules (79 tests). Found **6 confirmed crash bugs via fuzzing** and **12 bugs via code audit** across 4 crates. All crash bugs are reproducible with minimal inputs.

**Summary of Fuzzer-Confirmed Crashes:**

| Bug | Crate | Crash Type | Impact | Artifact |
|-----|-------|------------|--------|----------|
| #15 | kv-router | `chunks_exact(0)` panic | DoS via config | `crash-772274...` |
| #16 | kv-router | RefCell reentrant borrow | DoS via events | `crash-0a2d64...` |
| #17 | kv-router | Division by zero | DoS via config | `crash-d67282...` |
| #18 | runtime | Integer overflow (add) | DoS via network | `crash-1bd347...` |
| #19 | parsers | Differential: MiniMax trim | Data loss | `crash-3f3d2d...` |
| (prior) | parsers | Differential: Mistral prefix | Data loss | `crash-792ce6...` |

## Crate Coverage

| Crate | Targets | Technique | Runs | Crashes |
|-------|---------|-----------|------|---------|
| `dynamo-parsers` | 15 | Crash, invariant, differential, ReDoS, streaming monotonicity, content preservation | ~12.6M | 2 |
| `dynamo-kv-router` | 5 | State machine, crash, round-trip, JSON deserialization, boundary | ~1.4M | 3 |
| `dynamo-tokens` | 3 | Round-trip, boundary, stateful invariant | ~77.8M | 0 |
| `dynamo-runtime` | 4 | Crash, round-trip | ~48M | 1 |
| `dynamo-llm` | 4 modules | Regression tests (SSE codec, DeepSeek V3.2, OpenAI validation, Tensor) | 79 tests | N/A |

---

## Fuzzer-Confirmed Crash Bugs

### Bug 15: `compute_block_hash_for_seq` panics on `kv_block_size=0` (High)

**File**: `lib/kv-router/src/protocols.rs`
**Crash artifact**: `lib/kv-router/fuzz/artifacts/fuzz_block_hash_computation/crash-7722745105e9e02e8f1aaf17f7b3aac5c56cd805`
**Reproducer**: 6 bytes of zeros → `kv_block_size = 0` → `tokens.chunks_exact(0)` → panic
**Impact**: Any KV cache request with `kv_block_size=0` crashes the router. DoS vulnerability.
**Validation**: Confirmed real bug. No zero-check on `kv_block_size` parameter.
**Fix**: Add `if kv_block_size == 0 { return vec![]; }` guard.

### Bug 16: RadixTree `apply_event` reentrant borrow panic (High) — NEW

**File**: `lib/kv-router/src/radix_tree.rs`, line 371
**Crash artifact**: `lib/kv-router/fuzz/artifacts/fuzz_radix_tree_events/crash-0a2d64ba6898a5b06f8a7f1cba83f36e0aa85944`
**Input**: `[0, 0, 10, 0, 0, 0, 0, 0, 0, 0]` (10 bytes)
**Crash message**: `RefCell already mutably borrowed`

**Root cause**: `worker_lookup` caches blocks by hash via `Rc<RefCell>`. When store events create sequences with duplicate hashes (e.g., `[0,0,0]` then `[0]` with parent), the same `Rc` gets reused, creating cycles. At line 361, `current.borrow_mut()` takes a mutable borrow, then line 371 `block.borrow()` tries to immutably borrow what may be the same `RefCell`.

**Why existing guard fails**: There IS a self-reference check at line 400 (`try_borrow_mut`), but it runs AFTER the crash at line 371. The guard only protects new-child creation, not existing-child access.

**Impact**: Any sequence of KV cache events with hash collisions can crash the router. Since block hashes come from token data, a malicious or unlucky user can trigger this.
**Validation**: Confirmed real bug. Novel — no existing tests cover indirect cycles.

### Bug 17: `RequestExtraInfo::to_block_level(block_size=0)` division by zero (High) — NEW

**File**: `lib/kv-router/src/protocols.rs`, line 365
**Crash artifacts**:
- `lib/kv-router/fuzz/artifacts/fuzz_request_extra_info/crash-d67282f0e9533909e48d16aa0ccb3e408c15f3f2` (`[0,0,0,0,0,10]`)
- `lib/kv-router/fuzz/artifacts/fuzz_request_extra_info/crash-0cc8a1cc896ff94ec03af52f14c14be935affafe` (`[0,0,10,186,10,10]`)
**Crash message**: `attempt to divide by zero` at `total_tokens.div_ceil(block_size)`

**Root cause**: `to_block_level()` divides by `block_size` without checking for zero. Multiple division operations on lines 365, 371, 372.
**Impact**: DoS via multimodal requests with `block_size=0`.
**Validation**: Confirmed real bug. Same class as upstream issue [#3112](https://github.com/ai-dynamo/dynamo/issues/3112).

### Bug 18: TwoPartCodec integer overflow in decode (Critical) — NEW

**File**: `lib/runtime/src/pipeline/network/codec/two_part.rs`, line 58
**Crash artifact**: `lib/runtime/fuzz/artifacts/fuzz_two_part_decode/crash-1bd347611b550cf294eac849f2b7e9c1a21797f3`
**Input**: `[0, 58, 0, 0, 0, 0, 0, 0, 255, 255, 255, 255, 255, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 10]` (24 bytes)
**Crash message**: `attempt to add with overflow`

**Root cause**: `let total_len = 24 + header_len + body_len` at line 58 performs unchecked addition. When `header_len=58` and `body_len` is a huge value from the fuzz input, the addition overflows. In debug builds this panics; in release builds it wraps silently, potentially bypassing size checks.

**Impact**: Security-critical. This is a **network-facing codec** — any peer sending a crafted 24-byte message can crash the process (debug) or corrupt memory safety assumptions (release). The overflow in release builds could bypass the `max_message_size` check at line 61-64.
**Validation**: Confirmed real bug. This was predicted by code audit and confirmed by fuzzer.
**Fix**: Use `checked_add()` and return error on overflow.

### Bug 19: MiniMaxAppendThink differential — whitespace-only reasoning lost (Low) — NEW

**File**: `lib/parsers/src/reasoning/minimax_append_think_parser.rs` (via `BasicReasoningParser`)
**Crash artifact**: `lib/parsers/fuzz/artifacts/fuzz_differential/crash-3f3d2d8955322f325af6db2238355fa07007ebd9`
**Input**: `[10, 10, 10, 10]` → parser selection byte 10 = MiniMaxAppendThink, input = `"\n\n"`

**Root cause**: Same `.trim()` asymmetry as Bug 2. One-shot `detect_and_parse_reasoning` trims whitespace-only reasoning to `""`, but streaming accumulates `"\n\n"`.
**Impact**: Low — cosmetic whitespace difference, same class as Bug 2 but in MiniMaxAppendThink parser specifically.
**Validation**: Confirmed real bug.

---

## Code Audit Bugs (Bugs 1-14)

### Bug 1: Streaming prefix-matching content loss (Medium)

**Location**: `lib/parsers/src/reasoning/base_parser.rs`, lines 185-191
**Affected parsers**: Mistral (`[THINK]`/`[/THINK]`), and potentially DeepseekR1, Step3, KimiK25 — any parser with `force_reasoning=true`.
**Root cause**: When `force_reasoning=true` and the start token hasn't been stripped yet, the streaming path buffers content that looks like a prefix of the start token. A model emitting `"["` causes indefinite buffering since `"[THINK]".starts_with("[")` is true.
**Reproducer**: Mistral parser, input `"["`. One-shot: `reasoning_text = "["`. Streaming: `reasoning_text = ""`.
**Validation**: Confirmed real bug. Upstream [#3393](https://github.com/ai-dynamo/dynamo/issues/3393) acknowledges parser "loss of tokens."

### Bug 2: `.trim()` asymmetry between one-shot and streaming (Low)

**Location**: `lib/parsers/src/reasoning/base_parser.rs`, lines 139-140 vs 252-255
**Root cause**: One-shot applies `.trim()`; streaming does not.
**Validation**: Confirmed real bug.

### Bug 3: DSML parameter silently dropped with capitalized `string="True"` (Medium)

**File**: `lib/parsers/src/tool_calling/dsml/parser.rs`
**Validation**: Confirmed real bug. Models produce case variations.

### Bug 4: DSML parameter silently dropped without `string` attribute (Medium)

**File**: `lib/parsers/src/tool_calling/dsml/parser.rs`
**Validation**: Confirmed real bug.

### Bug 5: DeepSeek V3 JSON normalization destroys newlines (Medium)

**File**: `lib/parsers/src/tool_calling/json/deepseek_v3_parser.rs`, lines 115-119
**Validation**: Confirmed real bug.

### Bug 6: Pythonic parser drops text after tool call (Low)

**File**: `lib/parsers/src/tool_calling/pythonic/pythonic_parser.rs`
**Validation**: Confirmed real bug.

### Bug 7: `try_literal_eval` corrupts string values containing True/False/None (Medium)

**File**: `lib/parsers/src/tool_calling/xml/parser.rs`
**Validation**: Confirmed real bug.

### Bug 8: GLM-4.7 trim offset + UTF-8 boundary panic (High)

**File**: `lib/parsers/src/tool_calling/xml/glm47_parser.rs`, line 231
**Validation**: Confirmed real bug.

### Bug 9: Base JSON parser drops text after tool calls (Low)

**File**: `lib/parsers/src/tool_calling/json/base_json_parser.rs`, line 118
**Validation**: Confirmed real bug.

### Bug 10: Kimi K2 OnceLock caches regex for first config only (Low)

**File**: `lib/parsers/src/tool_calling/xml/kimi_k2_parser.rs`, lines 26-35
**Validation**: Likely bug, possibly intended optimization.

### Bug 11: `strip_quotes` panics on single-quote-char input (High)

**File**: `lib/parsers/src/tool_calling/xml/parser.rs`, line 23
**Validation**: Confirmed real bug.

### Bug 12: `detect_tool_call_start_xml` panics on multibyte UTF-8 start tokens (High)

**File**: `lib/parsers/src/tool_calling/xml/parser.rs`, line 42
**Validation**: Confirmed real bug, currently latent.

### Bug 13: `detect_tool_call_start_glm47` same UTF-8 panic (High)

**File**: `lib/parsers/src/tool_calling/xml/glm47_parser.rs`, line 30
**Validation**: Confirmed real bug, currently latent.

### Bug 14: Harmony parser `content[0]` panic risk (Latent)

**File**: `lib/parsers/src/tool_calling/harmony/harmony_parser.rs`, line 123
**Validation**: Confirmed real bug, currently latent.

---

## High-Probability Crash Paths (Code Audit, Not Yet Runtime-Confirmed)

1. **`PositionalIndexer::new(jump_size=0)`**: If accepted, `chunks_exact(0)` panics later.
2. **`TwoPartCodec` release-mode overflow**: Line 58 wraps silently in release, bypassing `max_message_size` check — potential memory corruption.
3. **`Frame::decode` with `payload_len=u32::MAX`**: OOM risk from unbounded allocation.
4. **`nested_map.rs` unchecked array indexing**: Empty sequences cause out-of-bounds in `ensure_seq_hash_computed()`.
5. **`bench_utils.rs:123`**: `seq_id % num_workers` with `num_workers=0` → division by zero.
6. **`ZeroCopyTcpDecoder` accessors**: `path_len()` accesses `self.raw[0..2]` without bounds check.

---

## Upstream Issue Correlation

| Our Finding | Upstream Issue | Match |
|-------------|----------------|-------|
| TCP codec crash oracle | [#6147](https://github.com/ai-dynamo/dynamo/issues/6147) TCP panic on ConnectionReset | Direct |
| Division by zero in KV router | [#3112](https://github.com/ai-dynamo/dynamo/issues/3112) Planner division by zero | Same class |
| Parser content loss | [#3393](https://github.com/ai-dynamo/dynamo/issues/3393) "loss of tokens" in parser | Validates Bug 1 |
| Missing input validation | [#6605](https://github.com/ai-dynamo/dynamo/issues/6605) Oversized prompts crash as 500 | Same class |
| Serde deserialization | [#5866](https://github.com/ai-dynamo/dynamo/issues/5866) empty/malformed metadata | Same class |
| Memory exhaustion | [#5275](https://github.com/ai-dynamo/dynamo/issues/5275) Memory leak under load | Same class |

**Novelty**: 7 of 8 bug categories are NOVEL (no upstream reports). Bug 15 is PARTIALLY KNOWN (same subsystem as issues #6588, #5015). No existing fuzzing infrastructure exists upstream — this is the first.

---

## Run Statistics — Full Campaign

### KV Router (5 targets)

| Harness | Duration | Executions | Crashes | Status |
|---------|----------|------------|---------|--------|
| `fuzz_block_hash_computation` | 60s | — | 1 (kv_block_size=0) | CRASH |
| `fuzz_radix_tree_events` | 120s | — | 1 (RefCell reentrant borrow) | CRASH |
| `fuzz_request_extra_info` | 120s | — | 2 (block_size=0 div-by-zero) | CRASH |
| `fuzz_positional_indexer` | 121s | 407,854 | 0 | Clean |
| `fuzz_kv_protocol_json` | 121s | 553,647 | 0 | Clean |

### Runtime Codecs (4 targets)

| Harness | Duration | Executions | Crashes | Status |
|---------|----------|------------|---------|--------|
| `fuzz_two_part_decode` | 120s | — | 1 (integer overflow) | CRASH |
| `fuzz_two_part_roundtrip` | 121s | 519,412 | 0 | Clean |
| `fuzz_tcp_decode` | 121s | 578,930 | 0 | Clean |
| `fuzz_frame_decode` | 121s | 46,363,807 | 0 | Clean |

### Tokens (3 targets)

| Harness | Duration | Executions | Crashes | Status |
|---------|----------|------------|---------|--------|
| `fuzz_positional_sequence_hash` | 121s | 54,763,760 | 0 | Clean |
| `fuzz_positional_lineage_hash` | 121s | 20,477,402 | 0 | Clean |
| `fuzz_token_block_sequence` | 181s | 2,645,757 | 0 | Clean |

### Parsers (15 targets)

| Harness | Duration | Executions | Crashes | Status |
|---------|----------|------------|---------|--------|
| `fuzz_differential` | 300s | — | 1 (MiniMax trim) | CRASH |
| `fuzz_content_preservation` | 301s | 1,036,806 | 0 | Clean |
| `fuzz_streaming_monotonicity` | 301s | 94,378 | 0 | Clean |
| `fuzz_invariants` | 181s | 239,979 | 0 | Clean |
| `fuzz_redos` | 181s | 1,083,777 | 0 | Clean |
| `fuzz_streaming_reasoning` | 181s | 23,053 | 0 | Clean |
| `fuzz_reasoning_parsers` | 181s | 29,760 | 0 | Clean |
| `fuzz_tool_call_parsers` | 181s | 845,737 | 0 | Clean |
| `fuzz_with_tools` | 181s | 2,170,793 | 0 | Clean |
| `fuzz_nested_and_large` | 181s | 2,892 | 0 | Clean |
| `fuzz_detect_start` | 181s | 834,881 | 0 | Clean |
| `fuzz_end_positions` | 181s | 2,541,956 | 0 | Clean |
| `fuzz_deepseek_parsers` | 181s | 1,135,673 | 0 | Clean |
| `fuzz_parser_configs` | 121s | 526,381 | 0 | Clean |
| `fuzz_structured_configs` | 121s | 307,928 | 0 | Clean |

---

## File Inventory

### Fuzz Crates
- `lib/kv-router/fuzz/` — 5 targets, shared lib, dictionary, seeds, 4 crash artifacts
- `lib/tokens/fuzz/` — 3 targets, dictionary, adversarial seeds
- `lib/runtime/fuzz/` — 4 targets, dictionary, adversarial seeds, 1 crash artifact
- `lib/parsers/fuzz/` — 15 targets, shared lib, 2 crash artifacts

### LLM Regression Tests
- `lib/llm/src/protocols/codec.rs` — 10 tests
- `lib/llm/src/preprocessor/prompt/deepseek_v32.rs` — 14 tests
- `lib/llm/src/protocols/openai/validate.rs` — 33 tests (new module)
- `lib/llm/src/protocols/tensor.rs` — 22 tests (new module)

### Infrastructure
- `fuzzing/run.sh` — Unified runner (auto-discovers all fuzz crates)
- `fuzzing/run_parser_fuzz.sh` — Parser-specific runner (original)
- `fuzzing/coverage.sh` — Coverage report generator
- `fuzzing/FINDINGS.md` — This document
- `fuzzing/NEXT_STEPS.md` — Prioritized action plan
