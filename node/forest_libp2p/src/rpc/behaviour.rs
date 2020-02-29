// Copyright 2020 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use super::handler::RPCHandler;
use super::{RPCEvent, RPCProtocol};
use futures::prelude::*;
use futures::task::Context;
use futures_util::task::Poll;
use libp2p::core::ConnectedPoint;
use libp2p::swarm::{
    protocols_handler::ProtocolsHandler, NetworkBehaviour, NetworkBehaviourAction, PollParameters,
};
use libp2p::{Multiaddr, PeerId};
use std::marker::PhantomData;

/// The RPC behaviour that gets consumed by the Swarm.
pub struct RPC<TSubstream, P: RPCProtocol> {
    /// Queue of events to processed.
    events: Vec<NetworkBehaviourAction<RPCEvent<P::Request, P::Response>, RPCMessage<P>>>,
    /// Pins the generic substream.
    marker: PhantomData<TSubstream>,
}

impl<TSubstream, P: RPCProtocol> RPC<TSubstream, P> {
    /// Creates a new RPC behaviour
    pub fn new() -> Self {
        RPC::default()
    }

    /// Send an RPCEvent to a peer specified by peer_id.
    pub fn send_rpc(&mut self, peer_id: PeerId, event: RPCEvent) {
        self.events
            .push(NetworkBehaviourAction::SendEvent { peer_id, event });
    }
}

impl<TSubstream, P> Default for RPC<TSubstream, P> {
    fn default() -> Self {
        RPC {
            events: vec![],
            marker: PhantomData,
        }
    }
}

/// Messages sent to the user from the RPC protocol.
#[derive(Debug)]
pub enum RPCMessage<P: RPCProtocol> {
    RPC(PeerId, RPCEvent<P::Request, P::Response>),
    PeerDialed(PeerId),
    PeerDisconnected(PeerId),
}

impl<TSubstream, P> NetworkBehaviour for RPC<TSubstream, P>
where
    P: RPCProtocol + Default + Clone,
    TSubstream: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    type ProtocolsHandler = RPCHandler<TSubstream, P>;
    type OutEvent = RPCMessage;
    fn new_handler(&mut self) -> Self::ProtocolsHandler {
        RPCHandler::default()
    }

    fn addresses_of_peer(&mut self, _: &PeerId) -> Vec<Multiaddr> {
        vec![]
    }

    fn inject_connected(&mut self, peer_id: PeerId, connected_point: ConnectedPoint) {
        if let ConnectedPoint::Dialer { .. } = connected_point {
            self.events.push(NetworkBehaviourAction::GenerateEvent(
                RPCMessage::PeerDialed(peer_id),
            ));
        }
    }

    fn inject_disconnected(&mut self, peer_id: &PeerId, connected_point: ConnectedPoint) {
        if let ConnectedPoint::Dialer { .. } = connected_point {
            self.events.push(NetworkBehaviourAction::GenerateEvent(
                RPCMessage::PeerDisconnected(peer_id.clone()),
            ));
        }
    }

    fn inject_node_event(
        &mut self,
        peer_id: PeerId,
        event: <Self::ProtocolsHandler as ProtocolsHandler>::OutEvent,
    ) {
        self.events
            .push(NetworkBehaviourAction::GenerateEvent(RPCMessage::RPC(
                peer_id, event,
            )))
    }

    fn poll(
        &mut self,
        _: &mut Context,
        _: &mut impl PollParameters,
    ) -> Poll<
        NetworkBehaviourAction<
            <Self::ProtocolsHandler as ProtocolsHandler>::InEvent,
            Self::OutEvent,
        >,
    > {
        if !self.events.is_empty() {
            return Poll::Ready(self.events.remove(0));
        }
        Poll::Pending
    }
}
