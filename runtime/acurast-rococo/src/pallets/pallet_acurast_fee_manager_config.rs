use acurast_runtime_common::{types::AccountId, weight};
use frame_system::EnsureRoot;

use crate::{DefaultFeePercentage, DefaultMatcherFeePercentage, Runtime, RuntimeEvent};

/// Runtime configuration for pallet_acurast_fee_manager instance 1.
impl pallet_acurast_fee_manager::Config<pallet_acurast_fee_manager::Instance1> for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type DefaultFeePercentage = DefaultFeePercentage;
	type UpdateOrigin = EnsureRoot<AccountId>;
	type WeightInfo = weight::pallet_acurast_fee_manager::WeightInfo<Self>;
}

/// Runtime configuration for pallet_acurast_fee_manager instance 2.
impl pallet_acurast_fee_manager::Config<pallet_acurast_fee_manager::Instance2> for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type DefaultFeePercentage = DefaultMatcherFeePercentage;
	type UpdateOrigin = EnsureRoot<AccountId>;
	type WeightInfo = weight::pallet_acurast_fee_manager::WeightInfo<Self>;
}
