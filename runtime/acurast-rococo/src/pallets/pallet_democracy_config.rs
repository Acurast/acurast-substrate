use acurast_runtime_common::{AccountId, UNIT};
use frame_support::traits::EitherOfDiverse;
use frame_system::{EnsureRoot, EnsureSigned, EnsureSignedBy, EnsureWithSuccess};
use sp_core::{ConstBool, ConstU128, ConstU32};

use crate::{
	Balances, CouncilAccountId, OriginCaller, Preimage, RootAccountId, Runtime, RuntimeEvent,
	Scheduler, TechCommitteeAccountId, DAYS, SUPPLY_FACTOR,
};

// The purpose of this offset is to ensure that a democratic proposal will not apply in the same
// block as a round change.
const ENACTMENT_OFFSET: u32 = 900;

impl pallet_democracy::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type EnactmentPeriod = ConstU32<{ 2 * DAYS + ENACTMENT_OFFSET }>;
	type LaunchPeriod = ConstU32<{ 7 * DAYS }>;
	type VotingPeriod = ConstU32<{ 14 * DAYS }>;
	type VoteLockingPeriod = ConstU32<{ 7 * DAYS }>;
	type FastTrackVotingPeriod = ConstU32<{ 1 * DAYS }>;
	type MinimumDeposit = ConstU128<{ 4 * UNIT * SUPPLY_FACTOR }>;

	type ExternalOrigin = EnsureSignedBy<CouncilAccountId, AccountId>;
	type ExternalMajorityOrigin = EnsureSignedBy<CouncilAccountId, AccountId>;
	type ExternalDefaultOrigin = EnsureSignedBy<CouncilAccountId, AccountId>;
	type SubmitOrigin = EnsureSigned<AccountId>;
	type FastTrackOrigin = EnsureSignedBy<TechCommitteeAccountId, AccountId>;
	type InstantOrigin = EnsureSignedBy<TechCommitteeAccountId, AccountId>;
	type CancellationOrigin =
		EitherOfDiverse<EnsureRoot<AccountId>, EnsureSignedBy<CouncilAccountId, AccountId>>;
	type BlacklistOrigin = EnsureRoot<AccountId>;
	type CancelProposalOrigin =
		EitherOfDiverse<EnsureRoot<AccountId>, EnsureSignedBy<TechCommitteeAccountId, AccountId>>;
	type VetoOrigin =
		EnsureWithSuccess<EnsureSignedBy<RootAccountId, AccountId>, AccountId, RootAccountId>;

	type CooloffPeriod = ConstU32<{ 7 * DAYS }>;
	type Slash = ();
	type InstantAllowed = ConstBool<true>;
	type Scheduler = Scheduler;
	type MaxVotes = ConstU32<100>;
	type PalletsOrigin = OriginCaller;
	type WeightInfo = pallet_democracy::weights::SubstrateWeight<Runtime>;
	type MaxProposals = ConstU32<100>;
	type Preimages = Preimage;
	type MaxDeposits = ConstU32<100>;
	type MaxBlacklisted = ConstU32<100>;
}
