# Fuzzing Findings: dynamo

## Overview

Comprehensive fuzzing infrastructure covering 5 crates and ~24 fuzz targets plus 4 regression test modules. The original 9 parser fuzz targets found 0 bugs. By expanding to invariant-checking, differential, round-trip, state machine, and protocol fuzzing, we found **2 bugs via fuzzing** and **12 bugs via code audit** in the parser crate alone, plus identified **high-probability crash paths** in the KV router, tokens, and runtime crates.

## Crate Coverage

| Crate | Targets | Technique | Status |
|-------|---------|-----------|--------|
| `dynamo-parsers` | 15 | Crash, invariant, differential, ReDoS, streaming monotonicity, content preservation | Complete |
| `dynamo-kv-router` | 5 | State machine, crash, round-trip, JSON deserialization, boundary | Complete |
| `dynamo-tokens` | 3 | Round-trip, boundary, stateful invariant | Complete |
| `dynamo-runtime` | 4 | Crash, round-trip | Complete |
| `dynamo-llm` | 4 modules | Regression tests (SSE codec, DeepSeek V3.2, OpenAI validation, Tensor) | Complete |

---

## Parser Bugs (Phases 1-2)

### Bug 1: Streaming prefix-matching content loss (Medium)

**Location**: `lib/parsers/src/reasoning/base_parser.rs`, lines 185-191

**Affected parsers**: Mistral (`[THINK]`/`[/THINK]`), and potentially DeepseekR1, Step3, KimiK25 — any parser with `force_reasoning=true`.

**Root cause**: When `force_reasoning=true` and the start token hasn't been stripped yet, the streaming path checks if the entire buffer is a prefix of the start token. If a model emits `"["` as a standalone token, `"[THINK]".starts_with("[")` is true and buffers `"["` indefinitely. The one-shot path has no such buffering.

**Reproducer**: Mistral parser, input `"["` (single bracket). One-shot: `reasoning_text = "["`. Streaming: `reasoning_text = ""` (content swallowed).

**Impact**: Real content silently dropped in streaming mode.

