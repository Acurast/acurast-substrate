use frame_system::EnsureRoot;

use acurast_runtime_common::types::AccountId;

use crate::{
	AcurastCandidatePreselection, Balances, MaxCandidates, MaxInvulnerables, MinCandidates, Period,
	PotId, Runtime, RuntimeEvent,
};

/// Runtime configuration for pallet_collator_selection.
impl pallet_collator_selection::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type UpdateOrigin = EnsureRoot<AccountId>;
	type PotId = PotId;
	type MaxCandidates = MaxCandidates;
	type MinEligibleCollators = MinCandidates;
	type MaxInvulnerables = MaxInvulnerables;
	// should be a multiple of session or things will get inconsistent
	type KickThreshold = Period;
	type ValidatorId = <Self as frame_system::Config>::AccountId;
	type ValidatorIdOf = pallet_collator_selection::IdentityCollator;
	type ValidatorRegistration = AcurastCandidatePreselection;
	type WeightInfo = ();
}
