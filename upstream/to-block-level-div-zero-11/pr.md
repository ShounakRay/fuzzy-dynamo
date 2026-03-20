# fix: handle zero block_size in RequestExtraInfo::to_block_level

#### Overview:

[ref: TBD — file issue first]

Add an early return for `block_size == 0` (and `total_tokens == 0`) in `to_block_level` to prevent three separate division-by-zero panics: `total_tokens.div_ceil(block_size)`, `req_start / block_size`, and `(req_end.saturating_sub(1)) / block_size`. A misconfigured zero block size crashes the router on any multimodal request. Found via fuzzing with cargo-fuzz / libfuzzer.

#### Details:

Insert a guard at the top of `to_block_level` that returns an empty `Vec` when `block_size == 0` or `total_tokens == 0`. This covers all three division paths without changing any downstream logic. The `total_tokens == 0` case is also guarded since it would produce an empty block array anyway.

#### Where should the reviewer start?

`lib/kv-router/src/protocols.rs`

#### Related Issues: (use one of the action keywords Closes / Fixes / Resolves / Relates to)

- Fixes GitHub issue: TBD
