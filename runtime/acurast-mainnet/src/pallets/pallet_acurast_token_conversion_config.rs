use frame_support::{parameter_types, traits::tokens::imbalance::ResolveTo, PalletId};
use frame_system::EnsureRoot;

use acurast_runtime_common::{
	constants::{MainnetTokenConversionPalletId, DAYS},
	types::{AccountId, BlockNumber},
};

use crate::{Balances, Runtime, RuntimeHoldReason, Treasury};

parameter_types! {
	pub const TokenConversionPalletId: PalletId = MainnetTokenConversionPalletId::get();
	pub const MinLockDuration: BlockNumber = 3 * 28 * DAYS;
	pub const MaxLockDuration: BlockNumber = 48 * 28 * DAYS;
	pub TreasuryAccountId: AccountId = Treasury::account_id();
}

impl pallet_acurast_token_conversion::Config for Runtime {
	type PalletId = TokenConversionPalletId;
	type Currency = Balances;
	type RuntimeHoldReason = RuntimeHoldReason;
	type MinLockDuration = MinLockDuration;
	type MaxLockDuration = MaxLockDuration;
	type OnSlash = ResolveTo<TreasuryAccountId, Balances>;
	type EnableOrigin = EnsureRoot<Self::AccountId>;
	type WeightInfo = crate::weights::pallet_acurast_token_conversion::WeightInfo<Self>;
}
