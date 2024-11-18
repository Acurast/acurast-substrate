use acurast_runtime_common::types::{AccountId, Balance};
use frame_support::traits::{fungible::HoldConsideration, LinearStoragePrice};

use crate::{
	Balances, EnsureAdminOrRoot, PreimageBaseDeposit, PreimageByteDeposit, PreimageHoldReason,
	Runtime, RuntimeEvent,
};

/// Runtime configuration for pallet_preimage.
impl pallet_preimage::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = pallet_preimage::weights::SubstrateWeight<Runtime>;
	type Currency = Balances;
	type ManagerOrigin = EnsureAdminOrRoot;
	type Consideration = HoldConsideration<
		AccountId,
		Balances,
		PreimageHoldReason,
		LinearStoragePrice<PreimageBaseDeposit, PreimageByteDeposit, Balance>,
	>;
}
