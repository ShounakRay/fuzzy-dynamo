# Discovery: Pythonic Parser Absorbs Prefix Characters into Function Name

## What's the bug?

When a large language model decides to call a tool (like a weather API or a calculator), it outputs the call in a specific text format. The Pythonic parser extracts function calls that look like Python code -- for example, `get_weather(location="NYC")`. It uses a regular expression (regex) to find these patterns in the model's output.

A regular expression is a pattern-matching language used to search text. The parser's regex is `[a-zA-Z]+\w*\(`, which means "one or more letters, followed by any number of word characters (letters, digits, underscores), followed by an opening parenthesis." The problem is that this pattern does not specify where the function name must *start*. It is like telling someone "find a word ending with an opening parenthesis" without saying "the word must begin after a space or punctuation." If there are letters immediately before the real function name, the regex greedily absorbs them.

Consider the model output `[vvvvvvvvv[v[vv]get_weather(location="NYC")`. A human can see that `get_weather` is the function name and the `vv]` before it is just noise. But the regex sees `vvget_weather(` as one continuous match -- the `vv` right before `get_weather` are letters, so they satisfy the `[a-zA-Z]+` part of the pattern. The parser extracts the function name as `vvget_weather` instead of `get_weather`.

In a production inference server, this means tool calls get dispatched to the wrong function. A call meant for `get_weather` goes to `vvget_weather`, which does not exist, causing a silent failure. Or worse, if the prefix happens to form a valid but different function name, the wrong tool gets called with the right arguments.

## When does this happen in real life?

This bug causes tool calls to be dispatched to the wrong function. In production:

- **Model output with surrounding text** — LLMs often generate tool calls embedded in natural language, like "Let me check that for you: get_weather(location='NYC')". If the text immediately before the function name ends with a letter (e.g., "you:get_weather" without a space), the parser absorbs that letter into the function name
- **Multi-tool responses** — when a model generates multiple tool calls in one response, the closing bracket of one call might run into the start of the next, causing prefix leakage
- **The wrong tool gets called** — the extracted function name `"youget_weather"` or `"vvget_weather"` doesn't match any registered tool. The tool call fails, or worse, if a tool with a similar prefix exists, the wrong tool is invoked with the right arguments

This is a silent correctness bug — no crash, no error. The tool call either fails with a "function not found" error (confusing to debug) or succeeds with the wrong function (dangerous).

## How we found it

### The fuzzing approach

The fuzz target `fuzz_parser_semantic` in `lib/parsers/fuzz` uses a *semantic round-trip oracle*. Instead of comparing two implementations against each other, it embeds a known-valid tool call into fuzz-controlled surrounding text and verifies that the parser extracts the correct function name and arguments. The fuzzer generates a random string and a split position. The string is split into a prefix and suffix, and a hardcoded valid tool call (like `get_weather(location="NYC")`) is sandwiched between them. This tests whether the parser can correctly isolate the tool call from surrounding noise. Five different parser formats (XML, Pythonic, DSML, Basic JSON, DeepSeek V3) are tested this way.

### What the fuzzer did

The fuzzer generated the text `[vvvvvvvvv[v[vv]` as surrounding noise, with the Pythonic test case selected (case 1 in the match). The split position placed `[vvvvvvvvv[v[vv]` as the prefix and an empty string as the suffix, producing the combined input `[vvvvvvvvv[v[vv]get_weather(location="NYC")`. The Pythonic parser successfully found a tool call but extracted the function name as `vvget_weather` -- the two `v` characters immediately before `get_weather` leaked into the name because they are valid `[a-zA-Z]` characters with no word boundary to stop the regex. The assertion `calls[0].function.name == "get_weather"` failed, and the fuzzer saved the crashing input as `crash-3a33dc87a512da1a1877cdd0de29326ff237cd76`.

### Why traditional testing missed this

Unit tests only tested clean inputs like `get_weather(location="NYC")` or inputs with non-alphabetic characters before the function name (brackets, spaces). Nobody tested the case where alphabetic garbage text runs directly into the function name with no separator.

## The fix

Add a word boundary assertion `\b` to the regex so the function name must start at a word boundary: `\b[a-zA-Z]+\w*\(`. Alternatively, use a negative lookbehind `(?<![a-zA-Z0-9_])` to ensure the character before the function name is not an identifier character.

## Fuzzing technique

**Strategy:** Semantic round-trip oracle
**Target:** `fuzz_parser_semantic.rs`
**Crate:** `lib/parsers/fuzz`
**Run command:** `cd lib/parsers/fuzz && ~/.cargo/bin/cargo +nightly fuzz run fuzz_parser_semantic -- -max_total_time=60`
