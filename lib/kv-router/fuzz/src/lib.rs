//! Shared helpers for KV router fuzz targets.

use dynamo_kv_router::protocols::{
    ExternalSequenceBlockHash, KvCacheEvent, KvCacheEventData, KvCacheRemoveData, KvCacheStoreData,
    KvCacheStoredBlockData, LocalBlockHash, RouterEvent,
};

/// Build a store event from fuzz bytes.
pub fn make_store_event(
    worker_id: u64,
    event_id: u64,
    hashes: &[u64],
    parent: Option<u64>,
) -> RouterEvent {
    RouterEvent {
        worker_id,
        event: KvCacheEvent {
            event_id,
            data: KvCacheEventData::Stored(KvCacheStoreData {
                parent_hash: parent.map(|h| ExternalSequenceBlockHash(h * 100)),
                blocks: hashes
                    .iter()
                    .map(|&h| KvCacheStoredBlockData {
                        tokens_hash: LocalBlockHash(h),
                        block_hash: ExternalSequenceBlockHash(h * 100),
                        mm_extra_info: None,
                    })
                    .collect(),
            }),
            dp_rank: 0,
        },
    }
}

/// Build a remove event from fuzz bytes.
pub fn make_remove_event(worker_id: u64, event_id: u64, hashes: &[u64]) -> RouterEvent {
    RouterEvent {
        worker_id,
        event: KvCacheEvent {
            event_id,
            data: KvCacheEventData::Removed(KvCacheRemoveData {
                block_hashes: hashes
                    .iter()
                    .map(|&h| ExternalSequenceBlockHash(h * 100))
                    .collect(),
            }),
            dp_rank: 0,
        },
    }
}

/// Build a clear event.
pub fn make_clear_event(worker_id: u64, event_id: u64) -> RouterEvent {
    RouterEvent {
        worker_id,
        event: KvCacheEvent {
            event_id,
            data: KvCacheEventData::Cleared,
            dp_rank: 0,
        },
    }
}

/// State tracker for fuzz event loops.
/// Tracks stored hashes per worker so we can build valid parent refs
/// and valid remove targets.
pub struct FuzzEventState {
    pub pos: usize,
    pub event_id: u64,
    pub stored: std::collections::HashMap<u64, Vec<u64>>,
}

impl FuzzEventState {
    pub fn new(start_pos: usize) -> Self {
        Self {
            pos: start_pos,
            event_id: 0,
            stored: std::collections::HashMap::new(),
        }
    }

    /// Parse the next operation from fuzz data. Returns None if data exhausted.
    /// Returns (op_type, worker_id, event) where op_type is 0=store, 1=remove, 2=clear, 3=query.
    pub fn next_event(&mut self, data: &[u8]) -> Option<(u8, u64, FuzzOp)> {
        if self.pos >= data.len() {
            return None;
        }
        let op_byte = data[self.pos];
        self.pos += 1;
        if self.pos >= data.len() {
            return None;
        }
        let worker_id = (data[self.pos] % 4) as u64;
        self.pos += 1;

        let op_type = op_byte % 4;
        match op_type {
            0 => {
                // Store: read 1-4 block hashes
                let count = if self.pos < data.len() {
                    (data[self.pos] % 4) as usize + 1
                } else {
                    1
                };
                self.pos += 1;

                let mut hashes = Vec::new();
                for _ in 0..count {
                    if self.pos >= data.len() {
                        break;
                    }
                    hashes.push((data[self.pos] % 16) as u64);
                    self.pos += 1;
                }

                if hashes.is_empty() {
                    return Some((op_type, worker_id, FuzzOp::Skip));
                }

                let parent = self.stored.get(&worker_id).and_then(|v| v.last().copied());
                let event = make_store_event(worker_id, self.event_id, &hashes, parent);
                self.stored
                    .entry(worker_id)
                    .or_default()
                    .extend_from_slice(&hashes);
                self.event_id += 1;
                Some((op_type, worker_id, FuzzOp::Event(event)))
            }
            1 => {
                // Remove
                if let Some(worker_hashes) = self.stored.get_mut(&worker_id) {
                    if !worker_hashes.is_empty() {
                        let idx = if self.pos < data.len() {
                            data[self.pos] as usize % worker_hashes.len()
                        } else {
                            0
                        };
                        self.pos += 1;
                        let hash = worker_hashes.remove(idx);
                        let event = make_remove_event(worker_id, self.event_id, &[hash]);
                        self.event_id += 1;
                        return Some((op_type, worker_id, FuzzOp::Event(event)));
                    }
                }
                Some((op_type, worker_id, FuzzOp::Skip))
            }
            2 => {
                // Clear
                let event = make_clear_event(worker_id, self.event_id);
                self.stored.remove(&worker_id);
                self.event_id += 1;
                Some((op_type, worker_id, FuzzOp::Event(event)))
            }
            3 => {
                // Query
                let count = if self.pos < data.len() {
                    (data[self.pos] % 8) as usize + 1
                } else {
                    1
                };
                self.pos += 1;

                let mut seq = Vec::new();
                for _ in 0..count {
                    if self.pos >= data.len() {
                        break;
                    }
                    seq.push(LocalBlockHash((data[self.pos] % 16) as u64));
                    self.pos += 1;
                }

                if seq.is_empty() {
                    return Some((op_type, worker_id, FuzzOp::Skip));
                }

                let early_exit = self.pos < data.len() && data[self.pos] % 2 == 0;
                if self.pos < data.len() {
                    self.pos += 1;
                }

                Some((op_type, worker_id, FuzzOp::Query(seq, early_exit)))
            }
            _ => unreachable!(),
        }
    }
}

/// Fuzz operation result.
pub enum FuzzOp {
    Event(RouterEvent),
    Query(Vec<LocalBlockHash>, bool),
    Skip,
}
