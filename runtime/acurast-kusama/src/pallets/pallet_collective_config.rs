use frame_system::EnsureRoot;
use pallet_collective::{MemberCount, MoreThanMajorityThenPrimeDefaultVote, ProposalIndex};
use sp_core::parameter_types;
use sp_runtime::{Perbill, Weight};

use acurast_runtime_common::{
	constants::DAYS,
	types::{AccountId, BlockNumber, CouncilInstance, CouncilThreeSeventh},
};

use crate::{
	Council, EnsureCouncilOrRoot, Runtime, RuntimeBlockWeights, RuntimeCall, RuntimeEvent,
	RuntimeOrigin,
};

type CouncilMembershipInstance = pallet_membership::Instance1;

parameter_types! {
	pub const MotionDuration: BlockNumber = 3 * DAYS;
	pub const MaxProposals: ProposalIndex = 10;
	pub const MaxMembers: MemberCount = 10;
	pub MaxCouncilProposalWeight: Weight = Perbill::from_percent(50) * RuntimeBlockWeights::get().max_block;
}

impl pallet_collective::Config<CouncilInstance> for Runtime {
	type RuntimeOrigin = RuntimeOrigin;
	type Proposal = RuntimeCall;
	type RuntimeEvent = RuntimeEvent;
	type MotionDuration = MotionDuration;
	type MaxProposals = MaxProposals;
	type MaxMembers = MaxMembers;
	type DefaultVote = MoreThanMajorityThenPrimeDefaultVote;
	type SetMembersOrigin = EnsureRoot<AccountId>;
	type DisapproveOrigin = CouncilThreeSeventh;
	type KillOrigin = CouncilThreeSeventh;
	type Consideration = ();
	type MaxProposalWeight = MaxCouncilProposalWeight;
	type WeightInfo = pallet_collective::weights::SubstrateWeight<Self>;
}

impl pallet_membership::Config<CouncilMembershipInstance> for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type AddOrigin = EnsureCouncilOrRoot;
	type RemoveOrigin = EnsureCouncilOrRoot;
	type SwapOrigin = EnsureCouncilOrRoot;
	type ResetOrigin = EnsureCouncilOrRoot;
	type PrimeOrigin = EnsureCouncilOrRoot;
	type MembershipInitialized = Council;
	type MembershipChanged = Council;
	type MaxMembers = MaxMembers;
	type WeightInfo = pallet_membership::weights::SubstrateWeight<Self>;
}
