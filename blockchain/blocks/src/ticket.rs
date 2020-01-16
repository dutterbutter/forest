// Copyright 2020 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0

use crypto::VRFResult;
use serde::{Deserialize, Serialize};

/// A Ticket is a marker of a tick of the blockchain's clock.  It is the source
/// of randomness for proofs of storage and leader election.  It is generated
/// by the miner of a block using a VRF and a VDF.
#[derive(Clone, Debug, PartialEq, PartialOrd, Eq, Default, Serialize, Deserialize)]
pub struct Ticket {
    /// A proof output by running a VRF on the VDFResult of the parent ticket
    pub vrfproof: VRFResult,
}

// TODO verify format or implement custom serialize/deserialize function (if necessary):
// https://github.com/ChainSafe/ferret/issues/143

impl Ticket {
    /// Ticket constructor
    pub fn new(vrfproof: VRFResult) -> Self {
        Self { vrfproof }
    }
}
