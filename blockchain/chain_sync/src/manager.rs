// Copyright 2020 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use super::bucket::SyncBucketSet;
use async_std::prelude::*;
use async_std::sync::{channel, Receiver, Sender};
use async_std::task;
use blocks::Tipset;
use libp2p::core::PeerId;
use log::warn;
use std::collections::HashMap;
use std::sync::Arc;

/// Manages tipsets pulled from network to be synced
#[derive(Clone)]
pub struct SyncManager {
    sync_queue: SyncBucketSet,
    _peer_heads: HashMap<PeerId, Tipset>,
    receiver: Receiver<SyncEvents>,
    sender: Sender<SyncEvents>,
}
#[derive(Clone)]
pub enum SyncEvents {
    _IncomingTipsets { _tipsets: Tipset },
    _Results { _tipsets: Tipset, _success: bool },
}

impl Default for SyncManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SyncManager {
    pub fn new() -> Self {
        let (sender, receiver) = channel(20);
        Self {
            sync_queue: SyncBucketSet::default(),
            _peer_heads: HashMap::new(),
            receiver,
            sender,
        }
    }
    /// Schedules a new tipset to be handled by the sync manager
    pub fn schedule_tipset(&mut self, tipset: Arc<Tipset>) {
        // TODO implement interactions for syncing state when SyncManager built out
        self.sync_queue.insert(tipset);
    }
    pub fn schedule_sync(&self) {
        // Switch case for the following:
        //  - Incoming tipsets
        //  - Incoming sync results
        //  - worker chan to add next sync target
        //  - channel to shut down scheduler

        // Steps:
        // 1. channel receiving incoming Tipsets (where sender is from set_peer_head)
        // 2. if channel is filled, call schedule_incoming(ts)
        // 3. schedule_incoming takes tipset param and sends it into the sync_target channel to be received
        // 4.

        let mut receiver = self.receiver.clone();
        task::spawn(async move {
            loop {
                match receiver.next().await {
                    Some(SyncEvents::_IncomingTipsets { _tipsets }) => {
                        // do something
                    }
                    Some(SyncEvents::_Results { _tipsets, _success }) => {
                        // do something
                    }
                    None => break,
                }
            }
        });
        todo!()
    }

    pub fn sync_worker(&self) {
        let mut receiver = self.receiver.clone();

        task::spawn(async move {
            loop {
                match receiver.next().await {
                    Some(SyncEvents::_IncomingTipsets { _tipsets }) => {
                        // do something
                    }
                    Some(SyncEvents::_Results { _tipsets, _success }) => {
                        // do something
                    }
                    None => break,
                }
            }
        });
        todo!()
    }
    /// Retrieves the heaviest tipset in the sync queue
    pub fn select_sync_target(&mut self) -> Option<Arc<Tipset>> {
        let mut peer_heads = Vec::new();
        for (_, ts) in self._peer_heads.clone() {
            peer_heads.push(ts);
        }
        peer_heads.sort_by_key(|header| (*header.epoch()));

        for (_, ts) in self._peer_heads.clone() {
            self.sync_queue.insert(Arc::new(ts));
        }

        if self.sync_queue.buckets().len() > 1 {
            warn!("caution, multiple distinct chains seen during head selections");
        }
        self.sync_queue.heaviest()
    }
    /// Sets the PeerId indicating the head tipset
    pub fn set_peer_head(&mut self, _peer: &PeerId, _ts: Tipset) {
        // TODO
        let _target = self.select_sync_target();
        todo!()
    }

    pub fn start(&self) {}

    // fn peer_count(&self) -> i32 {
    //     let mut count: i32;
    //     for (_, _ts) in self._peer_heads.clone() {
    //         count += 1;
    //     }
    //     count
    // }
}
