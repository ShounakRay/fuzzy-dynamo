# Discovery: XML Parser strip_quotes Panics on Single Quote Character

## What's the bug?

Many parsers need to "strip quotes" from strings -- if a value is wrapped in matching quotation marks like `"hello"`, you want to extract just `hello`. The standard approach is: check if the first and last characters are both quotes, then return everything between them. In code, that looks like `string[1..length-1]`, skipping the first character and the last character.

But what happens when the string is exactly one character long, and that character is a quote? The check "starts with a quote AND ends with a quote" passes -- the single `"` character is both the start and end. Then the code tries to compute `string[1..0]` -- a slice where the start position (1) is after the end position (0). This is mathematically impossible (you cannot have a range from 1 to 0), and Rust panics with "begin <= end (1 <= 0)".

This is a classic off-by-one error. The developer assumed that if a string starts and ends with a quote, it must be at least 2 characters long (one quote on each side). That assumption is wrong for a single-character string.

In the XML tool call parser, `strip_quotes` is called on function names and parameter names extracted by regex from model output. A model response like `<function=">` triggers the crash. Since model output is not fully controllable, any model that occasionally produces malformed tool calls with a bare quote character will crash the inference server.

## When does this happen in real life?

This bug triggers when a model generates a tool call where the function name or parameter name is exactly a single quote character (`"` or `'`). While unlikely in normal operation, this happens when:

- **The model hallucinates** malformed tool calls — LLMs sometimes produce garbled XML, especially at the start of generation or when the prompt is adversarial
- **Adversarial prompts** deliberately try to get the model to emit unusual characters in function name positions, which could be used as a denial-of-service attack against the inference server
- **Truncated generation** — if the model's output is cut off mid-token (e.g., due to max_tokens limit), the partial output might leave a lone quote character where a function name was expected

The server crashes on the specific request that triggered the malformed output. Other concurrent requests on the same process are also affected.

## How we found it

### The fuzzing approach

We wrote a structural fuzzer (`fuzz_xml_deep.rs`) that uses Rust's `Arbitrary` trait to generate structured inputs: random function names, parameter names and values, prefix/suffix text, and type hints for schema-aware parsing. The fuzzer wraps these into syntactically valid XML tool call structures and passes them to `try_tool_call_parse_xml`. This ensures the fuzzer reaches deep parsing code (like `strip_quotes`, `convert_param_value`, and `try_literal_eval`) that would never be reached by random byte strings.

### What the fuzzer did

The fuzzer generated a `FuzzInput` where the `func_name` field was the single character `"`. The target constructed the XML string `<tool_call><function="><parameter=param>...</parameter></function></tool_call>` and called the parser. The parser's regex captured `"` as the function name, passed it to `strip_quotes`, which saw that it starts and ends with a double quote, and attempted to return `&trimmed[1..0]`. Rust panicked.

This bug was also independently noticed during code review while developing the fuzz target -- the `is_safe_name` filter in the fuzz target was added specifically to work around this known panic so the fuzzer could continue exploring other code paths.

### Why traditional testing missed this

The existing test suite actually *documents* this bug: there is a `#[should_panic]` test called `test_strip_quotes_panics_on_single_quote_char` that confirms the panic exists. But a `should_panic` test does not fix the bug -- it just acknowledges it. No normal test uses a single quote character as a function name because it is not a realistic function name. But model output is not always realistic.

## The fix

Add a length check before the quote-stripping logic: `if trimmed.len() >= 2` before testing starts_with/ends_with. A single-character string cannot have meaningful content between its quotes, so it should be returned as-is or as an empty string.

## Fuzzing technique

**Strategy:** Structural fuzzing (generates valid XML tool call structures with fuzzed content)
**Target:** `fuzz_xml_deep.rs`
**Crate:** `lib/parsers/fuzz`
**Run command:** `cd lib/parsers/fuzz && ~/.cargo/bin/cargo +nightly fuzz run fuzz_xml_deep -- -max_total_time=60`
