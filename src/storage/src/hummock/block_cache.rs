// Copyright 2022 Singularity Data
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use std::sync::Arc;

use futures::Future;

use super::cache::{CachableEntry, LookupResult, LruCache};
use super::{Block, HummockError, HummockResult};

const CACHE_SHARD_BITS: usize = 6; // It means that there will be 64 shards lru-cache to avoid lock conflict.
const DEFAULT_OBJECT_POOL_SIZE: usize = 1024; // we only need a small object pool because when the cache reach the limit of capacity, it will
                                              // always release some object after insert a new block.

enum BlockEntry {
    Cache(CachableEntry<(u64, u64), Box<Block>>),
    Owned(Box<Block>),
}

pub struct BlockHolder {
    _handle: BlockEntry,
    block: *const Block,
}

impl BlockHolder {
    pub fn from_owned_block(block: Box<Block>) -> Self {
        let ptr = block.as_ref() as *const _;
        Self {
            _handle: BlockEntry::Owned(block),
            block: ptr,
        }
    }

    pub fn from_cached_block(entry: CachableEntry<(u64, u64), Box<Block>>) -> Self {
        let ptr = entry.value().as_ref() as *const _;
        Self {
            _handle: BlockEntry::Cache(entry),
            block: ptr,
        }
    }
}

impl Deref for BlockHolder {
    type Target = Block;

    fn deref(&self) -> &Self::Target {
        unsafe { &(*self.block) }
    }
}

unsafe impl Send for BlockHolder {}
unsafe impl Sync for BlockHolder {}

pub struct BlockCache {
    inner: Arc<LruCache<(u64, u64), Box<Block>>>,
}

impl BlockCache {
    pub fn new(capacity: usize) -> Self {
        let cache = LruCache::new(CACHE_SHARD_BITS, capacity, DEFAULT_OBJECT_POOL_SIZE);
        Self {
            inner: Arc::new(cache),
        }
    }

    pub fn get(&self, sst_id: u64, block_idx: u64) -> Option<BlockHolder> {
        self.inner
            .lookup(Self::hash(sst_id, block_idx), &(sst_id, block_idx))
            .map(BlockHolder::from_cached_block)
    }

    pub fn insert(&self, sst_id: u64, block_idx: u64, block: Box<Block>) {
        self.inner.insert(
            (sst_id, block_idx),
            Self::hash(sst_id, block_idx),
            block.len(),
            block,
        );
    }

    pub async fn get_or_insert_with<F>(
        &self,
        sst_id: u64,
        block_idx: u64,
        f: F,
    ) -> HummockResult<BlockHolder>
    where
        F: Future<Output = HummockResult<Box<Block>>>,
    {
        let h = Self::hash(sst_id, block_idx);
        let key = (sst_id, block_idx);
        match self.inner.lookup_for_request(h, key) {
            LookupResult::Cached(entry) => Ok(BlockHolder::from_cached_block(entry)),
            LookupResult::WaitPendingRequest(recv) => {
                let entry = recv.await.map_err(HummockError::other)?;
                Ok(BlockHolder::from_cached_block(entry))
            }
            LookupResult::Miss => match f.await {
                Ok(block) => {
                    let entry = self.inner.insert(key, h, block.len(), block);
                    Ok(BlockHolder::from_cached_block(entry))
                }
                Err(e) => {
                    self.inner.clear_pending_request(&key, h);
                    Err(e)
                }
            },
        }
    }

    fn hash(sst_id: u64, block_idx: u64) -> u64 {
        let mut hasher = DefaultHasher::default();
        sst_id.hash(&mut hasher);
        block_idx.hash(&mut hasher);
        hasher.finish()
    }

    #[cfg(test)]
    pub fn clear(&self) {
        self.inner.clear();
    }
}
