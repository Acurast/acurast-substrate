#![allow(dead_code)]

use derive_more::{From, Into};
use hex_literal::hex;
use sp_core::H256;
use sp_runtime::AccountId32;
use sp_std::prelude::*;
#[cfg(not(feature = "std"))]
use sp_std::prelude::*;

use crate::{StateKey, StateOwner, StateProof, StateProofNode, StateValue};

#[derive(Debug, From, Into, Clone, Eq, PartialEq)]
pub struct AcurastAccountId(AccountId32);
impl TryFrom<Vec<u8>> for AcurastAccountId {
    type Error = ();

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        let a: [u8; 32] = value.try_into().map_err(|_| ())?;
        Ok(AcurastAccountId(AccountId32::new(a)))
    }
}

pub fn alice_account_id() -> AccountId32 {
    [0; 32].into()
}
pub fn bob_account_id() -> AccountId32 {
    [1; 32].into()
}
pub const HASH: H256 = H256(hex!(
    "a3f18e4c6f0cdd0d8666f407610351cacb9a263678cf058294be9977b69f2cb3"
));

pub const ROOT_HASH: H256 = H256(hex!(
    "fd5f82b627a0b2c5ac0022a95422d435b204c4c1071d5dbda84ae8708d0110fd"
));

pub fn state_owner() -> StateOwner {
    StateOwner::try_from(hex!("050a0000001600009f7f36d0241d3e6a82254216d7de5780aa67d8f9").to_vec())
        .unwrap()
}

pub fn key() -> StateKey {
    StateKey::try_from(hex!("0000000000000000000000000003e7").to_vec()).unwrap()
}

pub fn value() -> StateValue {
    StateValue::try_from(hex!("0000000000000000000000000003e7").to_vec()).unwrap()
}

pub fn proof() -> StateProof<H256> {
    vec![
        StateProofNode::Left(H256(hex!(
            "19520b9dd118ede4c96c2f12718d43e22e9c0412b39cd15a36b40bce2121ddff"
        ))),
        StateProofNode::Left(H256(hex!(
            "29ac39fe8a6f05c0296b2f57769dae6a261e75a668c5b75bb96f43426e738a7d"
        ))),
        StateProofNode::Right(H256(hex!(
            "7e6f448ed8ceff132d032cc923dcd3f49fa7e702316a3db73e09b1ba2beea812"
        ))),
        StateProofNode::Left(H256(hex!(
            "47811eb10e0e7310f8e6c47b736de67b9b68f018d9dc7a224a5965a7fe90d405"
        ))),
        StateProofNode::Right(H256(hex!(
            "7646d25d9a992b6ebb996c2c4e5530ffc18f350747c12683ce90a1535305859c"
        ))),
        StateProofNode::Right(H256(hex!(
            "fe9181cc5392bc544a245964b1d39301c9ebd75c2128765710888ba4de9e61ea"
        ))),
        StateProofNode::Right(H256(hex!(
            "12f6db53d79912f90fd2a58ec4c30ebd078c490a6c5bd68c32087a3439ba111a"
        ))),
        StateProofNode::Right(H256(hex!(
            "efac0c32a7c7ab5ee5140850b5d7cbd6ebfaa406964a7e1c10239ccb816ea75e"
        ))),
        StateProofNode::Left(H256(hex!(
            "ceceb700876e9abc4848969882032d426e67b103dc96f55eeab84f773a7eeb5c"
        ))),
        StateProofNode::Left(H256(hex!(
            "abce2c418c92ca64a98baf9b20a3fcf7b5e9441e1166feedf4533b57c4bfa6a4"
        ))),
    ]
    .try_into()
    .unwrap()
}
