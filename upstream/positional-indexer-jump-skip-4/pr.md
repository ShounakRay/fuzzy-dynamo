# fix: cascade-remove blocks after removal in PositionalIndexer to preserve jump invariant

#### Overview:

[ref: TBD — file issue first]

After removing a block at position P for a worker, cascade-remove all blocks at positions > P for that worker. Without this, the jump optimization in `jump_search_matches` skips over removed positions and reports inflated match scores because orphan blocks at later positions survive parent removal. Found via fuzzing with cargo-fuzz / libfuzzer.

#### Details:

In `remove_blocks_impl`, after removing a block at position P, collect all entries in the worker's `LevelIndex` with position > P and remove them from both the worker map and the shared `(position, local_hash)` index. The removed orphan count is added to `num_removed_blocks` so the `tree_sizes` counter stays accurate. This preserves the monotonic-coverage invariant that `jump_search_matches` relies on: if a worker matches at positions A and B, it matches at every position in [A, B].

#### Where should the reviewer start?

`lib/kv-router/src/indexer/positional.rs` — the `remove_blocks_impl` method

#### Related Issues: (use one of the action keywords Closes / Fixes / Resolves / Relates to)

- Fixes GitHub issue: TBD
