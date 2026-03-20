# fix: prevent deadlock on duplicate block hashes in ConcurrentRadixTree

#### Overview:

[ref: TBD -- file issue first]

Add an `Arc::ptr_eq` guard in `apply_stored` before acquiring a read lock on an existing child node. When a store event contains duplicate `ExternalSequenceBlockHash` values, the worker lookup table can create a self-referential node; the code then holds a write lock on the node and attempts to read-lock the same node, causing an irrecoverable deadlock at 0% CPU. Found via fuzzing with cargo-fuzz / libfuzzer.

#### Details:

Before calling `existing.read()` to verify block hash consistency, check whether `existing` and `current` point to the same `Arc`. If they do, skip the read-lock verification and log a warning about the self-referential block. This preserves the existing verification logic for the non-degenerate case while preventing the deadlock on duplicate hashes.

#### Where should the reviewer start?

`lib/kv-router/src/indexer/concurrent_radix_tree.rs`

#### Related Issues: (use one of the action keywords Closes / Fixes / Resolves / Relates to)

- Fixes GitHub issue: TBD
