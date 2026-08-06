use frame_support::{parameter_types, PalletId};

use acurast_runtime_common::{
	constants::{CanaryTokenConversionPalletId, DAYS},
	types::BlockNumber,
};

use crate::{Balances, EnsureCouncilOrRoot, Runtime, RuntimeHoldReason};

parameter_types! {
	pub const TokenConversionPalletId: PalletId = CanaryTokenConversionPalletId::get();
	pub const MinLockDuration: BlockNumber = 3 * 28 * DAYS;
	pub const MaxLockDuration: BlockNumber = 48 * 28 * DAYS;
}

impl pallet_acurast_token_conversion::Config for Runtime {
	type PalletId = TokenConversionPalletId;
	type Currency = Balances;
	type RuntimeHoldReason = RuntimeHoldReason;
	type MinLockDuration = MinLockDuration;
	type MaxLockDuration = MaxLockDuration;
	type OnSlash = ();
	type EnableOrigin = EnsureCouncilOrRoot;
	type WeightInfo = crate::weights::pallet_acurast_token_conversion::WeightInfo<Self>;
}
