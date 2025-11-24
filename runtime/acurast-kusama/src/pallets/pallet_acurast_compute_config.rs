use frame_support::{
	parameter_types,
	traits::{
		nonfungibles::{Create, InspectEnumerable as NFTInspectEnumerable},
		tokens::imbalance::ResolveTo,
		LockIdentifier,
	},
	PalletId,
};

use acurast_runtime_common::{
	constants::{DAYS, HOURS, UNIT},
	types::{AccountId, Balance, BlockNumber},
	weight,
};
use pallet_acurast::ManagerProviderForEligibleProcessor;
use pallet_acurast_compute::BlockAuthorProvider;
use sp_core::ConstU32;
use sp_runtime::{FixedU128, Perbill, Perquintill};

use crate::{
	constants::CommitmentCollectionId,
	pallets::pallet_acurast_processor_manager_config::AcurastManagerIdProvider, Acurast,
	AcurastProcessorManager, Authorship, Balances, EnsureCouncilOrRoot, RootAccountId, Runtime,
	RuntimeEvent, Treasury, Uniques,
};

parameter_types! {
	pub const Epoch: BlockNumber = 900; // 1.5 hours
	pub const BusyWeightBonus: Perquintill = Perquintill::from_percent(10);
	pub const MetricEpochValidity: BlockNumber = 16 * 2;
	pub const WarmupPeriod: BlockNumber = 1800; // 3 hours, only for testing, we should use something like 2 weeks = 219027
	pub const MaxMetricCommitmentRatio: Perquintill = Perquintill::from_percent(80);
	pub const MinCooldownPeriod: BlockNumber = HOURS;
	pub const MaxCooldownPeriod: BlockNumber = 48 * HOURS;
	pub const TargetWeightPerComputeMultiplier: FixedU128 = FixedU128::from_u32(5); // 5.0 = 500%
	pub const TargetStakedTokenSupply: Perquintill = Perquintill::from_percent(80);
	pub const MinDelegation: Balance = UNIT;
	pub const MinStake: Balance = 10 * UNIT;
	pub const BaseSlashRation: Perquintill = Perquintill::from_parts(34246575340000); // 0.003424657534% of total stake per missed epoch
	pub const SlashRewardRatio: Perquintill = Perquintill::from_percent(10); // 10% of slash goes to caller
	pub const MaxCommissionIncreasePerDay: Perbill =Perbill::from_parts(2500000); // 0.25% per day
	pub const BlocksPerDay: BlockNumber = DAYS;
	pub const MaxDelegationRatio: Perquintill = Perquintill::from_percent(90);
	pub const CooldownRewardRatio: Perquintill = Perquintill::from_percent(50);
	pub const RedelegationBlockingPeriod: BlockNumber = 16; // can redelegate once per 16 epochs ~= 1 day
	pub const ComputeStakingLockId: LockIdentifier = *b"compstak";
	pub const ComputePalletId: PalletId = PalletId(*b"cmptepid");
	pub const InflationPerEpoch: Balance = 8_561_643_835_616_439; // ~ 5% a year for a total supply of 1B
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
