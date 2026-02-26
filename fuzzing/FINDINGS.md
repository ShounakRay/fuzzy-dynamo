# Fuzzing Findings: dynamo-parsers

## Overview

Advanced fuzzing of the `dynamo-parsers` library beyond crash-only detection. The original 9 fuzz targets with ~25k corpus inputs found 0 bugs. By adding invariant-checking, differential, ReDoS, and tool-definition-aware harnesses, we found 2 real bugs within minutes.

## Approach

### Why the original fuzzing found nothing

The original 9 harnesses only check "doesn't crash" — they call parser functions and discard the result. This misses:
- Logic bugs (wrong output)
- Divergences between code paths (streaming vs one-shot)
- Invariant violations (invalid JSON in arguments, empty function names)
- Performance bugs (ReDoS)
- Untested code paths (type coercion with tool definitions)

### What we added

| Harness | Technique | What it checks |
|---------|-----------|----------------|
| `fuzz_invariants` | Assertion-based | End positions in bounds, non-empty function names, valid JSON arguments, normal text doesn't exceed input |
| `fuzz_differential` | Differential | Streaming vs one-shot reasoning parsers produce identical output |
| `fuzz_redos` | Timeout-based | Regex-heavy parsers (pythonic, XML, GLM47) don't hang on adversarial input |
| `fuzz_with_tools` | Coverage-driven | `convert_param_value()` and `coerce_value()` type coercion with generated `ToolDefinition` structs |

### Supporting infrastructure

- **Targeted seed corpus** (`fuzz/seeds/`): 4 new seed files targeting specific code paths identified in audit (`strip_quotes`, glm47 trim/slice, kimi_k2 empty sections, pythonic regex backtracking)
- **Token dictionary** (`fuzz/parser_tokens.dict`): 70+ tokens covering all parser formats, enabling structure-aware mutations
- **Dictionary support** in `run_parser_fuzz.sh` via `FUZZ_DICT` env var
- **Coverage script** (`fuzzing/coverage.sh`): generates per-target HTML coverage reports

## Bugs Found

### Bug 1: Streaming prefix-matching content loss (Medium severity)

**Location**: `lib/parsers/src/reasoning/base_parser.rs`, lines 185-191

**Affected parsers**: Mistral (`[THINK]`/`[/THINK]`), and potentially DeepseekR1, Step3, KimiK25 (`<think>`/`</think>`) — any parser with `force_reasoning=true`.

**Root cause**: When `force_reasoning=true` and the start token hasn't been stripped yet, the streaming path checks if the entire buffer is a prefix of the start token:

```rust
if !self.stripped_think_start
    && self._in_reasoning
    && !current_text.is_empty()
    && self.think_start_token.starts_with(current_text.as_str())
{
    break; // buffer the content, wait for more data
}
```

If a model emits `"["` as a standalone token, the streaming path sees that `"[THINK]".starts_with("[")` is true and buffers `"["` indefinitely, waiting for more data to complete the tag. If the next token is something other than `"THINK]"`, the buffered `"["` may be silently lost or misattributed.

The one-shot path has no such buffering — it correctly treats `"["` as reasoning content.

**Reproducer**: Mistral parser, input `"["` (single bracket character).
- One-shot: `reasoning_text = "["`, `normal_text = ""`
- Streaming: `reasoning_text = ""`, `normal_text = ""` (content swallowed)

**Impact**: Real content silently dropped in streaming mode when a token happens to be a prefix of the think-start marker. In practice this requires a tokenizer to emit `"["` as a separate token when using Mistral's `[THINK]` format, which is plausible.

