// Fix for Bug 11: RequestExtraInfo::to_block_level panics on zero block_size
// File: lib/kv-router/src/protocols.rs
// Severity: HIGH
//
// Problem: Three division-by-zero paths in to_block_level when block_size == 0:
//          total_tokens.div_ceil(block_size), req_start / block_size, and
//          (req_end.saturating_sub(1)) / block_size all panic.
// Fix: Early return with empty Vec when block_size == 0 or total_tokens == 0.

// === ORIGINAL (lines 466-472) ===
// pub fn to_block_level(
//     &self,
//     block_size: usize,
//     total_tokens: usize,
// ) -> Vec<Option<BlockExtraInfo>> {
//     let num_blocks = total_tokens.div_ceil(block_size);  // panics when block_size == 0
//     let mut block_infos: Vec<Option<BlockExtraInfo>> = vec![None; num_blocks];

// === FIXED ===
    pub fn to_block_level(
        &self,
        block_size: usize,
        total_tokens: usize,
    ) -> Vec<Option<BlockExtraInfo>> {
        if block_size == 0 || total_tokens == 0 {
            return Vec::new();
        }
        let num_blocks = total_tokens.div_ceil(block_size);
        let mut block_infos: Vec<Option<BlockExtraInfo>> = vec![None; num_blocks];

        for req_mm_obj in &self.mm_objects {
            for (req_start, req_end) in &req_mm_obj.offsets {
                let start_block = req_start / block_size;
                let end_block = (req_end.saturating_sub(1)) / block_size;
                // ... rest unchanged ...
            }
        }

        block_infos
    }

// === TEST ===
#[test]
fn test_to_block_level_zero_block_size() {
    let info = RequestExtraInfo {
        mm_objects: vec![RequestMmObjectInfo {
            mm_hash: 42,
            offsets: vec![(0, 1)],
        }],
    };
    // Must not panic, should return empty vec
    let result = info.to_block_level(0, 10);
    assert!(result.is_empty());
}

#[test]
fn test_to_block_level_zero_total_tokens() {
    let info = RequestExtraInfo {
        mm_objects: vec![RequestMmObjectInfo {
            mm_hash: 42,
            offsets: vec![(0, 1)],
        }],
    };
    // Must not panic, should return empty vec
    let result = info.to_block_level(4, 0);
    assert!(result.is_empty());
}
