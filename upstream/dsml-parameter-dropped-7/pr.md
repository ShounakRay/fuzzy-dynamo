# fix: make DSML parser string attribute optional and case-insensitive

#### Overview:

[ref: TBD -- file issue first]

Update the parameter regex in `parse_parameters` to accept the `string` attribute in any case (e.g., `"True"`, `"FALSE"`) and to treat it as optional. Previously, parameters with a capitalized or missing `string` attribute were silently dropped from the parsed tool call arguments. Found via fuzzing with cargo-fuzz / libfuzzer.

#### Details:

Change the parameter regex from `string=\"(true|false)\"` (mandatory, lowercase-only) to `(?:\s+string=\"(true|false)\")?\s*>` with the `(?si)` flag for case-insensitive matching. When the attribute is omitted, default to non-string behavior (attempt JSON parse, fall back to string). The capture group index for the parameter value shifts from group 2 to group 3.

#### Where should the reviewer start?

`lib/parsers/src/tool_calling/dsml/parser.rs`

#### Related Issues: (use one of the action keywords Closes / Fixes / Resolves / Relates to)

- Fixes GitHub issue: TBD
