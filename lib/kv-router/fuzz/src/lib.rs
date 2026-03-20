use arbitrary::Arbitrary;
use dynamo_kv_router::protocols::{
    ExternalSequenceBlockHash, KvCacheEvent, KvCacheEventData, KvCacheRemoveData, KvCacheStoreData,
    KvCacheStoredBlockData, LocalBlockHash, RouterEvent, compute_hash,
};

/// Create a store event with correctly computed seq_hash chains.
///
/// `parent_seq_hash` is the seq_hash of the last block in the parent sequence
/// (None for the first store). Returns `(event, seq_hashes)` where seq_hashes
/// are the ExternalSequenceBlockHash values for each stored block.
pub fn make_store_event(
    worker_id: u64,
    event_id: u64,
    hashes: &[u64],
    parent_seq_hash: Option<u64>,
) -> (RouterEvent, Vec<u64>) {
    let mut seq_hashes: Vec<u64> = Vec::with_capacity(hashes.len());
    for (i, &h) in hashes.iter().enumerate() {
        let sh = if i == 0 {
            match parent_seq_hash {
                None => h,
                Some(parent) => {
                    let mut bytes = [0u8; 16];
                    bytes[..8].copy_from_slice(&parent.to_le_bytes());
                    bytes[8..].copy_from_slice(&h.to_le_bytes());
                    compute_hash(&bytes)
                }
            }
        } else {
            let mut bytes = [0u8; 16];
            bytes[..8].copy_from_slice(&seq_hashes[i - 1].to_le_bytes());
            bytes[8..].copy_from_slice(&h.to_le_bytes());
            compute_hash(&bytes)
        };
        seq_hashes.push(sh);
    }

    let event = RouterEvent {
        worker_id,
        event: KvCacheEvent {
            event_id,
            data: KvCacheEventData::Stored(KvCacheStoreData {
                parent_hash: parent_seq_hash.map(ExternalSequenceBlockHash),
                blocks: hashes
                    .iter()
                    .zip(seq_hashes.iter())
                    .map(|(&h, &sh)| KvCacheStoredBlockData {
                        tokens_hash: LocalBlockHash(h),
                        block_hash: ExternalSequenceBlockHash(sh),
                        mm_extra_info: None,
                    })
                    .collect(),
            }),
            dp_rank: 0,
        },
    };
    (event, seq_hashes)
}

/// Create a remove event. `seq_hashes` are the ExternalSequenceBlockHash values
/// of the blocks to remove (as returned by `make_store_event`).
pub fn make_remove_event(worker_id: u64, event_id: u64, seq_hashes: &[u64]) -> RouterEvent {
    RouterEvent {
        worker_id,
        event: KvCacheEvent {
            event_id,
            data: KvCacheEventData::Removed(KvCacheRemoveData {
                block_hashes: seq_hashes
                    .iter()
                    .map(|&sh| ExternalSequenceBlockHash(sh))
                    .collect(),
            }),
            dp_rank: 0,
        },
    }
}

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

#[derive(Debug, Arbitrary)]
pub enum FuzzOp {
    Store { worker_id: u8, hashes: Vec<u8> },
    Remove { worker_id: u8, index: u8 },
    Clear { worker_id: u8 },
    Query { seq: Vec<u8>, early_exit: bool },
}

#[derive(Debug, Arbitrary)]
pub struct FuzzInput {
    pub ops: Vec<FuzzOp>,
}
