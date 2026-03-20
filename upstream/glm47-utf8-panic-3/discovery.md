# Discovery: GLM-4.7 Parser Panics on Multibyte UTF-8 Function Names

## What's the bug?

Text on a computer is stored as a sequence of numbers (bytes). In the early days, each character was one byte (the ASCII standard), which works fine for English letters, digits, and punctuation. But the world has many more characters than 128 -- Cyrillic, Chinese, Arabic, emoji, and thousands more. UTF-8 is the encoding standard that solves this: simple ASCII characters still take 1 byte, but other characters take 2, 3, or even 4 bytes. The Cyrillic letter "sh" (written as "sh" in Russian) takes 2 bytes. A Chinese character like "huo" takes 3. An emoji takes 4.

This matters because you cannot just treat byte positions and character positions as the same thing. If a string contains the ASCII space character (1 byte) followed by the Cyrillic character (2 bytes: 0xD1 0x88), byte position 2 lands *in the middle* of that Cyrillic character -- it is not a valid place to split the string. Rust, unlike C, actively checks for this and crashes the program (called a "panic") rather than producing garbage data. This is safer than silently corrupting text, but a panic in a production server still means downtime.

The GLM-4.7 tool call parser has exactly this problem. It parses XML-formatted tool calls from the language model's output, extracting function names and arguments. The code trims whitespace from the function name, then uses the *trimmed* name's byte length to index back into the *original, untrimmed* string. When there is leading whitespace and the function name contains multibyte characters, the trimmed length is shorter than the original byte span, so the index lands in the middle of a multibyte character. Rust panics.

Because the input comes from model output (which can contain any Unicode text), this is a denial-of-service vector on any inference server using the GLM-4.7 parser. A single response containing a multibyte character near a function name with some whitespace will crash the server.

## When does this happen in real life?

This bug triggers whenever a GLM-4.7 model generates a tool call where the function name contains non-ASCII characters and there's any whitespace around it. In practice, this happens when:

- **Multilingual users** ask the model to call functions with Chinese, Japanese, Korean, or Cyrillic names — common in non-English deployments where tool names are localized (e.g., `获取天气(城市="北京")`)
- **The model hallucinates** a function name that includes accented characters, emoji, or other Unicode — LLMs regularly produce unexpected Unicode in tool call output
- **Whitespace in model output** — models frequently emit leading/trailing spaces around function names, especially after newlines in the generation

Any of these cases crash the inference server process immediately. Every request being served by that process is lost. The server must be restarted, and the crash will repeat if the same prompt is retried.

## How we found it

### The fuzzing approach

We wrote a crash oracle fuzzer: the fuzz target (`fuzz_glm47_utf8.rs`) accepts raw bytes from the fuzzer, validates them as UTF-8, and passes them to the `try_tool_call_parse_glm47()` function. Any panic counts as a bug. For short inputs (under 50 bytes), the target also wraps the fuzzed text as a function name inside a valid XML tool call structure with leading spaces -- this is the pattern most likely to trigger the byte-offset bug, since it guarantees the whitespace-plus-multibyte combination that the parser mishandles.

### What the fuzzer did

The fuzzer generated the byte sequence `[46, 209, 136, 24, 10]`. In UTF-8, bytes 209 and 136 together encode the Cyrillic character U+0448 ("sh"). The fuzzer wrapped this as the function name inside `<tool_call>  .sh\x18\n<arg_key>location</arg_key><arg_value>NYC</arg_value></tool_call>`. The parser found the function name region (which includes leading spaces), trimmed the spaces to get `.sh\x18\n`, computed its byte length as 5, then used byte offset 5 in the original untrimmed content -- which lands between the two bytes of "sh". Rust panicked with: "byte index 4 is not a char boundary; it is inside 'sh' (bytes 3..5)".

The crash artifact is `crash-ed5713d8cb0206d339613ca7de5428b9856ad393`.

### Why traditional testing missed this

Unit tests for parsers typically use ASCII function names like "get_weather" or "search". Nobody thinks to test with Cyrillic function names that have leading whitespace -- it is an edge case that only matters when byte-length and character-length diverge, which ASCII never exercises.

## The fix

Use the original byte position of the `<arg_key>` tag (already computed as `pos`) to slice into the content string, instead of using the trimmed function name's byte length. This avoids the mismatch between trimmed and untrimmed byte offsets entirely.

## Fuzzing technique

**Strategy:** Crash oracle (catch_unwind for panics)
**Target:** `fuzz_glm47_utf8.rs`
**Crate:** `lib/parsers/fuzz`
**Run command:** `cd lib/parsers/fuzz && ~/.cargo/bin/cargo +nightly fuzz run fuzz_glm47_utf8 -- -max_total_time=60`
