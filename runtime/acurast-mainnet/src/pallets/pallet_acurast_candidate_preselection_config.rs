use crate::{EnsureAdminOrRoot, Runtime, RuntimeEvent, Session};

impl pallet_acurast_candidate_preselection::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type ValidatorId = Self::AccountId;
	type ValidatorRegistration = Session;
	type UpdateOrigin = EnsureAdminOrRoot;
	type WeightInfo =
		acurast_runtime_common::weight::pallet_acurast_candidate_preselection::WeightInfo<Self>;
}
