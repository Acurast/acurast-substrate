use acurast_runtime_common::types::InvulnerableCollators;

use crate::{EnsureCouncilOrRoot, Runtime, Session};

impl pallet_acurast_candidate_preselection::Config for Runtime {
	type ValidatorId = Self::AccountId;
	type ValidatorRegistration = Session;
	type ExemptValidators = InvulnerableCollators<Self>;
	type UpdateOrigin = EnsureCouncilOrRoot;
	type WeightInfo =
		acurast_runtime_common::weight::pallet_acurast_candidate_preselection::WeightInfo<Self>;
}
