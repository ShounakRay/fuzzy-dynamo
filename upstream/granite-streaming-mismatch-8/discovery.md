# Discovery: Granite Streaming Parser Silently Drops Short Inputs

## What's the bug?

When a large language model generates text, an inference server can deliver it in two ways: wait for the entire response and send it at once (one-shot), or send it piece by piece as it is generated (streaming). Users expect both modes to produce identical output. The Granite reasoning parser in Dynamo has a bug where the streaming mode silently drops short inputs.

The root cause is how the streaming parser handles ambiguity. Imagine you are a mail sorter and some packages need to go to a special "reasoning" bin if they are labeled with a tag that starts with "Here's my response:". A package arrives labeled just "H". You cannot tell yet whether this is the start of that special tag or just ordinary text, so you set it aside and wait for more. But if no more packages arrive, you have lost that "H" forever -- it sits in your holding area and is never delivered.

That is exactly what happens. The streaming parser checks whether incoming text could be the start of any special token (like `<|thinking|>` or the think-end marker). The single character "H" is a valid prefix of the Granite think-end token "Here's my response:", so the parser buffers it. When no more data arrives, the buffer is never flushed. The one-shot parser, which sees the complete input at once, has no such ambiguity and correctly returns "H" as normal text.

In production, this means short model outputs -- single characters, brief tokens at the start of generation, or outputs that happen to match the beginning of a reasoning marker -- can vanish during streaming inference. Users see incomplete or empty responses with no error.

## When does this happen in real life?

This bug affects streaming inference responses from Granite models:

- **Short model responses** — when a Granite model generates a very short response (1-2 characters) without any reasoning markup, the streaming parser swallows the text entirely. The client receives an empty response instead of the actual content
- **First tokens of generation** — streaming sends tokens as they're generated. The very first token might be a single character like "H" (the start of "Hello"). If this token is a prefix of a think-end marker, it's buffered and never emitted
- **Small chunk sizes** — when the inference engine sends output in small chunks (common in token-by-token streaming), each chunk is short enough to be a prefix of a think token, causing the streaming parser to buffer repeatedly

Users would see missing or delayed text at the beginning of streamed responses, or completely empty responses for very short answers. The one-shot (non-streaming) API would return the correct response, making this a hard-to-diagnose inconsistency.

## How we found it

### The fuzzing approach

The fuzz target `fuzz_differential` in `lib/parsers/fuzz` uses *differential fuzzing* between one-shot and streaming parsing modes. The fuzzer generates three things via the `Arbitrary` trait: a parser type selector (choosing among Granite, DeepSeek, and others), a chunk size (controlling how the input is split for streaming), and a text string. It feeds the same text to both the one-shot parser and the streaming parser (broken into chunks of the fuzzed size), then asserts that the concatenated streaming output matches the one-shot output for both `reasoning_text` and `normal_text`.

### What the fuzzer did

The fuzzer generated an input with the Granite parser type and the text `"H"` -- a single character. The one-shot parser returned `normal_text = "H"` as expected. The streaming parser received `"H"` as a single chunk and returned `normal_text = ""` -- an empty string. The assertion `oneshot_result.normal_text == stream_normal` failed, and the fuzzer saved the crashing input as `crash-b9ede57e0851fac41d4ccdf2a4f9db0ab301a461`.

The bug is specifically that the streaming parser checks prefixes of ALL tokens -- both start-of-reasoning and end-of-reasoning markers -- regardless of its current state. When the parser is not in reasoning mode, there is no reason to buffer text that might match an end-of-reasoning marker, because you cannot end something you never started.

### Why traditional testing missed this

The parser has extensive tests, but all use multi-word inputs like full sentences. A single character "H" is not something a human would think to test, yet it is the simplest input that triggers the prefix-matching ambiguity.

## The fix

The streaming parser needs a finalization mechanism: when the stream ends, any text remaining in the buffer that does not fully match a special token must be flushed as normal output. Alternatively, the parser should only check prefixes of tokens that are relevant to its current state (e.g., only check start-of-reasoning tokens when not in reasoning mode).

## Fuzzing technique

**Strategy:** Differential (one-shot vs streaming parsing)
**Target:** `fuzz_differential.rs`
**Crate:** `lib/parsers/fuzz`
**Run command:** `cd lib/parsers/fuzz && ~/.cargo/bin/cargo +nightly fuzz run fuzz_differential -- -max_total_time=60`
