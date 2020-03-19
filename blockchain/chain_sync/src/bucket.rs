// Copyright 2020 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use blocks::Tipset;

/// SyncBucket defines a bucket of tipsets to sync
#[derive(Clone, Default, PartialEq, PartialOrd, Ord, Eq)]
pub struct SyncBucket {
    tips: Vec<Tipset>,
}

impl SyncBucket {
    /// Constructor for tipset bucket
    fn new(tips: Vec<Tipset>) -> SyncBucket {
        Self { tips }
    }
    /// heaviest_tipset returns the tipset with the max weight
    pub fn heaviest_tipset(&self) -> Option<Tipset> {
        if self.tips.is_empty() {
            return None;
        }

        // return max value pointer
        self.tips.iter().max_by_key(|a| a.weight()).cloned()
    }
    pub fn same_chain_as(&mut self, ts: &Tipset) -> bool {
        for t in self.tips.iter_mut() {
            // TODO Confirm that comparing keys will be sufficient on full tipset impl
            if ts.key() == t.key() || ts.key() == t.parents() || ts.parents() == t.key() {
                return true;
            }
        }

        false
    }
    pub fn add(&mut self, ts: Tipset) {
        if !self.tips.iter().any(|t| *t == ts) {
            self.tips.push(ts);
        }
    }
    /// Returns true if SyncBucket is empty
    pub fn _is_empty(&self) -> bool {
        self.tips.is_empty()
    }
}

/// Set of tipset buckets
#[derive(Default, Clone)]
pub(crate) struct SyncBucketSet {
    buckets: Vec<SyncBucket>,
}

impl SyncBucketSet {
    pub(crate) fn insert(&mut self, tipset: Tipset) {
        for b in self.buckets.iter_mut() {
            if b.same_chain_as(&tipset) {
                b.add(tipset);
                return;
            }
        }
        self.buckets.push(SyncBucket::new(vec![tipset]))
    }
    /// Removes the SyncBucket with heaviest weighted Tipset from SyncBucketSet
    pub(crate) fn _pop(&mut self) -> Option<SyncBucket> {
        if let Some(heaviest_bucket) = self.buckets().iter().max_by_key(|b| b.heaviest_tipset()) {
            self.clone()._remove(heaviest_bucket);
            Some(heaviest_bucket.clone())
        } else {
            None
        }
    }
    pub(crate) fn heaviest(&self) -> Option<Tipset> {
        // Transform max values from each bucket into a Vec
        let vals: Vec<Tipset> = self
            .buckets
            .iter()
            .filter_map(|b| b.heaviest_tipset())
            .collect();

        // Return the heaviest tipset bucket
        vals.iter().max_by_key(|b| b.weight()).cloned()
    }
    /// Updates SyncBucketSet by removing specified SyncBucket
    pub(crate) fn _remove(&mut self, ts_bucket: &SyncBucket) {
        let vals: Vec<SyncBucket> = self
            .buckets
            .clone()
            .into_iter()
            .filter(|b| b != ts_bucket)
            .collect();

        self.buckets = vals;
    }
    /// Removes SyncBucket specified by provided Tipset
    pub(crate) fn _pop_related(&mut self, ts: Tipset) {
        for b in self.buckets() {
            if b.clone().same_chain_as(&ts.clone()) {
                self.clone()._remove(b)
            }
        }
    }
    /// Returns a vector of SyncBuckets
    pub(crate) fn buckets(&self) -> &Vec<SyncBucket> {
        &self.buckets
    }
    /// Returns true if SyncBucket is empty
    pub(crate) fn _is_empty(&self) -> bool {
        if !self.buckets.is_empty() {
            return false;
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use blocks::BlockHeader;
    use cid::{multihash::Blake2b256, Cid};
    use num_bigint::BigUint;

    fn create_header(weight: u64, parent_bz: &[u8], cached_bytes: &[u8]) -> BlockHeader {
        let header = BlockHeader::builder()
            .weight(BigUint::from(weight))
            .cached_bytes(cached_bytes.to_vec())
            .cached_cid(Cid::new_from_cbor(parent_bz, Blake2b256).unwrap())
            .build()
            .unwrap();
        header
    }

    #[test]
    fn base_bucket_constructor() {
        SyncBucket::new(Vec::new());
    }

    #[test]
    fn heaviest_tipset() {
        let l_tip = Tipset::new(vec![create_header(1, b"", b"")]).unwrap();
        let h_tip = Tipset::new(vec![create_header(3, b"", b"")]).unwrap();

        // Test the comparison of tipsets
        let bucket = SyncBucket::new(vec![l_tip.clone(), h_tip]);
        assert_eq!(
            bucket.heaviest_tipset().unwrap().weight(),
            &BigUint::from(3u8)
        );
        assert_eq!(bucket.tips.len(), 2);

        // assert bucket with just one tipset still resolves
        let bucket = SyncBucket::new(vec![l_tip]);
        assert_eq!(
            bucket.heaviest_tipset().unwrap().weight(),
            &BigUint::from(1u8)
        );
    }

    #[test]
    fn sync_bucket_inserts() {
        let mut set = SyncBucketSet::default();
        let tipset1 = Tipset::new(vec![create_header(1, b"1", b"1")]).unwrap();
        set.insert(tipset1.clone());
        assert_eq!(set.buckets.len(), 1);
        assert_eq!(set.buckets[0].tips.len(), 1);

        // Assert a tipset on non relating chain is put in another bucket
        let tipset2 = Tipset::new(vec![create_header(2, b"2", b"2")]).unwrap();
        set.insert(tipset2);
        assert_eq!(
            set.buckets.len(),
            2,
            "Inserting seperate tipset should create new bucket"
        );
        assert_eq!(set.buckets[1].tips.len(), 1);

        // Assert a tipset connected to the first
        let tipset3 = Tipset::new(vec![create_header(3, b"1", b"1")]).unwrap();
        assert_eq!(tipset1.key(), tipset3.key());
        set.insert(tipset3);
        assert_eq!(
            set.buckets.len(),
            2,
            "Inserting into first chain should not create 3rd bucket"
        );
        assert_eq!(
            set.buckets[0].tips.len(),
            2,
            "Should be 2 tipsets in bucket 0"
        );

        // Assert that tipsets that are already added are not added twice
        set.insert(tipset1);
        assert_eq!(set.buckets.len(), 2);
        assert_eq!(set.buckets[0].tips.len(), 2);
    }
}
