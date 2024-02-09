#![allow(dead_code)]

use crate::*;
use codec::alloc::string::String;
use pallet_acurast_marketplace::PubKey;
use sp_runtime::AccountId32;

#[cfg(feature = "std")]
pub type UncheckedExtrinsic<T> = frame_system::mocking::MockUncheckedExtrinsic<T>;
#[cfg(feature = "std")]
pub type Block<T> = frame_system::mocking::MockBlock<T>;
pub type AccountId = AccountId32;
pub type BlockNumber = u64;

pub fn alice_account_id() -> AccountId {
    [0; 32].into()
}

pub fn tezos_account_id() -> String {
    "tz1h4EsGunH2Ue1T2uNs8mfKZ8XZoQji3HcK".into()
}

pub fn p256_public_key() -> PubKey {
    PubKey::SECP256r1([0u8; 33].to_vec().try_into().unwrap())
}

pub fn message(id: u128) -> Message {
    Message {
        id: id as u64,
        action: Action::AssignJob(id, p256_public_key()),
    }
}

pub fn action(id: u128) -> Action {
    Action::AssignJob(id, p256_public_key())
}
