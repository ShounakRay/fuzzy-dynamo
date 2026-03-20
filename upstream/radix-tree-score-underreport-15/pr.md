# fix: correct RadixTree find_matches scoring to track matched_depth

#### Overview:

[ref: https://github.com/ai-dynamo/dynamo/issues/5973]

Rewrite the `find_matches` traversal in `RadixTree` to accumulate the number of matching blocks along each tree path rather than counting matching tree nodes. The previous logic reported a score of 1 for a fully cached 3-block query, causing the KV-cache-aware router to miss reuse opportunities and recompute KV cache unnecessarily. Found via fuzzing with cargo-fuzz / libfuzzer.

#### Details:

The tree traversal was counting the number of matching tree nodes rather than the total matched depth (number of blocks) along the path from root to leaf. This was fixed in upstream PRs #5973 and #6122.

#### Where should the reviewer start?

`lib/kv-router/src/indexer/radix_tree.rs`

#### Related Issues: (use one of the action keywords Closes / Fixes / Resolves / Relates to)

- Fixes GitHub issue: #5973
- Fixes GitHub PR: #6122
