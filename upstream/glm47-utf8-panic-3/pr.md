# fix: use original byte position in GLM-4.7 parser to prevent UTF-8 boundary panic

#### Overview:

[ref: TBD — file issue first]

Save the byte position returned by `content.find(arg_key_start)` and reuse it when slicing `args_section`, instead of using `function_name.len()` which reflects the trimmed string and lands inside multibyte UTF-8 characters. The mismatch between trimmed length and untrimmed byte offset causes a "byte index is not a char boundary" panic on any input with leading whitespace and multibyte characters (CJK, Cyrillic, emoji). Found via fuzzing with cargo-fuzz / libfuzzer.

#### Details:

Store the result of `content.find(arg_key_start)` in a variable `arg_key_pos` before the `if let`. Use `arg_key_pos` again when computing `args_section`: instead of `&content[function_name.len()..]`, use `&content[pos..]` when `arg_key_pos` is `Some(pos)`, or `""` otherwise. This ensures the byte offset always refers to a valid position in the original `content` string regardless of whitespace trimming.

#### Where should the reviewer start?

`lib/parsers/src/tool_calling/xml/glm47_parser.rs`

#### Related Issues: (use one of the action keywords Closes / Fixes / Resolves / Relates to)

- Fixes GitHub issue: TBD
