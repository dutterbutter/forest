mod blocksync_message;
mod codec;

pub use self::blocksync_message::*;
pub use self::codec::*;

use super::rpc::RPCProtocol;
use libp2p::core::UpgradeInfo;

pub struct BlockSyncProtocol;

impl RPCProtocol for BlockSyncProtocol {
    type Request = BlockSyncRequest;
    type Response = BlockSyncResponse;

    type InboundCodec = InboundCodec;
    fn inbound_codec(&self, protocol: <Self as UpgradeInfo>::Info) -> Self::InboundCodec {
        Self::InboundCodec
    }

    type OutboundCodec = OutboundCodec;
    fn outbound_codec(&self, protocol: <Self as UpgradeInfo>::Info) -> Self::OutboundCodec {
        Self::OutboundCodec
    }
}
