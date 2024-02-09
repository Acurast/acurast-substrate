#![allow(dead_code)]

use frame_support::{
    parameter_types,
    sp_runtime::{traits::AccountIdConversion, AccountId32},
    PalletId,
};
#[cfg(feature = "std")]
pub type UncheckedExtrinsic<T> = frame_system::mocking::MockUncheckedExtrinsic<T>;
#[cfg(feature = "std")]
pub type Block<T> = frame_system::mocking::MockBlock<T>;
pub type AssetId = u128;
pub type Balance = u128;
pub type AccountId = AccountId32;
pub type BlockNumber = u32;

pub const SEED: u32 = 1337;
pub const EXISTENTIAL_DEPOSIT: Balance = MILLIUNIT;
pub const UNIT: Balance = 1_000_000;
pub const MILLIUNIT: Balance = UNIT / 1_000;
pub const MICROUNIT: Balance = UNIT / 1_000_000;
pub const INITIAL_BALANCE: u128 = UNIT * 100;

parameter_types! {
    pub const BlockHashCount: BlockNumber = 2400;
    pub const RootAccountId: AccountId = alice_account_id();
}
parameter_types! {
    pub const MinimumPeriod: u64 = 2000;
    pub const ExistentialDeposit: Balance = EXISTENTIAL_DEPOSIT;
}
parameter_types! {
    pub const MaxReserves: u32 = 50;
    pub const MaxLocks: u32 = 50;
    pub const AcurastPalletId: PalletId = PalletId(*b"acrstpid");
    pub const ReportTolerance: u64 = 12000;
}

pub fn pallet_assets_account() -> AccountId {
    AcurastPalletId::get().into_account_truncating()
}

pub const fn alice_account_id() -> AccountId {
    AccountId32::new([0u8; 32])
}

pub const fn bob_account_id() -> AccountId {
    AccountId32::new([1u8; 32])
}
