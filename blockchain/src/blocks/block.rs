#![allow(dead_code)]

use super::ticket::{Ticket, VRFProofIndex};
use super::TipSetKeys;

use address::Address;
use cid::{Cid, Codec, Prefix, Version};
use message::UnsignedMessage;
use multihash::Hash;

// DefaultHashFunction represents the default hashing function to use
// TODO SHOULD BE BLAKE2B
const DEFAULT_HASH_FUNCTION: Hash = Hash::Keccak256;

/// BlockHeader defines header of a block in the Filecoin blockchain
#[derive(Clone, Debug)]
pub struct BlockHeader {
    /// CHAIN LINKING
    ///
    /// Parents is the set of parents this block was based on. Typically one,
    /// but can be several in the case where there were multiple winning ticket-
    /// holders for an epoch
    pub parents: TipSetKeys,
    /// weight is the aggregate chain weight of the parent set
    pub weight: u64,
    /// epoch is the period in which a new block is generated. There may be multiple rounds in an epoch
    pub epoch: u64,
    /// height is the block height
    pub height: u64,

    /// MINER INFO
    ///
    /// miner_address is the address of the miner actor that mined this block
    pub miner_address: Address,

    /// STATE
    ///
    /// messages is the Cid of the root of an array of Messages
    pub messages: Cid,
    /// message_receipts is the Cid of the root of an array of MessageReceipts
    pub message_receipts: Cid,
    /// state_root is a cid pointer to the state tree after application of the transactions state transitions
    pub state_root: Cid,

    /// CONSENSUS
    ///
    /// timestamp, in seconds since the Unix epoch, at which this block was created
    pub timestamp: u64,
    /// ticket is the ticket submitted with this block
    pub ticket: Ticket,
    /// election_proof is the "scratched ticket" proving that this block won
    /// an election
    pub election_proof: VRFProofIndex,
    // SIGNATURES
    //
    // block_sig filCrypto Signature
    // BLSAggregateSig
    /// CACHE
    ///
    pub cached_cid: Cid,

    pub cached_bytes: u8,
}

/// Block defines a full block
pub struct Block {
    header: BlockHeader,
    messages: Vec<UnsignedMessage>,
}

impl BlockHeader {
    /// cid returns the content id of this header
    pub fn cid(&mut self) -> Cid {
        // TODO
        // Encode blockheader into cache_bytes
        // Change DEFAULT_HASH_FUNCTION to utilize blake2b
        //
        // Currently content id for headers will be incomplete until encoding and supporting libraries are completed
        let c = Prefix {
            version: Version::V1,
            codec: Codec::DagCBOR,
            mh_type: DEFAULT_HASH_FUNCTION,
            mh_len: 0,
        };
        let new_cid = Cid::new_from_prefix(&c, &[self.cached_bytes]);
        self.cached_cid = new_cid;
        self.cached_cid.clone()
    }
}