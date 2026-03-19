# fix: add bounds validation in create_stored_blocks to prevent OOB panics

#### Overview:

[ref: TBD — file issue first]

Add upfront length validation in `create_stored_blocks` and `create_stored_block_from_parts` to prevent out-of-bounds panics when `token_ids` is shorter than the block count implies. A malformed ZMQ event from a worker with mismatched `block_hashes` and `token_ids` lengths crashes the router. Found via fuzzing with cargo-fuzz / libfuzzer.

#### Details:

In `create_stored_blocks`, sum `num_block_tokens` and compare against `token_ids.len()` before the loop; return an empty `Vec` with a warning if the tokens are insufficient. In `create_stored_block_from_parts`, add an assertion that `token_ids.len() >= kv_block_size` before calling `compute_block_hash_for_seq`, which otherwise returns an empty `Vec` whose `[0]` index panics.

#### Where should the reviewer start?

`lib/kv-router/src/zmq_wire.rs`

#### Related Issues: (use one of the action keywords Closes / Fixes / Resolves / Relates to)

- Fixes GitHub issue: TBD
