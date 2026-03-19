# fix: remove OnceLock regex caching in KimiK2 parser to respect per-call config

#### Overview:

[ref: TBD -- file issue first]

Remove the `OnceLock`-based regex cache in `get_tool_call_regex` so the regex is rebuilt from the caller's `KimiK2ParserConfig` on every invocation. The current code bakes the first config's tokens into a process-wide static regex; all subsequent calls with different configs silently use the stale pattern and return zero tool calls. Found via fuzzing with cargo-fuzz / libfuzzer.

#### Details:

Replace `static TOOL_CALL_REGEX: OnceLock<Regex>` and the `get_or_init` wrapper with a plain function that compiles the regex fresh from the provided config. The return type changes from `&'static Regex` to an owned `Regex`. Regex compilation for these short patterns is ~1-2 us, negligible compared to network I/O. Callers need no further changes since the regex is consumed immediately in `captures_iter`.

#### Where should the reviewer start?

`lib/parsers/src/tool_calling/xml/kimi_k2_parser.rs`

#### Related Issues: (use one of the action keywords Closes / Fixes / Resolves / Relates to)

- Fixes GitHub issue: TBD
