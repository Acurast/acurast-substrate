use frame_support::{parameter_types, traits::tokens::imbalance::ResolveTo, PalletId};
use frame_system::EnsureRoot;
use sp_runtime::traits::AccountIdConversion;

use acurast_runtime_common::{
	constants::{CanaryTokenConversionPalletId, MINUTES},
	types::{AccountId, BlockNumber},
};

use crate::{Balances, Runtime, RuntimeHoldReason};

parameter_types! {
	pub const TokenConversionPalletId: PalletId = CanaryTokenConversionPalletId::get();
	pub TokenConversionPalletAccountId: AccountId = TokenConversionPalletId::get().into_account_truncating();
	pub const MinLockDuration: BlockNumber = MINUTES;
	pub const MaxLockDuration: BlockNumber = 5 * MINUTES;
}

impl pallet_acurast_token_conversion::Config for Runtime {
	type PalletId = TokenConversionPalletId;
	type Currency = Balances;
	type RuntimeHoldReason = RuntimeHoldReason;
	type MinLockDuration = MinLockDuration;
	type MaxLockDuration = MaxLockDuration;
	type OnSlash = ResolveTo<TokenConversionPalletAccountId, Balances>;
	type EnableOrigin = EnsureRoot<Self::AccountId>;
	type WeightInfo = pallet_acurast_token_conversion::weights::WeightInfo<Self>;
}
