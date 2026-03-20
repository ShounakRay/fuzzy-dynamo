# Discovery: DSML Parser Silently Drops Parameters

## What's the bug?

The DeepSeek V3.2 DSML (DeepSeek Markup Language) tool call parser uses a regular expression to extract function parameters from structured markup. Each parameter tag looks something like `<parameter name="query" string="true">hello</parameter>`. The regex that matches these tags requires a `string` attribute with a lowercase value of exactly `"true"` or `"false"`.

The problem surfaces in two ways. First, if the `string` attribute is capitalized -- `string="True"` instead of `string="true"` -- the regex does not match. Second, if the `string` attribute is omitted entirely, the regex also does not match. In both cases, the parameter is silently skipped during extraction. No error is raised, no warning is logged. The downstream tool simply receives incomplete arguments.

Silent data loss is arguably worse than a crash. When a server crashes, someone notices and investigates. When a parameter is quietly dropped, the tool receives a partial request and may produce a subtly wrong result -- or fail in a way that looks like a model quality issue rather than a parsing bug. Imagine a search tool that receives a function call but the `query` parameter was dropped: it would either error with a missing-argument message that gets blamed on the model, or default to an empty search that returns irrelevant results.

This bug is especially likely in practice because Python-influenced language models commonly emit `True` and `False` with capital letters (matching Python's boolean syntax), and some models omit optional-looking attributes entirely when they seem redundant.

## When does this happen in real life?

This bug causes tool call parameters to silently disappear:

- **Python-influenced LLM output** — many LLMs are trained predominantly on Python code, so they naturally produce `string="True"` (capital T) instead of `string="true"`. The DeepSeek V3.2 DSML format includes a `string` attribute on parameters, and Python-trained models frequently capitalize it
- **Missing string attribute** — some model outputs omit the `string` attribute entirely (e.g., `<parameter name="count">42</parameter>` without any `string="..."` attribute). The regex requires this attribute, so the parameter vanishes
- **Silent data loss** — the tool receives a partial set of arguments. A search function might get called without a query, a booking function without a date. The tool either fails with a confusing "missing required parameter" error or, worse, executes with default values the user didn't intend

This is particularly insidious because the parser returns success — it found the tool call and the function name. Only the arguments are silently incomplete. Debugging requires comparing raw model output against parsed tool calls, which most monitoring doesn't capture.

## How we found it

### The fuzzing approach

The fuzz target `fuzz_parser_semantic.rs` in `lib/parsers/fuzz` uses a **semantic round-trip oracle**. It embeds a known-valid tool call inside fuzz-controlled surrounding text, then verifies that the parser extracts the correct function name and argument values. For the DSML case (case 2 in the target), it wraps a `search(query=hello)` call in DSML markup with fuzz-generated prefix and suffix text.

The key insight of the semantic oracle is that we know exactly what the output should be -- if you embed `search` with `query="hello"`, the parser must return those values regardless of what garbage text surrounds the tool call.

### What the fuzzer did

The fuzzer generated inputs where the DSML tool call was correctly structured but the `query` parameter used the format expected by the test. The semantic oracle verified that the extracted argument for `"query"` should equal `"hello"`. When the parser returned either no arguments or a null value for the key, the assertion fired. Further manual investigation revealed the root cause: the regex requires an exact lowercase match on the `string` attribute and treats the attribute as mandatory.

### Why traditional testing missed this

All existing unit tests used the exact lowercase format `string="true"` that the regex expects. Nobody wrote a test with the capitalized Python-style `string="True"` or with the attribute omitted.

## The fix

Make the `string` attribute case-insensitive (e.g., `(?i)(true|false)`) and optional with a sensible default (e.g., treat missing attribute as "try JSON parse, fall back to string").

## Fuzzing technique

**Strategy:** Semantic round-trip oracle
**Target:** `fuzz_parser_semantic.rs`
**Crate:** `lib/parsers/fuzz`
**Run command:** `cd lib/parsers/fuzz && ~/.cargo/bin/cargo +nightly fuzz run fuzz_parser_semantic -- -max_total_time=60`
