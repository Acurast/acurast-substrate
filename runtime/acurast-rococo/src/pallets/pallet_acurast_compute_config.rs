use acurast_runtime_common::{
	constants::UNIT,
	types::{Balance, BlockNumber},
	weight,
};

use super::pallet_acurast_processor_manager_config::AcurastManagerIdProvider;
use frame_support::{
	parameter_types,
	traits::{
		nonfungibles::{Create, InspectEnumerable as NFTInspectEnumerable},
		ConstU32, LockIdentifier,
	},
	PalletId,
};

use sp_runtime::Perquintill;

use crate::{
	Acurast, AcurastProcessorManager, Balances, CommitmentCollectionId, RootAccountId, Runtime,
	RuntimeEvent, Uniques,
};
use pallet_acurast::ManagerProviderForEligibleProcessor;

parameter_types! {
	pub const Epoch: BlockNumber = 900; // 1.5 hours
	pub const Era: BlockNumber = 1; // 1.5 hours, only for testing, is normally 24 hours
	pub const MetricEpochValidity: BlockNumber = 16 * 90; // 3 months, will be changed to 24 hours in the future
	pub const WarmupPeriod: BlockNumber = 10; // 3 hours, only for testing, we should use something like 2 weeks = 219027
	pub const MaxMetricCommitmentRatio: Perquintill = Perquintill::from_percent(80);
	pub const MinCooldownPeriod: BlockNumber = 10; // 10 blocks (for testing purposes)
	pub const MaxCooldownPeriod: BlockNumber = 3600; // ~1 hour
	pub const MinDelegation: Balance = 1 * UNIT;
	pub const MaxDelegationRatio: Perquintill = Perquintill::from_percent(90);
	pub const CooldownRewardRatio: Perquintill = Perquintill::from_percent(50);
	pub const MinStake: Balance = 1 * UNIT;
	pub const ComputeStakingLockId: LockIdentifier = *b"compstak";
	pub const Decimals: Balance = UNIT;
	pub const ComputePalletId: PalletId = PalletId(*b"cmptepid");
	pub InflationPerDistribution: Perquintill = Perquintill::from_rational(835_451_506_486_784u128, 1_000_000_000_000_000_000u128);
	pub const InflationStakedBackedRation: Perquintill = Perquintill::from_percent(70);
}

impl pallet_acurast_compute::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type PalletId = ComputePalletId;
	type ManagerId = u128;
	type CommitmentId = u128;
	type ManagerIdProvider = AcurastManagerIdProvider;
	type CommitmentIdProvider = AcurastCommitmentIdProvider;
	type Epoch = Epoch;
	type Era = Era;
	type MaxPools = ConstU32<30>;
	type MaxMetricCommitmentRatio = MaxMetricCommitmentRatio;
	type MinCooldownPeriod = MinCooldownPeriod;
	type MaxCooldownPeriod = MaxCooldownPeriod;
	type MinDelegation = MinDelegation;
	type MaxDelegationRatio = MaxDelegationRatio;
	type CooldownRewardRatio = CooldownRewardRatio;
	type MinStake = MinStake;
	type MetricValidity = MetricEpochValidity;
	type WarmupPeriod = WarmupPeriod;
	type Currency = Balances;
	type Decimals = Decimals;
	type LockIdentifier = ComputeStakingLockId;
	type ManagerProviderForEligibleProcessor = ManagerProviderForEligibleProcessor<
		Self::AccountId,
		Acurast,
		AcurastProcessorManager,
		AcurastProcessorManager,
	>;
	type InflationPerDistribution = InflationPerDistribution;
	type InflationStakedBackedRation = InflationStakedBackedRation;
	type WeightInfo = weight::pallet_acurast_compute::WeightInfo<Runtime>;
}

pub struct AcurastCommitmentIdProvider;
impl
	pallet_acurast::CommitmentIdProvider<
		<Runtime as frame_system::Config>::AccountId,
		<Runtime as pallet_acurast_compute::Config>::CommitmentId,
	> for AcurastCommitmentIdProvider
{
	fn create_commitment_id(
		id: <Runtime as pallet_acurast_compute::Config>::CommitmentId,
		owner: &<Runtime as frame_system::Config>::AccountId,
	) -> frame_support::pallet_prelude::DispatchResult {
		if Uniques::collection_owner(CommitmentCollectionId::get()).is_none() {
			Uniques::create_collection(
				&CommitmentCollectionId::get(),
				&RootAccountId::get(),
				&RootAccountId::get(),
			)?;
		}
		Uniques::do_mint(CommitmentCollectionId::get(), id, owner.clone(), |_| Ok(()))
	}

	fn commitment_id_for(
		owner: &<Runtime as frame_system::Config>::AccountId,
	) -> Result<<Runtime as pallet_acurast_compute::Config>::CommitmentId, sp_runtime::DispatchError>
	{
		Uniques::owned_in_collection(&CommitmentCollectionId::get(), owner)
			.nth(0)
			.ok_or(frame_support::pallet_prelude::DispatchError::Other("Manager ID not found"))
	}

	fn owner_for(
		commitment_id: <Runtime as pallet_acurast_compute::Config>::CommitmentId,
	) -> Result<
		<Runtime as frame_system::Config>::AccountId,
		frame_support::pallet_prelude::DispatchError,
	> {
		Uniques::owner(CommitmentCollectionId::get(), commitment_id).ok_or(
			frame_support::pallet_prelude::DispatchError::Other(
				"Onwer for provided Manager ID not found",
			),
		)
	}
}
