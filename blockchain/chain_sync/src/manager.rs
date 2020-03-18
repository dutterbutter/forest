// Copyright 2020 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

#![allow(dead_code)]

use super::bucket::{SyncBucket, SyncBucketSet};
use async_std::prelude::*;
use async_std::sync::{channel, Receiver, Sender};
use async_std::task;
use blocks::Tipset;
use libp2p::core::PeerId;
use log::{info, warn};
use std::collections::HashMap;
use std::sync::Arc;

/// Manages tipsets pulled from network to be synced
#[derive(Clone)]
pub struct SyncManager {
    /// Represents current sync status
    status: SyncStatus,
    /// Queue for
    sync_queue: SyncBucketSet,
    /// Represents next tipset to be synced
    next_sync_target: SyncBucket,
    /// Represents peers and proposed tipsets from the network
    peer_heads: HashMap<PeerId, Arc<Tipset>>,

    /// Syncing channels
    sync_receiver: Receiver<SyncEvents>,
    sync_sender: Sender<SyncEvents>,
}

/// Results of the sync process
struct SyncResults {
    tipsets: Arc<Tipset>,
    success: bool,
}

impl SyncResults {
    /// constructor
    pub fn new(ts: Arc<Tipset>, success: bool) -> Self {
        Self {
            tipsets: ts,
            success,
        }
    }
}

/// Sync events...
#[derive(Clone)]
pub enum SyncEvents {
    /// New incoming tipsets from the network tbe be potentially synced
    NewTipsets { tipsets: Arc<Tipset> },
    /// TBD
    _Worker { _tipsets: Tipset },
    /// Tipsets that will be attempted to be synced
    Targets { tipsets: Arc<Tipset> },
    /// Results of syncing tipsets
    Results {
        _tipsets: Arc<Tipset>,
        _success: bool,
    },
}

/// SyncStatus represents the current state of the sync manager
#[derive(PartialEq, Debug, Clone, Copy)]
pub enum SyncStatus {
    /// Initial sync state
    Init,
    /// Indicates target tipsets have been added to the queue
    Ready,
    /// Indicates tipsets will be synced in the near future
    Scheduled,
    /// Indicates the completion of a successful sync
    Completed,
}

impl Default for SyncManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SyncManager {
    /// constructor
    pub fn new() -> Self {
        let (sync_sender, sync_receiver) = channel(20);

        Self {
            sync_queue: SyncBucketSet::default(),
            next_sync_target: SyncBucket::default(),
            peer_heads: HashMap::new(),
            sync_receiver,
            sync_sender,
            status: SyncStatus::Init,
        }
    }

    /// Sets
    pub async fn set_peer_head(&mut self, peer: &PeerId, ts: Arc<Tipset>) {
        // update peer heads map
        self.peer_heads.insert(peer.clone(), ts.clone());
        if self.get_status() == &SyncStatus::Init {
            // ensure we have at least one peer to begin syncing process
            const MIN_PEERS: usize = 1;
            if self.peer_count() >= MIN_PEERS {
                if let Some(best_target) = self.select_sync_target() {
                    self.set_status(SyncStatus::Ready);
                    self.sync_sender
                        .send(SyncEvents::NewTipsets {
                            tipsets: best_target,
                        })
                        .await;
                }
            }
            info!("sync bootstrap has {} peers", self.peer_count());
            return;
        }

        self.sync_sender
            .send(SyncEvents::NewTipsets { tipsets: ts })
            .await;
    }

    /// Retrieves the heaviest tipset in the sync queue; considered best target head
    pub fn select_sync_target(&mut self) -> Option<Arc<Tipset>> {
        let mut heads = Vec::new();
        for (_, ts) in self.peer_heads.clone() {
            heads.push(ts);
        }
        heads.sort_by_key(|header| (*header.epoch()));

        for (_, ts) in self.peer_heads.clone() {
            self.sync_queue.insert(ts);
        }

        if self.sync_queue.buckets().len() > 1 {
            warn!("caution, multiple distinct chains seen during head selections");
        }
        self.sync_queue.heaviest()
    }

