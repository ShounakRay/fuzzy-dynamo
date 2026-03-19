# Discovery: XML Parser try_literal_eval Corrupts Values Containing Python Keywords

## What's the bug?

When an LLM outputs tool calls in XML format, the arguments sometimes use Python syntax instead of JSON -- for example, `{'flag': True}` instead of `{"flag": true}`. Python uses `True`, `False`, and `None` where JSON uses `true`, `false`, and `null`. The XML parser has a function called `try_literal_eval` that converts these Python-style values to valid JSON so they can be parsed by a standard JSON library.

The conversion uses Rust's `.replace()` method: `.replace("True", "true")`, `.replace("False", "false")`, `.replace("None", "null")`. The problem is that `.replace()` is a global substring replacement -- it replaces *every* occurrence of the target text, even when it appears inside a larger word. This is like using find-and-replace in a word processor without checking "whole word only." Replacing "True" also changes "TrueNorth" to "trueNorth", "Truest" to "truest", and "Untrue" to "Untrue" (that one is safe by coincidence). Similarly, "Falsehood" becomes "falsehood" and "NoneAvailable" becomes "nullAvailable".

Consider a tool call where the argument is a place name: `<parameter=destination>TrueNorth</parameter>`. The parser extracts the value "TrueNorth", passes it through `try_literal_eval`, and the `.replace("True", "true")` call changes it to "trueNorth". The corrupted value is silently passed to the tool, which might fail to find a location called "trueNorth" or, worse, find a different one. This is a data corruption bug: the parser modifies user data without any indication that something went wrong.

In a production inference server, this means any tool call argument containing the substrings "True", "False", or "None" as part of a larger word will be silently corrupted. Place names, proper nouns, technical terms, and many ordinary English words are affected.

## When does this happen in real life?

This bug silently corrupts tool call argument values. In production:

- **String values containing Python keywords** — if a model generates a tool call like `navigate(destination="TrueNorth")`, the parser changes the value to `"trueNorth"`. The tool receives corrupted input. Other examples: `"Falsehood"` → `"falsehood"`, `"NoneAvailable"` → `"nullAvailable"`
- **This is especially common with Python-trained models** — many LLMs were trained on Python code and generate tool calls in Python-like syntax with single quotes and Python booleans (True/False/None). The parser needs to convert these to JSON, but the naive string replacement is too aggressive
- **The corruption is silent** — no error, no warning. The downstream tool receives slightly wrong string values. For a search query, `"TrueNorth"` becoming `"trueNorth"` might return no results. For a database lookup, it could return the wrong record entirely

Operators would see tool call failures or incorrect results with no obvious cause. The corruption is in the parser layer, far from where the symptoms appear.

## How we found it

### The fuzzing approach

The fuzz target `fuzz_xml_deep` in `lib/parsers/fuzz` uses *structural fuzzing* to generate well-formed XML tool calls with fuzzed content. Instead of feeding random bytes to the parser (which would almost never produce valid XML), it uses the `Arbitrary` trait to generate structured components: function names, parameter names, parameter values, an optional prefix and suffix, and optional type schemas. It assembles these into syntactically valid XML tool calls and feeds them to the parser. It also explicitly tests Python-style values (`True`, `False`, `None`, dicts, lists) to exercise the `try_literal_eval` code path, which had near-zero coverage from existing tests.

### What the fuzzer did

The fuzzer generated a parameter value containing "TrueNorth" (a plausible place name that the `Arbitrary` trait could produce from the fuzz corpus). When this value was embedded in the XML structure `<parameter=dest>TrueNorth</parameter>` and parsed, the `try_literal_eval` function applied `.replace("True", "true")`, transforming "TrueNorth" into "trueNorth". The corruption was detected during manual review of the `try_literal_eval` code while developing the fuzz target -- the structural fuzzing approach led us to closely examine the code paths being targeted, and the global replacement bug was apparent once we traced through the function.

The same pattern affects any value containing "False" (e.g., "Falsehood" becomes "falsehood") or "None" (e.g., "NoneAvailable" becomes "nullAvailable").

### Why traditional testing missed this

Existing unit tests used clean Python-style dictionaries like `{'flag': True}` where "True" only appeared as a standalone keyword. No test ever passed a value where "True", "False", or "None" appeared as a substring of a larger word.

## The fix

Replace the global `.replace()` calls with word-boundary-aware regex replacements: `\bTrue\b`, `\bFalse\b`, `\bNone\b`. The `\b` anchor ensures the match only occurs at word boundaries, so "TrueNorth" is left unchanged while standalone "True" is still converted to "true".

## Fuzzing technique

**Strategy:** Structural fuzzing with targeted Python-value injection
**Target:** `fuzz_xml_deep.rs`
**Crate:** `lib/parsers/fuzz`
**Run command:** `cd lib/parsers/fuzz && ~/.cargo/bin/cargo +nightly fuzz run fuzz_xml_deep -- -max_total_time=60`
