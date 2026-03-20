# fix: handle zero block_size in DefaultWorkerSelector::select_worker

#### Overview:

[ref: TBD — file issue first]

Add an early `block_size == 0` check in `select_worker` that returns `Err(KvSchedulerError::NoEndpoints)` to prevent a division-by-zero panic at `isl.div_ceil(block_size as usize)`. A secondary issue is that `(prefill_token as f64) / (block_size as f64)` produces `Inf`, causing NaN propagation in the softmax scoring. Found via fuzzing with cargo-fuzz / libfuzzer.

#### Details:

Insert a guard after the `isl_tokens > 0` assertion that checks `block_size == 0` and returns an error. This prevents both the integer division-by-zero panic at line 113 and the floating-point Inf/NaN issue at line 141. The error variant `NoEndpoints` is reused since a zero block size means no worker can serve the request.

#### Where should the reviewer start?

`lib/kv-router/src/scheduling/selector.rs`

#### Related Issues: (use one of the action keywords Closes / Fixes / Resolves / Relates to)

- Fixes GitHub issue: TBD