    /// Triages sync events
    pub async fn _sync_triage(mut self) {
        let mut receiver = self.sync_receiver.clone();
        task::spawn(async move {
            loop {
                match receiver.next().await {
                    Some(SyncEvents::NewTipsets { tipsets }) => {
                        self.schedule_tipset(tipsets).await;
                    }
                    Some(SyncEvents::_Worker { _tipsets }) => {
                        // do something
                    }
                    Some(SyncEvents::Targets { tipsets }) => {
                        // TODO call ChainSyncer sync here!!!

                        self.sync_sender
                            .send(SyncEvents::Results {
                                _tipsets: tipsets,
                                _success: true,
                            })
                            .await;
                    }
                    Some(SyncEvents::Results { _tipsets, _success }) => {
                        self.process_result(SyncResults::new(_tipsets, _success))
                            .await;
                    }
                    None => break,
                }
            }
        });
    }

    /// Schedules a new tipset to be handled by the sync manager
    async fn schedule_tipset(&mut self, tipset: Arc<Tipset>) {
        info!("scheduling incoming tipsets to sync: {:?}", tipset.cids());

        // check sync status if indicates tipsets are ready to be synced
        if self.get_status() == &SyncStatus::Ready {
            // set the sync status to scheduled
            self.set_status(SyncStatus::Scheduled);
            // send tipsets to be synced
            self.sync_sender
                .send(SyncEvents::Targets {
                    tipsets: tipset.clone(),
                })
                .await
        }

        // TODO logic for dealing with tipset already included in active sync

        // check if status indicates SCHEDULED; insert into bucket for future syncing
        if self.get_status() == &SyncStatus::Scheduled {
            self.sync_queue.insert(tipset.clone());
        }

        if !self.next_sync_target.is_empty() && self.next_sync_target.same_chain_as(tipset.clone())
        {
            self.next_sync_target.add(tipset);
        } else {
            self.sync_queue.insert(tipset.clone());
            if self.next_sync_target.is_empty() {
                if let Some(target_bucket) = self.sync_queue.pop() {
                    self.next_sync_target = target_bucket;
                    if let Some(heaviest_target) = self.next_sync_target.heaviest_tipset() {
                        self.sync_sender
                            .send(SyncEvents::Targets {
                                tipsets: heaviest_target,
                            })
                            .await
                    }
                }
            }
        }
    }

    /// Process results of synced tipsets from the network
    async fn process_result(&mut self, results: SyncResults) {
        if results.success && self.get_status() != &SyncStatus::Completed {
            self.set_status(SyncStatus::Completed);
        }
        // TODO remove tipset from active sync map
        // TODO deal with active sync tipsets

        if self.next_sync_target.is_empty() && !self.sync_queue.is_empty() {
            if let Some(target_bucket) = self.sync_queue.pop() {
                self.next_sync_target = target_bucket;
                if let Some(heaviest_target) = self.next_sync_target.heaviest_tipset() {
                    self.sync_sender
                        .send(SyncEvents::Targets {
                            tipsets: heaviest_target,
                        })
                        .await
                }
            }
        }
    }

    // /// Worker that initiates ChainSyncer to sync with Tipset
    // async fn sync_worker(mut self) {
    //     let mut receiver = self.exec_receiver.clone();

    //     task::spawn(async move {
    //         loop {
    //             match receiver.next().await {
    //                 Some(SyncTarget::_Targets { _tipsets }) => {
    //                     // TODO call ChainSyncer sync here!!!
    //                     self.exec_sender
    //                         .send(SyncTarget::_Results {
    //                             _tipsets,
    //                             _success: true,
    //                         })
    //                         .await
    //                 }
    //                 Some(SyncTarget::_Results { _tipsets, _success }) => {
    //                     self.process_result(SyncResults::_new(_tipsets, _success));
    //                 }
    //                 None => break,
    //             }
    //         }
    //     });
    // }

    /// schedule work sent
    async fn _schedule_work_sent(&mut self) {
        let _hts = self.next_sync_target.heaviest_tipset();
        if !self.sync_queue.is_empty() && self.next_sync_target.is_empty() {
            if let Some(target_bucket) = self.sync_queue.pop() {
                self.next_sync_target = target_bucket;
                if let Some(heaviest_target) = self.next_sync_target.heaviest_tipset() {
                    self.sync_sender
                        .send(SyncEvents::Targets {
                            tipsets: heaviest_target,
                        })
                        .await
                }
            }
        } else {
            // do something
        }
    }

    /// Returns the number of peers
    fn peer_count(&self) -> usize {
        self.peer_heads.clone().keys().len()
    }
    /// Returns the managed sync status
    pub fn get_status(&self) -> &SyncStatus {
        &self.status
    }
    /// Sets the managed sync status
    pub fn set_status(&mut self, new_status: SyncStatus) {
        self.status = new_status
    }
}
