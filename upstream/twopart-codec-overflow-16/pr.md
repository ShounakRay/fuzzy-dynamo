# fix: use checked arithmetic in TwoPartCodec to prevent integer overflow

#### Overview:

[ref: https://github.com/ai-dynamo/dynamo/issues/6955]

Replace unchecked `24 + header_len + body_len` with `checked_add()` in both the decode and encode paths. The unchecked addition overflows on crafted input: in debug builds this panics, and in release builds the wrapped value silently bypasses the `max_message_size` guard. Found via fuzzing with cargo-fuzz / libfuzzer.

#### Details:

Use `24usize.checked_add(header_len).and_then(|v| v.checked_add(body_len))` to detect overflow and return an error instead of panicking or wrapping. This was filed as #6955 and fixed in PR #6959.

#### Where should the reviewer start?

`lib/runtime/src/pipeline/network/codec/two_part.rs`

#### Related Issues: (use one of the action keywords Closes / Fixes / Resolves / Relates to)

- Fixes GitHub issue: #6955
- Fixes GitHub PR: #6959
