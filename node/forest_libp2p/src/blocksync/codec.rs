// Copyright 2020 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use super::{BlockSyncRequest, BlockSyncResponse};
use crate::rpc::RPCError;
use bytes::BytesMut;
use forest_encoding::{from_slice, to_vec};
use futures_codec::{Decoder, Encoder};

/// Codec used for inbound connections. Decodes the inbound message into a BlockSyncRequest, and encodes the BlockSyncResponse to send.
pub struct InboundCodec;
/// Codec used for outbound connections. Encodes the outbound message into a BlockSyncRequest to send, and decodes the BlockSyncResponse when received.
pub struct OutboundCodec;

impl Encoder for InboundCodec {
    type Error = RPCError;
    type Item = BlockSyncResponse;

    fn encode(&mut self, item: Self::Item, dst: &mut BytesMut) -> Result<(), Self::Error> {
        let resp = to_vec(&item)?;
        dst.clear();
        dst.extend_from_slice(&resp);
        Ok(())
    }
}

impl Decoder for InboundCodec {
    type Error = RPCError;
    type Item = BlockSyncRequest;

    fn decode(&mut self, bz: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if bz.is_empty() {
            return Ok(None);
        }

        Ok(Some(
            from_slice(bz).map_err(|err| RPCError::Codec(err.to_string()))?,
        ))
    }
}

impl Encoder for OutboundCodec {
    type Error = RPCError;
    type Item = BlockSyncRequest;

    fn encode(&mut self, item: Self::Item, dst: &mut BytesMut) -> Result<(), Self::Error> {
        let resp = to_vec(&item)?;
        dst.clear();
        dst.extend_from_slice(&resp);
        Ok(())
    }
}
impl Decoder for OutboundCodec {
    type Error = RPCError;
    type Item = BlockSyncResponse;

    fn decode(&mut self, bz: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if bz.is_empty() {
            return Ok(None);
        }

        Ok(Some(
            // Replace map
            from_slice(bz).map_err(|err| RPCError::Codec(err.to_string()))?,
        ))
    }
}
