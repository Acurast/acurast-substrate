#![allow(unused)]

use frame_support::sp_runtime::AccountId32;

pub type AccountId = AccountId32;

pub fn alice_account_id() -> AccountId {
    [0; 32].into()
}

pub fn bob_account_id() -> AccountId {
    [1; 32].into()
}
