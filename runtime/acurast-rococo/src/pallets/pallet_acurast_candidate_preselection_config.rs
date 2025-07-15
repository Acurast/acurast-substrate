use crate::{Runtime, RuntimeEvent, Session};

impl pallet_acurast_candidate_preselection::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type ValidatorId = Self::AccountId;
	type ValidatorRegistration = Session;
	type WeightInfo = ();
}
