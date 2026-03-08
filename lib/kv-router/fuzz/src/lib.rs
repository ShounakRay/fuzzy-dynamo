use arbitrary::Arbitrary;
use dynamo_kv_router::protocols::{
    ExternalSequenceBlockHash, KvCacheEvent, KvCacheEventData, KvCacheRemoveData, KvCacheStoreData,
    KvCacheStoredBlockData, LocalBlockHash, RouterEvent,
};

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
