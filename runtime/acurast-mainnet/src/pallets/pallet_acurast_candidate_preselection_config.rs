use frame_system::EnsureSignedBy;

use crate::{Admin, Runtime, RuntimeEvent, Session};

impl pallet_acurast_candidate_preselection::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type ValidatorId = Self::AccountId;
	type ValidatorRegistration = Session;
	type UpdateOrigin = EnsureSignedBy<Admin, Self::AccountId>;
	type WeightInfo =
		acurast_runtime_common::weight::pallet_acurast_candidate_preselection::WeightInfo<Self>;
}
