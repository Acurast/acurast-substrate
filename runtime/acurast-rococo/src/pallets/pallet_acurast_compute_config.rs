use acurast_runtime_common::{
	constants::{DAYS, MINUTES, UNIT},
	types::{AccountId, Balance, BlockNumber},
	weight,
};
use pallet_acurast_compute::BlockAuthorProvider;
use polkadot_runtime_common::prod_or_fast;

use super::pallet_acurast_processor_manager_config::AcurastManagerIdProvider;
use frame_support::{
	parameter_types,
	traits::{
		nonfungibles::{Create, InspectEnumerable as NFTInspectEnumerable},
		tokens::imbalance::ResolveTo,
		ConstU32, LockIdentifier,
	},
	PalletId,
};

use sp_runtime::{FixedU128, Perbill, Perquintill};

use crate::{
	Acurast, AcurastProcessorManager, Authorship, Balances, CommitmentCollectionId,
	EnsureCouncilOrRoot, RootAccountId, Runtime, RuntimeEvent, Treasury, Uniques,
};
use pallet_acurast::ManagerProviderForEligibleProcessor;

parameter_types! {
	pub const Epoch: BlockNumber = prod_or_fast!(35 * MINUTES, 5);
	pub const BusyWeightBonus: Perquintill = Perquintill::from_percent(20);
	pub const MetricEpochValidity: BlockNumber = 16; // 24 hours
	pub const WarmupPeriod: BlockNumber = prod_or_fast!(1800, 10); // 3 hours
	pub const MaxMetricCommitmentRatio: Perquintill = Perquintill::from_percent(80);
	pub const MinCooldownPeriod: BlockNumber = 10; // 10 blocks (for testing purposes)
	pub const MaxCooldownPeriod: BlockNumber = 3600; // ~1 hour
	pub const TargetWeightPerComputeMultiplier: FixedU128 = FixedU128::from_u32(5); // 5.0 = 500%
	pub const TargetStakedTokenSupply: Perquintill = Perquintill::from_percent(80);
	pub const MinDelegation: Balance = UNIT;
	pub const MaxDelegationRatio: Perquintill = Perquintill::from_percent(90);
	pub const CooldownRewardRatio: Perquintill = Perquintill::from_percent(50);
	pub const RedelegationBlockingPeriod: BlockNumber = 16; // can redelegate once per 16 epochs ~= 1 day
	pub const MinStake: Balance = UNIT;
	pub const BaseSlashRation: Perquintill = Perquintill::from_percent(1); // 1% of total stake per missed epoch
	pub const SlashRewardRatio: Perquintill = Perquintill::from_percent(10); // 10% of slash goes to caller
	pub const MaxCommissionIncreasePerDay: Perbill =Perbill::from_parts(2500000); // 0.25% per day
	pub const BlocksPerDay: BlockNumber = DAYS;
	pub const ComputeStakingLockId: LockIdentifier = *b"compstak";
	pub const ComputePalletId: PalletId = PalletId(*b"cmptepid");
	pub const InflationPerEpoch: Balance = 8_561_643_835_616_438; // ~ 5% a year for a total supply of 1B
	pub const InflationStakedComputeRatio: Perquintill = Perquintill::from_percent(70);
	pub const InflationMetricsRatio: Perquintill = Perquintill::from_percent(10);
	pub const InflationCollatorsRatio: Perquintill = Perquintill::from_percent(5);
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
	type TargetWeightPerComputeMultiplier = TargetWeightPerComputeMultiplier;
	type TargetStakedTokenSupply = TargetStakedTokenSupply;
	type MinDelegation = MinDelegation;
	type MaxDelegationRatio = MaxDelegationRatio;
	type CooldownRewardRatio = CooldownRewardRatio;
	type RedelegationBlockingPeriod = RedelegationBlockingPeriod;
	type MinStake = MinStake;
	type BaseSlashRation = BaseSlashRation;
	type SlashRewardRatio = SlashRewardRatio;
	type MaxCommissionIncreasePerDay = MaxCommissionIncreasePerDay;
	type BlocksPerDay = BlocksPerDay;
	type MetricValidity = MetricEpochValidity;
	type WarmupPeriod = WarmupPeriod;
	type Currency = Balances;
	type LockIdentifier = ComputeStakingLockId;
	type ManagerProviderForEligibleProcessor = ManagerProviderForEligibleProcessor<
		Self::AccountId,
		Self::ManagerId,
		Acurast,
		AcurastProcessorManager,
		AcurastProcessorManager,
	>;
	type InflationPerEpoch = InflationPerEpoch;
	type InflationStakedComputeRatio = InflationStakedComputeRatio;
	type InflationMetricsRatio = InflationMetricsRatio;
	type InflationCollatorsRatio = InflationCollatorsRatio;
	type InflationHandler = ResolveTo<TreasuryAccountId, Balances>;
	type CreateModifyPoolOrigin = EnsureCouncilOrRoot;
	type OperatorOrigin = EnsureCouncilOrRoot;
	type AuthorProvider = AuthorProvider;
	type WeightInfo = weight::pallet_acurast_compute::WeightInfo<Runtime>;
}

pub struct AuthorProvider;
impl BlockAuthorProvider<<Runtime as frame_system::Config>::AccountId> for AuthorProvider {
	fn author() -> Option<<Runtime as frame_system::Config>::AccountId> {
		Authorship::author()
	}
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