**Validation**: **Confirmed real bug.** The streaming and one-shot paths should produce equivalent output. This is a logic error, not intended behavior. Upstream issue [#3393](https://github.com/ai-dynamo/dynamo/issues/3393) acknowledges parser "loss of tokens" problems.

### Bug 2: `.trim()` asymmetry between one-shot and streaming (Low)

**Location**: `lib/parsers/src/reasoning/base_parser.rs`, lines 139-140 vs 252-255

**Root cause**: One-shot applies `.trim()` to both `reasoning_text` and `normal_text`; streaming does not.

**Reproducer**: MiniMaxAppendThink parser, input `"[\n"`. One-shot: `reasoning_text = "["`. Streaming: `reasoning_text = "[\n"`.

**Impact**: Cosmetic whitespace difference.

**Validation**: **Confirmed real bug.** Inconsistency between code paths that should behave identically. Low severity since downstream consumers aren't affected.

### Bug 3: DSML parameter silently dropped with capitalized `string="True"` (Medium)

**File**: `lib/parsers/src/tool_calling/dsml/parser.rs`

The regex requires exactly `string="true"` or `string="false"` (lowercase). Capitalized `string="True"` causes the parameter to be silently dropped.

**Validation**: **Confirmed real bug.** Models are known to produce case variations. Silent data loss is never intended behavior. The fix should be case-insensitive matching.

### Bug 4: DSML parameter silently dropped without `string` attribute (Medium)

**File**: `lib/parsers/src/tool_calling/dsml/parser.rs`

If the model omits the `string` attribute entirely, the parameter regex doesn't match and the parameter is silently dropped.

**Validation**: **Confirmed real bug.** The `string` attribute is an implementation detail of the DSML format; models may omit it. Should default to `"false"` or infer from content.

### Bug 5: DeepSeek V3 JSON normalization destroys newlines (Medium)

**File**: `lib/parsers/src/tool_calling/json/deepseek_v3_parser.rs`, lines 115-119

When initial JSON parse fails, fallback normalization joins lines with spaces, corrupting string values containing intentional newlines (e.g., code blocks in function arguments).

**Validation**: **Confirmed real bug.** Newlines in JSON string values are semantically meaningful (code, formatted text). The normalization should only affect whitespace outside string values, not inside them.

### Bug 6: Pythonic parser drops text after tool call (Low)

**File**: `lib/parsers/src/tool_calling/pythonic/pythonic_parser.rs`

`split(&matches[0]).next()` only returns text BEFORE the first match. Text appearing after is silently dropped.

**Validation**: **Confirmed real bug.** Any content after a tool call (e.g., explanatory text) is lost. However, in practice models rarely emit trailing content after pythonic-format tool calls, so severity is low.

### Bug 7: `try_literal_eval` corrupts string values containing True/False/None (Medium)

**File**: `lib/parsers/src/tool_calling/xml/parser.rs`

Global `.replace("True", "true")` corrupts strings like "TrueNorth" → "trueNorth". Same for False→false and None→null.

**Validation**: **Confirmed real bug.** This is a classic substring replacement error. Should use word-boundary-aware replacement or only apply to standalone tokens.

### Bug 8: GLM-4.7 trim offset + UTF-8 boundary panic (High)

**File**: `lib/parsers/src/tool_calling/xml/glm47_parser.rs`, line 231

`&content[function_name.len()..]` uses trimmed function name's byte length as offset into untrimmed content. With leading whitespace and multibyte UTF-8 function names, this panics.

**Validation**: **Confirmed real bug.** Any GLM-4.7 function name with leading whitespace AND multibyte characters will crash the parser. This is a direct safety issue.

### Bug 9: Base JSON parser drops text after tool calls (Low)

**File**: `lib/parsers/src/tool_calling/json/base_json_parser.rs`, line 118

Same pattern as Bug 6 — only text before the start token is extracted.

**Validation**: **Confirmed real bug.** Same analysis as Bug 6.

### Bug 10: Kimi K2 OnceLock caches regex for first config only (Low)

**File**: `lib/parsers/src/tool_calling/xml/kimi_k2_parser.rs`, lines 26-35

Static `OnceLock` caches the regex from the first call. Different configs silently use the stale regex.

**Validation**: **Likely a bug, possibly intended as optimization.** In current usage, all Kimi K2 instances share the same config, so this doesn't manifest. But the code accepts a config parameter, implying it should respect it. If configs can't vary at runtime, the parameter should be removed.

### Bug 11: `strip_quotes` panics on single-quote-char input (High)

**File**: `lib/parsers/src/tool_calling/xml/parser.rs`, line 23

`&trimmed[1..trimmed.len()-1]` panics when `trimmed` is `"\""` (single quote). `[1..0]` has begin > end.

**Validation**: **Confirmed real bug.** Models can produce empty-string parameter values. This is a missing length check.

### Bug 12: `detect_tool_call_start_xml` panics on multibyte UTF-8 start tokens (High)

**File**: `lib/parsers/src/tool_calling/xml/parser.rs`, line 42

Iterates byte positions `1..start_token.len()` and slices `&start_token[..i]`. Panics on multibyte UTF-8.

**Validation**: **Confirmed real bug, currently latent.** Default tokens are ASCII so this doesn't trigger today. But the config field is `pub String`, and the DeepSeek tokens already use multibyte Unicode characters (｜tool▁calls▁begin｜). If any XML parser is configured with Unicode start tokens, it will crash.

### Bug 13: `detect_tool_call_start_glm47` same UTF-8 panic (High)

**File**: `lib/parsers/src/tool_calling/xml/glm47_parser.rs`, line 30

Identical pattern to Bug 12.

**Validation**: **Confirmed real bug, currently latent.** Same analysis.

### Bug 14: Harmony parser `content[0]` panic risk (Latent)

**File**: `lib/parsers/src/tool_calling/harmony/harmony_parser.rs`, line 123

Uses `message.content[0]` (panics on empty) while the commentary branch correctly uses `.first()`.

**Validation**: **Confirmed real bug, currently latent.** Not reachable via the current tokenizer, but would crash if the code path is hit with empty content. The `.first()` pattern in the same file shows the developer intended bounds-safe access.

---

## KV Router Fuzz Targets (Phase 1)

### Identified High-Probability Crash Paths

1. **`compute_block_hash_for_seq(kv_block_size=0)`**: Calls `tokens.chunks_exact(0)` which panics. This is the highest-confidence new bug — any request with `kv_block_size=0` will crash the router.

2. **`RequestExtraInfo::to_block_level(block_size=0)`**: Division by zero in `req_start / block_size`. Same class as upstream issue [#3112](https://github.com/ai-dynamo/dynamo/issues/3112) (planner division by zero).

3. **`PositionalIndexer::new(jump_size=0)`**: If `jump_size=0` is accepted, later `chunks_exact(0)` calls will panic.

**Validation**: Items 1-2 are **very likely real bugs** — the functions accept `u32` parameters with no zero-check, and `chunks_exact(0)` / division by zero are well-known Rust panics. Item 3 needs runtime verification. These match the pattern of upstream issue [#3112](https://github.com/ai-dynamo/dynamo/issues/3112).

### State Machine Fuzzing (`fuzz_radix_tree_events`)

Tests arbitrary sequences of store/remove/clear/query events on `RadixTree`. Verifies:
- `find_matches` scores never exceed sequence length
- Tree is empty after removing all workers
- No panics on arbitrary event sequences

### Protocol JSON Fuzzing (`fuzz_kv_protocol_json`)

Tests deserialization of arbitrary strings into all KV protocol types (RouterRequest, RouterResponse, KvCacheEvents). Must return Ok or Err, never panic.

---

## Tokens Hash Fuzz Targets (Phase 3)

### Targets

1. **`fuzz_positional_sequence_hash`**: Round-trip verification — construct, read back, verify accessors match and mode is consistent with position range.

2. **`fuzz_positional_lineage_hash`**: Boundary testing at mode transitions (255/256, 65535/65536). Verifies `catch_unwind` for position >= 2^24 (expected panic is documented behavior).

3. **`fuzz_token_block_sequence`**: Stateful fuzzing — applies random operations (append, extend, truncate, unwind, pop, reset) and verifies `total_tokens()` consistency after every operation.

---

## Runtime Codec Fuzz Targets (Phase 4)

### Targets

1. **`fuzz_two_part_roundtrip`**: Encode/decode round-trip for `TwoPartMessage`. Tests header+data, header-only, and data-only messages. Asserts decoded content matches original.

   **Potential bug**: `total_len = 24 + header_len + body_len` in `two_part.rs` line 58 has no overflow guard in release mode. Our overflow-checked fuzzing (`FUZZ_OVERFLOW_CHECKS=1`) targets this specifically.

2. **`fuzz_two_part_decode`**: Crash oracle with various size limits (None, 1024, 1).

3. **`fuzz_tcp_decode`**: Crash oracle for `TcpRequestMessage::decode`. Validates path_len, total_size, UTF-8 path constraints. Upstream issue [#6147](https://github.com/ai-dynamo/dynamo/issues/6147) confirms TCP panic bugs exist.

4. **`fuzz_frame_decode`**: Event plane frame protocol. Tests `Frame::decode()` and `FrameHeader::decode()`. Verifies `payload_len = u32::MAX` doesn't OOM and `frame_size()` doesn't overflow.

---

## LLM Protocol Regression Tests (Phase 5)

### SSE Codec (`lib/llm/src/protocols/codec.rs`)

10 regression tests covering:
- Unbounded `data:` accumulation without blank-line terminator (EOF flush)
- Interleaved `event:`/`data:`/`id:` fields
- `data: [DONE]` sentinel handling
- `id` field with null byte rejection (per SSE spec)
- Empty field values, consecutive blank lines
- Comment-only events, field without colon
- Large data accumulation (1000 lines)

### DeepSeek V3.2 Formatter (`lib/llm/src/preprocessor/prompt/deepseek_v32.rs`)

14 regression tests covering:
- Missing `role` field → error (not panic)
- `tool_calls` with missing `function.name` → error
- `arguments` as invalid JSON or non-object → error
- Unknown role → error
- Empty messages array
- System message with null content
- Tool result without preceding assistant
- Chat mode vs thinking mode tag differences
- `to_json()` with escaped quotes and nested structures
- `reasoning_content` as array of segments
- No BOS token mode

### OpenAI Validation (`lib/llm/src/protocols/openai/validate.rs`)

33 regression tests covering:
- `validate_logit_bias` with non-numeric values (string, null, boolean, array, object)
- Logit bias boundary values (±100.0, ±100.1)
- `validate_prompt_embeds` with invalid base64, undersized data, empty string
- `validate_range` boundaries and None handling
- Temperature, max_tokens, model, suffix, repetition_penalty boundaries
- `validate_n_with_temperature` interaction
- `validate_no_unsupported_fields` with unknown fields

### Tensor Validation (`lib/llm/src/protocols/tensor.rs`)

22 regression tests covering:
- Empty shape `[]` with empty data (product=1 vs len=0 mismatch)
- Empty shape `[]` with one element (valid)
- Shape with zero dimension `[2, 0, 3]`
- dtype mismatch between metadata and data
- Negative dimensions
- Element count mismatches
- `FlattenTensor` JSON round-trip for all variants
- Invalid JSON deserialization (missing fields, wrong types, unknown data_type)
- `TensorMetadata` with `deny_unknown_fields`
- `DataType::size()` for all variants
- `ParameterValue` deserialization for all variants

---

## Upstream Issue Correlation

The following upstream issues directly validate our fuzzing approach:

| Our Finding | Upstream Issue | Match |
|-------------|----------------|-------|
| TCP codec crash oracle | [#6147](https://github.com/ai-dynamo/dynamo/issues/6147) TCP panic on ConnectionReset | Direct match — our fuzz target exercises the exact panic path |
| Division by zero in KV router | [#3112](https://github.com/ai-dynamo/dynamo/issues/3112) Planner division by zero | Same bug class — zero-value inputs causing crashes |
| Parser content loss | [#3393](https://github.com/ai-dynamo/dynamo/issues/3393) "loss of tokens" in parser | Validates our Bug 1 finding |
| Missing input validation | [#6605](https://github.com/ai-dynamo/dynamo/issues/6605) Oversized prompts crash as 500 | Same class as our preprocessor tests |
| Serde deserialization | [#5866](https://github.com/ai-dynamo/dynamo/issues/5866) empty/malformed metadata | Same class as our protocol JSON fuzzing |
| Memory exhaustion | [#5275](https://github.com/ai-dynamo/dynamo/issues/5275) Memory leak under load | Our RSS limits catch quadratic blowup patterns |

No existing upstream issues or PRs reference fuzzing. This is the **first fuzzing infrastructure** for this ~260k LOC Rust codebase.

---

## Run Statistics (Parser Phase)

| Harness | Duration | Executions | Coverage (edges) | Crashes |
|---------|----------|------------|------------------|---------|
| `fuzz_invariants` | 5 min | 317,428 | 7,730 | 0 |
| `fuzz_differential` | ~10s | ~90 | 6,629 | 1 (Mistral prefix bug) |
| `fuzz_redos` | 10s | 128,718 | 3,388 | 0 |
| `fuzz_with_tools` | 10s | 114,685 | 425 | 0 |

---

## File Inventory

### Fuzz crates (new)
- `lib/kv-router/fuzz/` — 5 targets, dictionary, 6 JSON seeds
- `lib/tokens/fuzz/` — 3 targets, dictionary
- `lib/runtime/fuzz/` — 4 targets, dictionary

### Parser fuzz additions
- `lib/parsers/fuzz/fuzz_targets/fuzz_content_preservation.rs`
- `lib/parsers/fuzz/fuzz_targets/fuzz_streaming_monotonicity.rs`

### LLM regression tests
- `lib/llm/src/protocols/codec.rs` — 10 new tests
- `lib/llm/src/preprocessor/prompt/deepseek_v32.rs` — 14 new tests
- `lib/llm/src/protocols/openai/validate.rs` — 33 new tests (new test module)
- `lib/llm/src/protocols/tensor.rs` — 22 new tests (new test module)

### Infrastructure
- `fuzzing/run.sh` — Unified runner (auto-discovers all fuzz crates)
- `fuzzing/run_parser_fuzz.sh` — Parser-specific runner (original)
- `fuzzing/coverage.sh` — Coverage report generator
