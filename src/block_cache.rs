use std::{
    collections::hash_map::Entry,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};

use parking_lot::RwLock;
use rustc_hash::FxHashMap;

#[derive(Clone)]
pub struct CachedBlock {
    pub epoch: usize,
    pub data: Arc<Vec<u8>>,
}

/// Thread safe block cache that allows concurrent access and cleanup of old blocks.
pub struct BlockCache {
    current_epoch: AtomicUsize,
    blocks: RwLock<FxHashMap<usize, CachedBlock>>,
}

impl BlockCache {
    pub const MAX_BLOCKS: usize = 32;

    pub fn new() -> Self {
        BlockCache {
            current_epoch: AtomicUsize::new(0),
            blocks: RwLock::new(FxHashMap::default()),
        }
    }

    pub fn get<F>(&self, block_index: usize, read_block: F) -> anyhow::Result<Arc<Vec<u8>>>
    where
        F: FnOnce(usize) -> anyhow::Result<Vec<u8>>,
    {
        let _span = tracing::debug_span!("PackageCommonD2::get_block", block_index).entered();
        let CachedBlock { data, .. } = match self.blocks.write().entry(block_index) {
            Entry::Occupied(o) => o.get().clone(),
            Entry::Vacant(v) => {
                let block = read_block(*v.key())?;
                let b = v
                    .insert(CachedBlock {
                        epoch: self.current_epoch.fetch_add(1, Ordering::Relaxed),
                        data: Arc::new(block),
                    })
                    .clone();

                b
            }
        };

        self.remove_old_blocks();

        Ok(data)
    }

    fn remove_old_blocks(&self) {
        while self.blocks.read().len() > Self::MAX_BLOCKS {
            let bc = self.blocks.read();
            let (oldest, _) = bc
                .iter()
                .min_by(|(_, a), (_, b)| a.epoch.cmp(&b.epoch))
                .unwrap();

            let oldest = *oldest;
            drop(bc);

            self.blocks.write().remove(&oldest);
        }
    }
}
