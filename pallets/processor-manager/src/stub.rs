#![allow(dead_code)]

use codec::Encode;
use frame_support::{
    parameter_types,
    sp_runtime::{AccountId32, MultiSignature},
    weights::Weight,
    PalletId,
};
use hex_literal::hex;
use sp_core::sr25519;
#[cfg(feature = "std")]
use sp_core::Pair;
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
    pub BlockWeights: frame_system::limits::BlockWeights = frame_system::limits::BlockWeights::simple_max(Weight::from_parts(1024, 0));
    pub const MinimumPeriod: u64 = 2000;
    pub const ExistentialDeposit: Balance = EXISTENTIAL_DEPOSIT;
}
parameter_types! {
    pub const MaxReserves: u32 = 50;
    pub const MaxLocks: u32 = 50;
    pub const AcurastPalletId: PalletId = PalletId(*b"acrstpid");
    pub const ReportTolerance: u64 = 12000;
}

pub fn processor_account_id() -> AccountId {
    hex!("b8bc25a2b4c0386b8892b43e435b71fe11fa50533935f027949caf04bcce4694").into()
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

pub fn generate_account(index: u32) -> AccountId {
    let mut buffer = [0u8; 32];
    let byte1: u8 = (index >> 24) as u8;
    let byte2: u8 = ((index << 8) >> 24) as u8;
    let byte3: u8 = ((index << 16) >> 24) as u8;
    let byte4: u8 = ((index << 24) >> 24) as u8;
    buffer[28] = byte1;
    buffer[29] = byte2;
    buffer[30] = byte3;
    buffer[31] = byte4;

    let account_id: AccountId = buffer.into();

    account_id
}

#[cfg(feature = "std")]
pub fn generate_pair_account() -> (sr25519::Pair, AccountId) {
    let (pair, _) = sr25519::Pair::generate();
    let account_id: AccountId = pair.public().into();

    (pair, account_id)
}

#[cfg(feature = "std")]
pub fn generate_signature(
    signer: &sr25519::Pair,
    account: &AccountId,
    timestamp: u128,
    counter: u64,
) -> MultiSignature {
    let message = [
        b"<Bytes>".to_vec(),
        account.encode(),
        timestamp.encode(),
        counter.encode(),
        b"</Bytes>".to_vec(),
    ]
    .concat();
    signer.sign(&message).into()
}
