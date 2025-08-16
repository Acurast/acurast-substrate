use acurast_runtime_common::{
	constants::UNIT,
	types::{Balance, BlockNumber, ComputeRewardDistributor},
	weight,
};

use frame_support::{
	parameter_types,
	traits::{ConstU32, LockIdentifier, nonfungibles::{Create, InspectEnumerable as NFTInspectEnumerable}},
};
use sp_runtime::Perquintill;

use crate::{Balances, CommitterCollectionId, RootAccountId, Runtime, RuntimeEvent, Uniques};

use super::pallet_acurast_marketplace_config::ManagerProvider;
use super::pallet_acurast_processor_manager_config::AcurastManagerIdProvider;

parameter_types! {
	pub const Epoch: BlockNumber = 900; // 1.5 hours
	pub const EpochBase: BlockNumber = 0;
	pub const Era: BlockNumber = 16200; // 24 hours
	pub const WarmupPeriod: BlockNumber = 1800; // 3 hours, only for testing, we should use something like 2 weeks = 219027
	pub const MaxMetricCommitmentRatio: Perquintill = Perquintill::from_percent(80);
		pub const MinCooldownPeriod: BlockNumber = 432000; // ~1 month
	pub const MaxCooldownPeriod: BlockNumber = 20736000; // ~4 years
	pub const MinDelegation: Balance = 1 * UNIT;
	pub const MinStake: Balance = 10 * UNIT;
	pub const MaxDelegationRatio: Perquintill = Perquintill::from_percent(10);
	pub const ComputeStakingLockId: LockIdentifier = *b"compstak";
    pub const Decimals: Balance = UNIT;
}

impl pallet_acurast_compute::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type ManagerId = u128;
    type CommitterId = u128;
	type ManagerProvider = ManagerProvider;
	type ManagerIdProvider = AcurastManagerIdProvider;
    type CommitterIdProvider = AcurastCommitterIdProvider;
	type Epoch = Epoch;
	type EpochBase = EpochBase;
	type Era = Era;
	type MaxPools = ConstU32<30>;
	type MaxMetricCommitmentRatio = MaxMetricCommitmentRatio;
	type MinCooldownPeriod = MinCooldownPeriod;
	type MaxCooldownPeriod = MaxCooldownPeriod;
	type MinDelegation = MinDelegation;
	type MaxDelegationRatio = MaxDelegationRatio;
	type MinStake = MinStake;
	type WarmupPeriod = WarmupPeriod;
	type Balance = Balance;
	type BlockNumber = BlockNumber;
	type Currency = Balances;
    type Decimals = Decimals;
	type LockIdentifier = ComputeStakingLockId;
	type ComputeRewardDistributor = ComputeRewardDistributor<Runtime, (), Balances>;
	type WeightInfo = weight::pallet_acurast_compute::WeightInfo<Runtime>;
}

pub struct AcurastCommitterIdProvider;
impl
	pallet_acurast::CommitterIdProvider<
		<Runtime as frame_system::Config>::AccountId,
		<Runtime as pallet_acurast_compute::Config>::CommitterId,
	> for AcurastCommitterIdProvider
{
	fn create_committer_id(
		id: <Runtime as pallet_acurast_compute::Config>::CommitterId,
		owner: &<Runtime as frame_system::Config>::AccountId,
	) -> frame_support::pallet_prelude::DispatchResult {
		if Uniques::collection_owner(CommitterCollectionId::get()).is_none() {
			Uniques::create_collection(
				&CommitterCollectionId::get(),
				&RootAccountId::get(),
				&RootAccountId::get(),
			)?;
		}
		Uniques::do_mint(CommitterCollectionId::get(), id, owner.clone(), |_| Ok(()))
	}

	fn committer_id_for(
		owner: &<Runtime as frame_system::Config>::AccountId,
	) -> Result<
		<Runtime as pallet_acurast_compute::Config>::CommitterId,
		sp_runtime::DispatchError,
	> {
		Uniques::owned_in_collection(&CommitterCollectionId::get(), owner)
			.nth(0)
			.ok_or(frame_support::pallet_prelude::DispatchError::Other("Manager ID not found"))
	}

	fn owner_for(
		committer_id: <Runtime as pallet_acurast_compute::Config>::CommitterId,
	) -> Result<
		<Runtime as frame_system::Config>::AccountId,
		frame_support::pallet_prelude::DispatchError,
	> {
		Uniques::owner(CommitterCollectionId::get(), committer_id).ok_or(
			frame_support::pallet_prelude::DispatchError::Other(
				"Onwer for provided Manager ID not found",
			),
		)
	}
}