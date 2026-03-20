# fix: guard strip_quotes against single-character quote input

#### Overview:

[ref: TBD — file issue first]

Add a `trimmed.len() >= 2` guard in `strip_quotes` before attempting to slice off matching quote characters. A single-character input like `"` matches both `starts_with('"')` and `ends_with('"')`, then `&trimmed[1..0]` panics because begin > end. This is reachable via model output containing `<function=">`. Found via fuzzing with cargo-fuzz / libfuzzer.

#### Details:

Move the existing `starts_with`/`ends_with` check inside a `trimmed.len() >= 2` condition so that single-character strings (and empty strings after trimming) are returned as-is without attempting the interior slice. Normal quoted strings of length 2 or more are unaffected.

#### Where should the reviewer start?

`lib/parsers/src/tool_calling/xml/parser.rs`

#### Related Issues: (use one of the action keywords Closes / Fixes / Resolves / Relates to)

- Fixes GitHub issue: TBD
