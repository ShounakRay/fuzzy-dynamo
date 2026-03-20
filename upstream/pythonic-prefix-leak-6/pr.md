# fix: add word boundary to Pythonic parser regex to prevent prefix absorption

#### Overview:

[ref: TBD — file issue first]

Add a `(?<!\w)` negative lookbehind before each function-name pattern in the Pythonic tool call parser regex. Without this, characters from surrounding text (e.g., `"vv"` before `"get_weather"`) are greedily absorbed by `[a-zA-Z]+\w*`, producing incorrect function names like `"vvget_weather"`. Found via fuzzing with cargo-fuzz / libfuzzer.

#### Details:

In `get_pythonic_regex`, insert `(?<!\w)` before each `[a-zA-Z]+\w*\(` group in the regex pattern. A bare `\b` is insufficient here because the bracket character `]` already constitutes a word boundary — the real problem is that the regex engine starts matching at the first alphabetic character it finds before the function name. The negative lookbehind ensures no word character (`[a-zA-Z0-9_]`) immediately precedes the function name match, preventing prefix absorption.

#### Where should the reviewer start?

`lib/parsers/src/tool_calling/pythonic/pythonic_parser.rs` — the `get_pythonic_regex` function

#### Related Issues: (use one of the action keywords Closes / Fixes / Resolves / Relates to)

- Fixes GitHub issue: TBD
