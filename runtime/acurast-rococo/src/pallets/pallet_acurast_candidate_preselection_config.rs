use frame_system::EnsureRoot;

use crate::{Runtime, RuntimeEvent, Session};

impl pallet_acurast_candidate_preselection::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type ValidatorId = Self::AccountId;
	type ValidatorRegistration = Session;
	type UpdateOrigin = EnsureRoot<Self::AccountId>;
	type WeightInfo =
		acurast_runtime_common::weight::pallet_acurast_candidate_preselection::WeightInfo<Self>;
}
