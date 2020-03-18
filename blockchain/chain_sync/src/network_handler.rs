// Copyright 2020 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use super::sync::ChainSyncer;
use super::peer_manager::PeerManager;
use async_std::prelude::*;
use async_std::sync::{Receiver, Sender};
use async_std::task;
use blocks::{FullTipset};
use forest_libp2p::rpc::{RPCResponse, RequestId};
use forest_libp2p::NetworkEvent;
use log::trace;
use log::warn;
use std::sync::Arc;

pub(crate) type RPCReceiver = Receiver<(RequestId, RPCResponse)>;
pub(crate) type RPCSender = Sender<(RequestId, RPCResponse)>;

/// Handles network events from channel and splits based on request
pub struct NetworkHandler {
    rpc_send: RPCSender,
    event_send: Sender<NetworkEvent>,
    receiver: Receiver<NetworkEvent>,
}

impl NetworkHandler {
    pub fn new(
        receiver: Receiver<NetworkEvent>,
        rpc_send: RPCSender,
        event_send: Sender<NetworkEvent>,
    ) -> Self {
        Self {
            receiver,
            rpc_send,
            event_send,
        }
    }

    pub(crate) fn spawn(&self, peer_manager: Arc<PeerManager>, syncer: ChainSyncer<DB, ST>) {
        let mut receiver = self.receiver.clone();
        let rpc_send = self.rpc_send.clone();
        let event_send = self.event_send.clone();

        task::spawn(async move {
            loop {
                match receiver.next().await {
                    // Handle specifically RPC responses and send to that channel
                    Some(NetworkEvent::RPCResponse { req_id, response }) => {
                        rpc_send.send((req_id, response)).await
                    }
                    // Pass any non RPC responses through event channel
                    Some(event) => {
                        // Update peer on this thread before sending hello
                        if let NetworkEvent::Hello { source, message } = &event {
                            peer_manager.add_peer(source.clone()).await;
                            let fts: FullTipset = match syncer.fetch_tipset(source.clone(), message.heaviest_tip_set) {
                                Ok(ts) => ts,
                                Err(e) => {
                                    // TODO temporary
                                    warn!("Failed to fetch full tipset from block sync");
                                    peer_manager.remove_peer(&source.clone());
                                    continue;
                                }
                            };
                            syncer.inform_new_head(source.clone(), &fts);
                        }

                        // TODO revisit, doing this to avoid blocking this thread but can handle better
                        if !event_send.is_full() {
                            event_send.send(event).await
                        }
                    }
                    None => break,
                }
            }
        });
        trace!("Spawned network handler");
    }
}
