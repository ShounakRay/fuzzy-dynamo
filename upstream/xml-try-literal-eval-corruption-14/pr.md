# fix: use word-boundary regex in try_literal_eval to prevent keyword substring corruption

#### Overview:

[ref: TBD — file issue first]

Replace global `.replace("True", "true")` / `.replace("False", "false")` / `.replace("None", "null")` calls with word-boundary-aware regex replacements (`\bTrue\b`, `\bFalse\b`, `\bNone\b`). The global replacements corrupt argument values containing these substrings — e.g., `"TrueNorth"` becomes `"trueNorth"` and `"NoneAvailable"` becomes `"nullAvailable"`. Found via fuzzing with cargo-fuzz / libfuzzer.

#### Details:

Introduce three `OnceLock<Regex>` statics for `\bTrue\b`, `\bFalse\b`, and `\bNone\b`. In `try_literal_eval`, after replacing single quotes with double quotes, apply these regexes via `replace_all` instead of `str::replace`. This ensures only standalone Python keyword literals are converted to their JSON equivalents (`true`, `false`, `null`), leaving substrings within larger words (e.g., `"TrueNorth"`, `"Falsehood"`, `"NoneAvailable"`) intact. The fast path for already-valid JSON is unchanged.

#### Where should the reviewer start?

`lib/parsers/src/tool_calling/xml/parser.rs` — the `try_literal_eval` function

#### Related Issues: (use one of the action keywords Closes / Fixes / Resolves / Relates to)

- Fixes GitHub issue: TBD
