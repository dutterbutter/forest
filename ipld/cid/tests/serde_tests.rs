// // Copyright 2020 ChainSafe Systems
// // SPDX-License-Identifier: Apache-2.0, MIT

// #![cfg(feature = "serde_derive")]

// use forest_cid::{Cid, Codec};
// use multihash::Blake2b256;
// use serde_cbor::{from_slice, to_vec};

// #[test]
// fn vector_cid_serialize_round() {
//     let cids = vec![
//         Cid::new_v1(Codec::DagCBOR, Blake2b256::digest(&[0, 1])),
//         Cid::new_v1(Codec::DagCBOR, Blake2b256::digest(&[1, 2])),
//         Cid::new_v1(Codec::DagCBOR, Blake2b256::digest(&[3, 2])),
//     ];

//     // Serialize cids with cbor
//     let enc = to_vec(&cids).unwrap();

//     // decode cbor bytes to vector again
//     let dec: Vec<Cid> = from_slice(&enc).unwrap();

//     assert_eq!(cids, dec);
// }
