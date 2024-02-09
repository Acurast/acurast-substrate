#![allow(dead_code)]

use frame_support::{parameter_types, sp_runtime::AccountId32};

#[cfg(feature = "std")]
pub type UncheckedExtrinsic<T> = frame_system::mocking::MockUncheckedExtrinsic<T>;
pub type Balance = u128;
pub type AccountId = AccountId32;
// needs to be same as frame_system::mocking::MockBlock used in tests
pub type BlockNumber = u64;

pub const UNIT: Balance = 1_000_000;
pub const MILLIUNIT: Balance = UNIT / 1_000;
pub const MICROUNIT: Balance = UNIT / 1_000_000;

parameter_types! {
    pub const BlockHashCount: BlockNumber = 2400;
}

pub const fn alice_account_id() -> AccountId {
    AccountId32::new([0u8; 32])
}

pub const fn bob_account_id() -> AccountId {
    AccountId32::new([1u8; 32])
}

pub const fn charlie_account_id() -> AccountId {
    AccountId32::new([2u8; 32])
}

pub const fn dave_account_id() -> AccountId {
    AccountId32::new([3u8; 32])
}

pub const fn eve_account_id() -> AccountId {
    AccountId32::new([4u8; 32])
}
