// Copyright 2020 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use super::bucket::{SyncBucketSet, SyncBucket};
use async_std::prelude::*;
use async_std::sync::{channel, Receiver, Sender};
use async_std::task;
use blocks::Tipset;
use libp2p::core::PeerId;
use log::{info, warn};
use std::collections::HashMap;
use std::sync::Arc;
use crate::errors::Error;

/// Manages tipsets pulled from network to be synced
#[derive(Clone)]
pub struct SyncManager {
    // syncing scheduler
    sync_queue: SyncBucketSet,
    next_sync_target: SyncBucket,


    _peer_heads: HashMap<PeerId, Tipset>,
    receiver: Receiver<SyncEvents>,
    sender: Sender<SyncEvents>,
    status: SyncStatus,
  //  doSync: fn(),
}

#[derive(Clone)]
pub enum SyncEvents {
    _NewTipsets { _tipsets: Tipset },
    _Worker { _tipsets: Tipset },
    _Targets { _tipsets: Arc<Tipset> },
    _Results { _tipsets: Arc<Tipset>, _success: bool },
}

#[derive(PartialEq, Debug, Clone)]
enum SyncStatus {
    _Init, 
    _Selected,
    _Scheduled,
    _Completed,
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
            next_sync_target: SyncBucket::default(),
            _peer_heads: HashMap::new(),
            receiver,
            sender,
            status: SyncStatus::_Init,
        }
    }
    pub fn start(&self) {
        todo!()
    }
    pub fn stop(&self) {
        todo!()
    }
    /// Schedules a new tipset to be handled by the sync manager
    async fn schedule_tipset(&mut self, tipset: Arc<Tipset>) -> Result<(), Error> {
        info!("scheduling incoming tipsets to sync: {:?}", tipset.cids());

        // check sync status if indicates tipsets are ready to be synced
        if self.get_status() == SyncStatus::_Selected {
            // set the sync status to scheduled 
            self.set_status(SyncStatus::_Scheduled);
            // send SELECTED tipsets to be synced via the sync_worker
            self.sender.send(SyncEvents::_Targets { _tipsets: tipset }).await
        }

        // TODO logic for dealing with tipset already included in active sync

        // check if status indicates SCHEDULED; insert into bucket for future syncing
        if self.get_status() == SyncStatus::_Scheduled {
            self.sync_queue.insert(tipset);
        }

        if !self.next_sync_target.is_empty() && self.next_sync_target.same_chain_as(tipset) {
            self.next_sync_target.add(tipset);
        } else {
            self.sync_queue.insert(tipset);
            if self.next_sync_target.is_empty() {
                self.next_sync_target = self.sync_queue._pop()?;
                
            }
        }


        // TODO implement interactions for syncing state when SyncManager built out
        Ok(())
    }
    /// Triages sync events 
    async fn sync_triage<'a>(&'a self) {
        let mut receiver = self.receiver.clone();
        task::spawn(async move {
            loop {
                match receiver.next().await {
                    Some(SyncEvents::_NewTipsets { _tipsets }) => {
                        let ts = Arc::new(_tipsets);
                        self.schedule_tipset(ts.clone());
                    }
                    Some(SyncEvents::_Results { _tipsets, _success }) => {
                        // do something
                    }
                    None => break,
                }
            }
        });
    }

    async fn sync_worker<'a>(&'a self) {
        let mut receiver = self.receiver.clone();

        task::spawn(async move {
            loop {
                match receiver.next().await {
                    Some(SyncEvents::_Targets { _tipsets }) => {
                        // TODO call ChainSyncer sync here!!!
                        self.sender.send(SyncEvents::_Results {
                            _tipsets: _tipsets,
                            _success: true
                        }).await
                    }
                    Some(SyncEvents::_NewTipsets { _tipsets }) => {
                        // do something
                    }
                    Some(SyncEvents::_Results { _tipsets, _success }) => {
                        // do something
                    }
                    None => break,
                }
            }
        });
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
    /// Returns the number of peers
    fn _peer_count(&self) -> usize {
        self._peer_heads.clone().keys().len()
    }
    /// Returns the managed sync status
    pub fn get_status(&self) -> SyncStatus {
        self.status
    }
    /// Sets the managed sync status
    pub fn set_status(&mut self, new_status: SyncStatus) {
        self.status = new_status
    }
}
