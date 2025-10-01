use acurast_runtime_common::{
	constants::{DAYS, UNIT},
	types::{AccountId, Balance, BlockNumber},
	weight,
};

use super::pallet_acurast_processor_manager_config::AcurastManagerIdProvider;
use frame_support::{
	parameter_types,
	traits::{
		nonfungibles::{Create, InspectEnumerable as NFTInspectEnumerable},
		tokens::imbalance::ResolveTo,
		ConstU32, LockIdentifier,
	},
};
use sp_runtime::Perquintill;

use pallet_acurast::ManagerProviderForEligibleProcessor;

use crate::{
	constants::{CommitmentCollectionId, ComputePalletId, RootAccountId},
	Acurast, AcurastProcessorManager, Balances, EnsureCouncilOrRoot, Runtime, RuntimeEvent,
	Treasury, Uniques,
};

parameter_types! {
	pub const Epoch: BlockNumber = 900; // 1.5 hours
	pub const BusyWeightBonus: Perquintill = Perquintill::from_percent(20);
	pub const MetricEpochValidity: BlockNumber = 16 * 2;
	pub const WarmupPeriod: BlockNumber = 1800; // 3 hours, only for testing, we should use something like 2 weeks = 219027
	pub const MaxMetricCommitmentRatio: Perquintill = Perquintill::from_percent(80);
	pub const MinCooldownPeriod: BlockNumber = 28 * DAYS;
	pub const MaxCooldownPeriod: BlockNumber = 48 * 28 * DAYS;
	pub const TargetCooldownPeriod: BlockNumber = 28 * DAYS; // same as MinCooldownPeriod
	pub const TargetStakedTokenSupply: Perquintill = Perquintill::from_percent(80);
	pub const MinDelegation: Balance = UNIT;
	pub const MinStake: Balance = 10 * UNIT;
	pub const MaxDelegationRatio: Perquintill = Perquintill::from_percent(90);
	pub const CooldownRewardRatio: Perquintill = Perquintill::from_percent(50);
	pub const RedelegationBlockingPeriod: BlockNumber = 112; // can redelegate once per 7*16=112 epochs ~= 1 week
	pub const ComputeStakingLockId: LockIdentifier = *b"compstak";
	pub const InflationPerEpoch: Balance = 0;//8_561_643_835_616_439; // ~ 5% a year for a total supply of 1B: ((1000000000 * 10^12 * 0.05) / 365 / 24) * 1.5
	pub const InflationStakedComputeRation: Perquintill = Perquintill::from_percent(70);
	pub const InflationMetricsRation: Perquintill = Perquintill::from_percent(10);
	pub TreasuryAccountId: AccountId = Treasury::account_id();
}

impl pallet_acurast_compute::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type PalletId = ComputePalletId;
	type ManagerId = u128;
	type CommitmentId = u128;
	type ManagerIdProvider = AcurastManagerIdProvider;
	type CommitmentIdProvider = AcurastCommitmentIdProvider;
	type Epoch = Epoch;
	type BusyWeightBonus = BusyWeightBonus;
	type MaxPools = ConstU32<30>;
	type MaxMetricCommitmentRatio = MaxMetricCommitmentRatio;
	type MinCooldownPeriod = MinCooldownPeriod;
	type MaxCooldownPeriod = MaxCooldownPeriod;
	type TargetCooldownPeriod = TargetCooldownPeriod;
	type TargetStakedTokenSupply = TargetStakedTokenSupply;
	type MinDelegation = MinDelegation;
	type MaxDelegationRatio = MaxDelegationRatio;
	type CooldownRewardRatio = CooldownRewardRatio;
	type RedelegationBlockingPeriod = RedelegationBlockingPeriod;
	type MinStake = MinStake;
	type MetricValidity = MetricEpochValidity;
	type WarmupPeriod = WarmupPeriod;
	type Currency = Balances;
	type LockIdentifier = ComputeStakingLockId;
	type ManagerProviderForEligibleProcessor = ManagerProviderForEligibleProcessor<
		Self::AccountId,
		Acurast,
		AcurastProcessorManager,
		AcurastProcessorManager,
	>;
	type InflationPerEpoch = InflationPerEpoch;
	type InflationStakedComputeRation = InflationStakedComputeRation;
	type InflationMetricsRation = InflationMetricsRation;
	type InflationHandler = ResolveTo<TreasuryAccountId, Balances>;
	type CreateModifyPoolOrigin = EnsureCouncilOrRoot;
	type OperatorOrigin = EnsureCouncilOrRoot;
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
			.ok_or(frame_support::pallet_prelude::DispatchError::Other("Commitment ID not found"))
	}

	fn owner_for(
		commitment_id: <Runtime as pallet_acurast_compute::Config>::CommitmentId,
	) -> Result<
		<Runtime as frame_system::Config>::AccountId,
		frame_support::pallet_prelude::DispatchError,
	> {
		Uniques::owner(CommitmentCollectionId::get(), commitment_id).ok_or(
			frame_support::pallet_prelude::DispatchError::Other(
				"Onwer for provided Commitment ID not found",
			),
		)
	}
}
