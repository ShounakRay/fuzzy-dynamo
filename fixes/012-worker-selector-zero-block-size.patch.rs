// Fix for Bug 12: DefaultWorkerSelector::select_worker panics on zero block_size
// File: lib/kv-router/src/scheduling/selector.rs
// Severity: HIGH
//
// Problem: isl.div_ceil(block_size as usize) panics with division by zero when block_size == 0.
//          Additionally, (prefill_token as f64) / (block_size as f64) produces Inf, causing
//          NaN propagation in the softmax scoring.
// Fix: Early return with an error when block_size == 0.

// === ORIGINAL (lines 97-117) ===
// impl<C: WorkerConfigLike> WorkerSelector<C> for DefaultWorkerSelector {
//     fn select_worker(
//         &self,
//         workers: &HashMap<WorkerId, C>,
//         request: &SchedulingRequest,
//         block_size: u32,
//     ) -> Result<WorkerSelectionResult, KvSchedulerError> {
//         assert!(request.isl_tokens > 0);
//
//         let allowed_ids = request.allowed_worker_ids.as_ref();
//
//         if allowed_ids.map_or(workers.is_empty(), |ids| {
//             !workers.keys().any(|wid| ids.contains(wid))
//         }) {
//             return Err(KvSchedulerError::NoEndpoints);
//         }
//
//         let isl = request.isl_tokens;
//         let request_blocks = isl.div_ceil(block_size as usize);  // div by zero

// === FIXED ===
impl<C: WorkerConfigLike> WorkerSelector<C> for DefaultWorkerSelector {
    fn select_worker(
        &self,
        workers: &HashMap<WorkerId, C>,
        request: &SchedulingRequest,
        block_size: u32,
    ) -> Result<WorkerSelectionResult, KvSchedulerError> {
        assert!(request.isl_tokens > 0);

        if block_size == 0 {
            return Err(KvSchedulerError::NoEndpoints);
        }

        let allowed_ids = request.allowed_worker_ids.as_ref();

        if allowed_ids.map_or(workers.is_empty(), |ids| {
            !workers.keys().any(|wid| ids.contains(wid))
        }) {
            return Err(KvSchedulerError::NoEndpoints);
        }

        let isl = request.isl_tokens;
        let request_blocks = isl.div_ceil(block_size as usize);
        // ... rest unchanged ...
    }
}

// === TEST ===
#[test]
fn test_select_worker_zero_block_size_returns_error() {
    use std::collections::HashMap;

    let selector = DefaultWorkerSelector::default();
    let mut workers = HashMap::new();
    workers.insert(0u64, TestConfig::default());

    let request = SchedulingRequest {
        isl_tokens: 100,
        overlaps: OverlapScores::new(),
        ..Default::default()
    };

    // Must not panic, should return Err
    let result = selector.select_worker(&workers, &request, 0);
    assert!(result.is_err());
}