**Fix direction**: When `force_reasoning=true`, the start token should be considered already consumed (the model's tokenizer ate it). The prefix-buffering check should either be skipped entirely when `force_reasoning=true`, or should require a minimum overlap threshold (like the `ol >= 2` check used elsewhere in the same function).

### Bug 2: `.trim()` asymmetry between one-shot and streaming (Low severity)

**Location**: `lib/parsers/src/reasoning/base_parser.rs`, line 139-140 vs line 252-255

**Affected parsers**: All `BasicReasoningParser`-based parsers (all 11 reasoning parser types).

**Root cause**: The one-shot `detect_and_parse_reasoning` applies `.trim()` to both `reasoning_text` and `normal_text` before returning:

```rust
let reasoning_text = reasoning_parts.join("").trim().to_string();  // line 139
let normal_text = normal_parts.join("").trim().to_string();        // line 140
```

The streaming `parse_reasoning_streaming_incremental` returns raw accumulated text without trimming:

```rust
ParserResult {
    normal_text: accumulated_normal,      // line 253 — not trimmed
    reasoning_text: accumulated_reasoning, // line 254 — not trimmed
}
```

**Reproducer**: MiniMaxAppendThink parser, input `"[\n"`.
- One-shot: `reasoning_text = "["` (newline trimmed)
- Streaming: `reasoning_text = "[\n"` (newline preserved)

**Impact**: Cosmetic. Leading/trailing whitespace in reasoning blocks differs between streaming and non-streaming API responses. Downstream consumers (tool call parser, API response formatter) are not affected since they don't compare the two paths.

**Fix direction**: Either remove `.trim()` from the one-shot path (preserving raw content) or add trimming to the streaming path's final output. Removing from one-shot is safer since it avoids changing streaming behavior.

## Run Statistics

| Harness | Duration | Executions | Coverage (edges) | Crashes |
|---------|----------|------------|------------------|---------|
| `fuzz_invariants` | 5 min | 317,428 | 7,730 | 0 |
| `fuzz_differential` | ~10s | ~90 | 6,629 | 1 (Mistral prefix bug) |
| `fuzz_redos` | 10s | 128,718 | 3,388 | 0 |
| `fuzz_with_tools` | 10s | 114,685 | 425 | 0 |

The differential fuzzer is extremely effective — it found a real bug almost immediately with trivially small inputs (1-2 bytes of payload). The invariant fuzzer confirmed that the tool call parsers are solid on their correctness properties over 300K+ executions.

## Recommended Next Steps

1. **Fix Bug 1** (prefix-matching content loss): Skip the prefix-buffering check when `force_reasoning=true` and `stripped_think_start` is false, or set `stripped_think_start = true` at construction time when `force_reasoning=true`.

2. **Fix Bug 2** (trim asymmetry): Remove `.trim()` from the one-shot path to match streaming behavior, or document the intentional difference.

3. **Extended differential runs**: Run `fuzz_differential` for 2+ hours with the dictionary to explore larger inputs and more parser types. The Kimi unicode tokens (`◁think▷`) and multi-block reasoning paths haven't been deeply explored yet.

4. **Extended ReDoS runs**: Run `fuzz_redos` for 1+ hour with `FUZZ_TIMEOUT_PER_INPUT=2 FUZZ_MAX_LEN=1024` to stress-test the pythonic regex, which has the most complex pattern with nested quantifiers.

5. **Coverage analysis**: Run `./fuzzing/coverage.sh` to identify remaining uncovered code paths and create targeted seeds.

## Code Audit Bugs (from deep audit, with regression tests)

Beyond the 2 fuzzer-found bugs above, a deep audit of unexplored parser code paths found 10 additional bugs. Regression tests were written for all of them (17 tests total, 12 confirm real bugs, 5 pass as guards).

### Bug 3: DSML parameter silently dropped with capitalized `string="True"` (Medium)
**File**: `lib/parsers/src/tool_calling/dsml/parser.rs`
The regex requires exactly `string="true"` or `string="false"` (lowercase). Capitalized `string="True"` causes the parameter to be silently dropped.

### Bug 4: DSML parameter silently dropped without `string` attribute (Medium)
**File**: `lib/parsers/src/tool_calling/dsml/parser.rs`
If the model omits the `string` attribute entirely, the parameter regex doesn't match and the parameter is silently dropped.

### Bug 5: DeepSeek V3 JSON normalization destroys newlines in string values (Medium)
**File**: `lib/parsers/src/tool_calling/json/deepseek_v3_parser.rs`, lines 115-119
When initial JSON parse fails, the fallback normalization joins lines with spaces (`.lines().map(|line| line.trim_start()).join(" ")`), corrupting string values containing intentional newlines.

### Bug 6: Pythonic parser drops text after tool call (Low)
**File**: `lib/parsers/src/tool_calling/pythonic/pythonic_parser.rs`
`split(&matches[0]).next()` only returns text BEFORE the first match. Text appearing after the tool call is silently dropped.

### Bug 7: `try_literal_eval` corrupts string values containing True/False/None (Medium)
**File**: `lib/parsers/src/tool_calling/xml/parser.rs`
Global `.replace("True", "true")` / `.replace("False", "false")` / `.replace("None", "null")` corrupts string values containing these as substrings (e.g., "TrueNorth" → "trueNorth", "Falsehood" → "falsehood", "NoneAvailable" → "nullAvailable").

### Bug 8: GLM-4.7 trim offset + UTF-8 boundary panic (High)
**File**: `lib/parsers/src/tool_calling/xml/glm47_parser.rs`, line 231
`&content[function_name.len()..]` uses the trimmed function name's byte length as an offset into the untrimmed content. With leading whitespace and multibyte UTF-8 function names, this panics with "byte index N is not a char boundary".

### Bug 9: Base JSON parser drops text after tool calls (Low)
**File**: `lib/parsers/src/tool_calling/json/base_json_parser.rs`, line 118
`try_parse_normal_text()` only extracts text BEFORE the start token (`input[..idx]`). Text appearing after the tool call end token is silently dropped.

### Bug 10: Kimi K2 OnceLock caches regex for first config only (Low)
**File**: `lib/parsers/src/tool_calling/xml/kimi_k2_parser.rs`, lines 26-35
`get_tool_call_regex()` takes config as a parameter but stores the result in a static `OnceLock`. Only the first call's config is used; subsequent calls with different token configs silently use the stale cached regex.

### Bug 11: `strip_quotes` panics on single-quote-char input (High)
**File**: `lib/parsers/src/tool_calling/xml/parser.rs`, line 23
`&trimmed[1..trimmed.len()-1]` panics when `trimmed` is a single quote character (`"\""`). Both `starts_with('"')` and `ends_with('"')` return true, but `[1..0]` has begin > end. Affects any XML tool call with a parameter value that is exactly `"` after trimming.

### Bug 12: `detect_tool_call_start_xml` panics on multibyte UTF-8 start tokens (High)
**File**: `lib/parsers/src/tool_calling/xml/parser.rs`, line 42
Iterates byte positions `1..start_token.len()` and slices `&start_token[..i]`. When `start_token` contains multibyte UTF-8 characters, this slices at non-char-boundaries causing a panic. Default tokens are ASCII so this is safe today, but the config field is `pub String`.

### Bug 13: `detect_tool_call_start_glm47` same UTF-8 byte-slicing panic (High)
**File**: `lib/parsers/src/tool_calling/xml/glm47_parser.rs`, line 30
Identical bug pattern to Bug 12. Byte-based iteration over `start_token` panics on multibyte UTF-8 characters.

### Bug 14: Harmony parser `content[0]` panic risk (Latent)
**File**: `lib/parsers/src/tool_calling/harmony/harmony_parser.rs`, line 123
Analysis channel uses `message.content[0]` (panics on empty) while the commentary branch correctly uses `.first()`. Not currently reachable via the harmony tokenizer, but latent risk.

## File Inventory

### New fuzz targets
- `lib/parsers/fuzz/fuzz_targets/fuzz_invariants.rs`
- `lib/parsers/fuzz/fuzz_targets/fuzz_differential.rs`
- `lib/parsers/fuzz/fuzz_targets/fuzz_redos.rs`
- `lib/parsers/fuzz/fuzz_targets/fuzz_with_tools.rs`

### New seeds
- `lib/parsers/fuzz/seeds/strip_quotes_edge.txt`
- `lib/parsers/fuzz/seeds/glm47_trim_mismatch.txt`
- `lib/parsers/fuzz/seeds/kimi_k2_empty_section.txt`
- `lib/parsers/fuzz/seeds/pythonic_redos.txt`

### New infrastructure
- `lib/parsers/fuzz/parser_tokens.dict`
- `fuzzing/coverage.sh`

### Modified files
- `lib/parsers/fuzz/Cargo.toml` (added `serde_json` dep, 4 new `[[bin]]` entries)
- `fuzzing/run_parser_fuzz.sh` (added `FUZZ_DICT` support, 4 new targets in `ALL_TARGETS`)
- `lib/parsers/src/reasoning/granite_parser.rs` (2 regression tests for streaming prefix bug)
- `lib/parsers/src/tool_calling/dsml/parser.rs` (2 regression tests for capitalized/missing string attr)
- `lib/parsers/src/tool_calling/json/deepseek_v3_parser.rs` (1 regression test for JSON normalization)
- `lib/parsers/src/tool_calling/pythonic/pythonic_parser.rs` (2 regression tests for text drop + zero-arg)
- `lib/parsers/src/tool_calling/xml/parser.rs` (3 regression tests for try_literal_eval corruption)
- `lib/parsers/src/tool_calling/xml/glm47_parser.rs` (2 regression tests for trim offset + UTF-8 panic)
- `lib/parsers/src/tool_calling/json/base_json_parser.rs` (2 regression tests for post-tool-call text drop)
- `lib/parsers/src/tool_calling/harmony/harmony_parser.rs` (2 regression tests for [0] panic risk)
- `lib/parsers/src/tool_calling/xml/kimi_k2_parser.rs` (1 regression test for OnceLock config caching)
- `lib/parsers/src/tool_calling/xml/parser.rs` (3 more: strip_quotes panic, XML Unicode token panic)
- `lib/parsers/src/tool_calling/xml/glm47_parser.rs` (1 more: GLM47 Unicode token panic)
