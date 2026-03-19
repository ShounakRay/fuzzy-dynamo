# fix: handle zero kv_block_size in compute_block_hash_for_seq

#### Overview:

[ref: TBD — file issue first]

Add an early return for `kv_block_size == 0` in `compute_block_hash_for_seq` to prevent a "chunk size must be non-zero" panic from `chunks_exact(0)`. A misconfigured worker or malformed ZMQ event with zero block size crashes the router on the first hash computation. Found via fuzzing with cargo-fuzz / libfuzzer.

#### Details:

Insert a guard at the top of `compute_block_hash_for_seq` that returns an empty `Vec` when `kv_block_size == 0`, before the call to `tokens.chunks_exact(kv_block_size as usize)`. All existing callers that pass a valid block size are unaffected.

#### Where should the reviewer start?

`lib/kv-router/src/protocols.rs`

#### Related Issues: (use one of the action keywords Closes / Fixes / Resolves / Relates to)

- Fixes GitHub issue: TBD
