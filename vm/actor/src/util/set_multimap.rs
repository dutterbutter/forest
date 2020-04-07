// Copyright 2020 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use super::Set;
use crate::{parse_uint_key, u64_key, DealID, HAMT_BIT_WIDTH};
use address::Address;
use cid::Cid;
use ipld_blockstore::BlockStore;
use ipld_hamt::{Error, Hamt};

/// SetMultimap is a hamt with values that are also a hamt but are of the set variant.
/// This allows hash sets to be indexable by an address.
pub struct SetMultimap<'a, BS>(Hamt<'a, String, BS>);
impl<'a, BS> SetMultimap<'a, BS>
where
    BS: BlockStore,
{
    /// Initializes a new empty SetMultimap.
    pub fn new(bs: &'a BS) -> Self {
        Self(Hamt::new_with_bit_width(bs, HAMT_BIT_WIDTH))
    }

    /// Initializes a SetMultimap from a root Cid.
    pub fn from_root(bs: &'a BS, cid: &Cid) -> Result<Self, Error> {
        Ok(Self(Hamt::load_with_bit_width(cid, bs, HAMT_BIT_WIDTH)?))
    }

    /// Retrieve root from the SetMultimap.
    #[inline]
    pub fn root(&mut self) -> Result<Cid, Error> {
        self.0.flush()
    }

    /// Puts the DealID in the hash set of the key.
    pub fn put(&mut self, key: &Address, value: DealID) -> Result<(), String> {
        // Get construct amt from retrieved cid or create new
        let mut set = self.get(key)?.unwrap_or_else(|| Set::new(self.0.store()));

        set.put(u64_key(value))?;

        // Save and calculate new root
        let new_root = set.root()?;

        // Set hamt node to set new root
        Ok(self.0.set(key.hash_key(), &new_root)?)
    }

    /// Gets the set at the given index of the `SetMultimap`
    #[inline]
    pub fn get(&self, key: &Address) -> Result<Option<Set<'a, BS>>, String> {
        match self.0.get(&key.hash_key())? {
            Some(cid) => Ok(Some(Set::from_root(self.0.store(), &cid)?)),
            None => Ok(None),
        }
    }

    /// Removes a DealID from a key hash set.
    #[inline]
    pub fn remove(&mut self, key: &Address, v: DealID) -> Result<(), String> {
        // Get construct amt from retrieved cid and return if no set exists
        let mut set = match self.get(key)? {
            Some(s) => s,
            None => return Ok(()),
        };

        set.delete(&u64_key(v))?;

        // Save and calculate new root
        let new_root = set.root()?;

        Ok(self.0.set(key.hash_key(), &new_root)?)
    }

    /// Removes set at index.
    #[inline]
    pub fn remove_all(&mut self, key: &Address) -> Result<(), String> {
        // Remove entry from table
        self.0.delete(&key.hash_key())?;

        Ok(())
    }

    /// Iterates through keys and converts them to a DealID to call a function on each.
    pub fn for_each<F>(&self, key: &Address, mut f: F) -> Result<(), String>
    where
        F: FnMut(DealID) -> Result<(), String>,
    {
        // Get construct amt from retrieved cid and return if no set exists
        let set = match self.get(key)? {
            Some(s) => s,
            None => return Ok(()),
        };

        set.for_each(|k| {
            let v =
                parse_uint_key(k).map_err(|e| format!("Could not parse key: {}, ({})", k, e))?;

            // Run function on all parsed keys
            f(v)
        })
    }
}