#![cfg_attr(not(feature = "std"), no_std)]
#![allow(deprecated)]

pub use datastructures::*;
use frame_system::pallet_prelude::BlockNumberFor;
pub use pallet::*;
pub use traits::*;
pub use types::*;

mod datastructures;
mod hooks;
mod migration;
mod staking;
mod traits;
mod types;

#[cfg(test)]
pub mod mock;
#[cfg(any(test, feature = "runtime-benchmarks"))]
mod stub;
#[cfg(test)]
mod tests;

pub mod weights;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

const LOG_TARGET: &str = "runtime::acurast_compute";

#[frame_support::pallet]
pub mod pallet {
	use acurast_common::{
		CommitmentIdProvider, ManagerIdProvider, ManagerLookup, MetricInput, PoolId,
	};
	use frame_support::{
		dispatch::DispatchResultWithPostInfo,
		pallet_prelude::*,
		traits::{
			fungible::{Balanced, Credit},
			tokens::{Fortitude, Precision, Preservation},
			Currency, EnsureOrigin, Get, Imbalance, InspectLockableCurrency, LockIdentifier,
			OnUnbalanced,
		},
		PalletId, Parameter,
	};
	use frame_system::pallet_prelude::*;
	use frame_system::pallet_prelude::{BlockNumberFor, OriginFor};
	use sp_core::U256;
	use sp_runtime::{
		traits::{AccountIdConversion, One, SaturatedConversion, Saturating, Zero},
		FixedPointNumber, FixedU128, Perbill, Perquintill,
	};
	use sp_std::cmp::max;
	use sp_std::prelude::*;

	use crate::*;

	/// Configure the pallet by specifying the parameters and types on which it depends.
	#[pallet::config]
	pub trait Config<I: 'static = ()>: frame_system::Config {
		/// Because this pallet emits events, it depends on the runtime's definition of an event.
		type RuntimeEvent: From<Event<Self, I>>
			+ IsType<<Self as frame_system::Config>::RuntimeEvent>;
		type PalletId: Get<PalletId>;
		type ManagerId: Member
			+ Parameter
			+ MaxEncodedLen
			+ MaybeSerializeDeserialize
			+ Copy
			+ CheckedAdd
			+ From<u128>;
		type CommitmentId: Member
			+ Parameter
			+ MaxEncodedLen
			+ MaybeSerializeDeserialize
			+ Copy
			+ CheckedAdd
			+ Default
			+ From<u128>;
		type ManagerIdProvider: ManagerIdProvider<Self::AccountId, Self::ManagerId>;
		type CommitmentIdProvider: CommitmentIdProvider<Self::AccountId, Self::CommitmentId>;
		/// Defines the duration of an epoch.
		///
		/// This is currently the important cycle on which the compute reward system operates.
		///
		/// An epoch is used for the duration of all of the aligned
		///
		/// - commit-reward cycle length for active processor
		/// - the unstaked compute rewards payout cycle
		/// - the stake-backed compute rewards
		///
		/// Must be longer than a heartbeat interval and better include multiple heartbeats so there is a chance of recovery if one heartbeat is missed.
		/// Does technically not need to be a multiple of the heartbeat interval but it's more reasonable to choose so for simplicity.
		#[pallet::constant]
		type Epoch: Get<EpochOf<Self>>;
		/// The bonus busy devices get in weight. A bonus of `20%` means the weight will be `120%` of the idle weight.
		#[pallet::constant]
		type BusyWeightBonus: Get<Perquintill>;
		/// How many epochs a metric is valid for.
		#[pallet::constant]
		type MetricValidity: Get<EpochOf<Self>>;
		#[pallet::constant]
		type MaxPools: Get<u32>;
		/// The maximum ratio `committed_metric / last_era_metric_averate`. This ratio limit is enforced separately by metric pool
		#[pallet::constant]
		type MaxMetricCommitmentRatio: Get<Perquintill>;
		/// The minimum cooldown period for delegators in number of blocks.
		#[pallet::constant]
		type MinCooldownPeriod: Get<BlockNumberFor<Self>>;
		/// The maximum cooldown period for delegators in number of blocks. Delegator's weight is linear as [`Stake`]`::cooldown_period / MaxCooldownPeriod`.
		#[pallet::constant]
		type MaxCooldownPeriod: Get<BlockNumberFor<Self>>;
		/// The target cooldown period for delegators in number of blocks, used as reference for economic calculations.
		#[pallet::constant]
		type TargetCooldownPeriod: Get<BlockNumberFor<Self>>;
		/// The target ratio of total token supply that should be staked, used for adjusting incentives.
		#[pallet::constant]
		type TargetStakedTokenSupply: Get<Perquintill>;
		/// The minimum possible delegated amount towards a commitment. There is no maximum for the amount which a delegator can offer, but it's still limited by the [`Self::MaxDelegationRatio`].
		#[pallet::constant]
		type MinDelegation: Get<BalanceFor<Self, I>>;
		/// The maximum ratio `delegated_stake / commitment_total_stake = delegated_stake / (delegated_stake + committer_stake)`.
		#[pallet::constant]
		type MaxDelegationRatio: Get<Perquintill>;
		#[pallet::constant]
		type CooldownRewardRatio: Get<Perquintill>;
		/// The period a delegator is blocked after redelegation. Applies only if the current committer is not in cooldown.
		#[pallet::constant]
		type RedelegationBlockingPeriod: Get<EpochOf<Self>>;
		/// The minimum stake by a committer.
		#[pallet::constant]
		type MinStake: Get<BalanceFor<Self, I>>;
		/// The slash ratio applied to total commitment stake (committer + delegations) when a commitment fails to deliver committed metrics.
		#[pallet::constant]
		type BaseSlashRation: Get<Perquintill>;
		/// The ratio of slashed amount that is rewarded to the caller who triggers the slash extrinsic.
		#[pallet::constant]
		type SlashRewardRatio: Get<Perquintill>;
		/// How long a processor needs to warm up before his metrics are respected for compute score and reward calculation.
		#[pallet::constant]
		type WarmupPeriod: Get<BlockNumberFor<Self>>;
		type Currency: InspectLockableCurrency<Self::AccountId, Moment = BlockNumberFor<Self>>
			+ Balanced<Self::AccountId, Balance = BalanceFor<Self, I>>;
		/// The single lock indentifier used for the sum of all staked and delegated amounts.
		///
		/// We have to use the same lock identifier since we do not want the locks to overlap;
		/// Eventhough a staker can also delegate at the same time, the same funds contributing to total balance of an account is either delegated or staked, not both.
		#[pallet::constant]
		type LockIdentifier: Get<LockIdentifier>;
		type ManagerProviderForEligibleProcessor: ManagerLookup<
			AccountId = Self::AccountId,
			ManagerId = Self::ManagerId,
		>;
		#[pallet::constant]
		type InflationPerEpoch: Get<BalanceFor<Self, I>>;
		#[pallet::constant]
		type InflationStakedComputeRation: Get<Perquintill>;
		#[pallet::constant]
		type InflationMetricsRation: Get<Perquintill>;
		/// Handler of remaining inflation.
		type InflationHandler: OnUnbalanced<Credit<Self::AccountId, Self::Currency>>;
		/// Origin that can create and modify pools
		type CreateModifyPoolOrigin: EnsureOrigin<Self::RuntimeOrigin>;
		/// Origin that can execute operational extrinsics
		type OperatorOrigin: EnsureOrigin<Self::RuntimeOrigin>;
		/// Weight Info for extrinsics.
		type WeightInfo: WeightInfo;
	}

	#[pallet::genesis_config]
	pub struct GenesisConfig<T: Config<I>, I: 'static = ()> {
		pub pools: Vec<(MetricPoolName, Perquintill, MetricPoolConfigValues)>,
		phantom: PhantomData<(T, I)>,
	}

	impl<T: Config<I>, I: 'static> Default for GenesisConfig<T, I> {
		fn default() -> Self {
			Self { pools: Default::default(), phantom: Default::default() }
		}
	}

	#[pallet::genesis_build]
	impl<T: Config<I>, I: 'static> BuildGenesisConfig for GenesisConfig<T, I>
	where
		BalanceFor<T, I>: From<u128>,
	{
		fn build(&self) {
			for (name, reward_ratio, config) in self.pools.clone() {
				if let Err(e) = Pallet::<T, I>::do_create_pool(name, reward_ratio, config) {
					log::error!(
						target: LOG_TARGET,
						"AcurastCompute Genesis error: {:?}",
						e,
					);
				}
			}
		}
	}

	/// Ever increasing number of all pools created so far.
	#[pallet::storage]
	#[pallet::getter(fn last_metric_pool_id)]
	pub type LastMetricPoolId<T: Config<I>, I: 'static = ()> = StorageValue<_, PoolId, ValueQuery>;

	/// Individual processors' epoch start and active status.
	#[pallet::storage]
	#[pallet::getter(fn processors)]
	pub type Processors<T: Config<I>, I: 'static = ()> =
		StorageMap<_, Twox64Concat, T::AccountId, ProcessorStateFor<T, I>>;

	/// Individual processors' epoch start and active status.
	#[pallet::storage]
	#[pallet::getter(fn manager_metrics_rewards)]
	pub type ManagerMetricRewards<T: Config<I>, I: 'static = ()> =
		StorageMap<_, Identity, T::ManagerId, MetricsRewardStateFor<T, I>>;

	/// Storage for pools' config and current total value over all active processors.
	#[pallet::storage]
	#[pallet::getter(fn metric_pools)]
	pub type MetricPools<T: Config<I>, I: 'static = ()> =
		StorageMap<_, Identity, PoolId, MetricPoolFor<T>>;

	/// The pool members, active and in warmup status.
	#[pallet::storage]
	#[pallet::getter(fn metrics)]
	pub(super) type Metrics<T: Config<I>, I: 'static = ()> =
		StorageDoubleMap<_, Twox64Concat, T::AccountId, Identity, PoolId, MetricCommitFor<T>>;

	/// The pool members, active and in warmup status.
	#[pallet::storage]
	#[pallet::getter(fn metric_pool_lookup)]
	pub(super) type MetricPoolLookup<T: Config<I>, I: 'static = ()> =
		StorageMap<_, Identity, MetricPoolName, PoolId>;

	/// The commitments of compute as a map `commitment_id` -> `pool_id` -> `metric`.
	///
	/// Metrics committable are limited by what was measured in last completed epoch (see [`MetricsEpochSum`]).
	#[pallet::storage]
	#[pallet::getter(fn compute_commitments)]
	pub(super) type ComputeCommitments<T: Config<I>, I: 'static = ()> =
		StorageDoubleMap<_, Identity, T::CommitmentId, Identity, PoolId, Metric>;

	/// The actual (adjusted) scores of compute as a map `commitment_id` -> `pool_id` -> `SlidingBuffer[epoch -> (score, bonus_score)]`.
	///
	/// The adjustement of scores involves checks if a commitment is in cooldown, if the stake-metric ratio was superseded and stored in second tuple element, if the committer gets a bonus for being busy.
	#[pallet::storage]
	#[pallet::getter(fn scores)]
	pub(super) type Scores<T: Config<I>, I: 'static = ()> = StorageDoubleMap<
		_,
		Identity,
		T::CommitmentId,
		Identity,
		PoolId,
		SlidingBuffer<BlockNumberFor<T>, (U256, U256)>,
		ValueQuery,
	>;

	/// The measured metrics average over an era by pool and all of a manager's active devices as a map `manager_id` -> `pool_id` -> `sliding_buffer[block % (T::Era * T::Epoch) -> (metric, avg_count)]`.
	///
	/// The time unit in [`SlidingBuffer::epoch`] confusingly corresponds to an era for this storage structure!
	///
	/// **DEPRECATED:** This storage item is no longer used and will be removed in a future version.
	#[pallet::storage]
	#[pallet::getter(fn metrics_era_average)]
	#[deprecated]
	pub(super) type MetricsEraAverage<T: Config<I>, I: 'static = ()> = StorageDoubleMap<
		_,
		Identity,
		T::ManagerId,
		Identity,
		PoolId,
		SlidingBuffer<BlockNumberFor<T>, (Metric, u32)>,
	>;

	/// The measured metrics sum over an epoch by pool and all of a manager's active devices as a map `manager_id` -> `pool_id` -> `sliding_buffer[epoch -> (metric_sum, metric_with_bonus_sum)]`.
	#[pallet::storage]
	#[pallet::getter(fn metrics_epoch_sum)]
	pub(super) type MetricsEpochSum<T: Config<I>, I: 'static = ()> = StorageDoubleMap<
		_,
		Identity,
		T::ManagerId,
		Identity,
		PoolId,
		SlidingBuffer<EpochOf<T>, (Metric, Metric)>,
		ValueQuery,
	>;

	/// Pending offers to back manager's compute with a commitment. These are pending offers made by committers waiting for acceptance by manager as a map `committer_id` -> `manager_id` -> `()`.
	#[pallet::storage]
	#[pallet::getter(fn backing_offers)]
	pub(super) type BackingOffers<T: Config<I>, I: 'static = ()> =
		StorageMap<_, Identity, T::AccountId, T::ManagerId>;

	/// Backing managers by commitment as a map `commitment_id` -> `manager_id`. Reverse map of [`Commitments`].
	#[pallet::storage]
	#[pallet::getter(fn backings)]
	pub(super) type Backings<T: Config<I>, I: 'static = ()> =
		StorageMap<_, Identity, T::CommitmentId, T::ManagerId>;

	/// Providers by manager as a map `manager_id` -> `commitment_id`. Reverse map of [`Managers`].
	#[pallet::storage]
	#[pallet::getter(fn backing_lookup)]

	pub(super) type BackingLookup<T: Config<I>, I: 'static = ()> =
		StorageMap<_, Identity, T::ManagerId, T::CommitmentId>;

	#[pallet::storage]
	#[pallet::getter(fn next_commitment_id)]
	pub(super) type NextCommitmentId<T: Config<I>, I: 'static = ()> =
		StorageValue<_, T::CommitmentId, ValueQuery>;

	/// Commitments as a map `commitment_id` -> [`Commitment`].
	#[pallet::storage]
	#[pallet::getter(fn commitments)]
	pub(super) type Commitments<T: Config<I>, I: 'static = ()> =
		StorageMap<_, Identity, T::CommitmentId, CommitmentFor<T, I>>;

	/// Tracks the total stake of all commitments.
	#[pallet::storage]
	#[pallet::getter(fn total_stake)]
	pub type TotalStake<T: Config<I>, I: 'static = ()> =
		StorageValue<_, BalanceFor<T, I>, ValueQuery>;

	/// Delegations as a map `delegator` -> `commitment_id` -> [`Delegation`].
	#[pallet::storage]
	#[pallet::getter(fn delegations)]
	pub(super) type Delegations<T: Config<I>, I: 'static = ()> = StorageDoubleMap<
		_,
		Twox64Concat,
		T::AccountId,
		Identity,
		T::CommitmentId,
		DelegationFor<T, I>,
	>;

	/// Tracks a delegator's total delegated stake. It excludes stakes by committers.
	#[pallet::storage]
	#[pallet::getter(fn delegator_total)]
	pub(super) type DelegatorTotal<T: Config<I>, I: 'static = ()> =
		StorageMap<_, Twox64Concat, T::AccountId, BalanceFor<T, I>, ValueQuery>;

	/// The current epoch with sequential epoch number that increases every [`T::Epoch`] and the start of current epoch.
	#[pallet::storage]
	#[pallet::getter(fn current_cycle)]
	pub type CurrentCycle<T: Config<I>, I: 'static = ()> = StorageValue<_, CycleFor<T>, ValueQuery>;

	/// Storage for compute-based rewards that are distributed for all compute, also **non**-stake-backed compute, as a sliding buffer `epoch` -> `reward`.
	#[pallet::storage]
	#[pallet::getter(fn compute_based_rewards)]
	pub type ComputeBasedRewards<T: Config<I>, I: 'static = ()> =
		StorageValue<_, SlidingBuffer<EpochOf<T>, BalanceFor<T, I>>, ValueQuery>;

	/// Storage for stake-based rewards that are distributed for stake-backed compute, as a sliding buffer `epoch` -> [`RewardBudget`].
	#[pallet::storage]
	#[pallet::getter(fn stake_based_rewards)]
	pub type StakeBasedRewards<T: Config<I>, I: 'static = ()> = StorageMap<
		_,
		Identity,
		PoolId,
		SlidingBuffer<EpochOf<T>, RewardBudgetFor<T, I>>,
		ValueQuery,
	>;

	/// Migration state for V6 migration (clearing MetricsEraAverage)
	#[pallet::storage]
	pub type V11MigrationState<T: Config<I>, I: 'static = ()> =
		StorageValue<_, BoundedVec<u8, ConstU32<80>>, OptionQuery>;

	pub(crate) const STORAGE_VERSION: StorageVersion = StorageVersion::new(11);

	#[pallet::pallet]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T, I = ()>(PhantomData<(T, I)>);

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config<I>, I: 'static = ()> {
		/// PoolCreated. [pool_id, pool_state]
		PoolCreated(PoolId, MetricPoolFor<T>),
		/// A potential committer offered backing to a manager. [account, manager_id]
		BackingOffered(T::AccountId, T::ManagerId),
		/// An potential committer withdrew backing to a manager. [account, manager_id]
		BackingOfferWithdrew(T::AccountId, T::ManagerId),
		/// A manager accepted a committer's backing offer. [committer_id, manager_id]
		BackingAccepted(T::CommitmentId, T::ManagerId),
		/// A commitment with corresponding. [account, commitment_id]
		CommitmentCreated(T::AccountId, T::CommitmentId),
		/// An account started delegation to a commitment. [delegator, commitment_id]
		Delegated(T::AccountId, T::CommitmentId),
		/// An account increased its delegated amount to a commitment. [delegator, commitment_id]
		DelegatedMore(T::AccountId, T::CommitmentId),
		/// An account started the cooldown for an accepted delegation. [delegator, commitment_id]
		DelegationCooldownStarted(T::AccountId, T::CommitmentId),
		/// An account passed the cooldown and ended delegation. [delegator, commitment_id, reward_amount]
		DelegationEnded(T::AccountId, T::CommitmentId, BalanceFor<T, I>),
		/// A committer staked and committed compute provided by the manager he is backing. [commitment_id]
		ComputeCommitted(T::CommitmentId),
		/// A committer increased its stake. [commitment_id]
		StakedMore(T::CommitmentId),
		/// The cooldown for a commitment got started. [commitment_id]
		ComputeCommitmentCooldownStarted(T::CommitmentId),
		/// The cooldown for a commitment has ended. [commitment_id, reward_amount]
		ComputeCommitmentEnded(T::CommitmentId, BalanceFor<T, I>),
		/// A delegator got kicked out. [delegator, commitment_id, reward_amount]
		KickedOut(T::AccountId, T::CommitmentId, BalanceFor<T, I>),
		/// A commitment got slahsed. [commitment_id]
		Slashed(T::CommitmentId),
		/// A delegation was moved from one commitment to another. [delegator, old_commitment_id, new_commitment_id]
		Redelegated(T::AccountId, T::CommitmentId, T::CommitmentId),
		/// A delegator withdrew his accrued rewards and slashes. [delegator, commitment_id, reward_amount]
		DelegatorWithdrew(T::AccountId, T::CommitmentId, BalanceFor<T, I>),
		/// A committer withdrew his accrued rewards and slashes. [committer, commitment_id, reward_amount]
		CommitterWithdrew(T::AccountId, T::CommitmentId, BalanceFor<T, I>),
		/// A delegator compounded his accrued rewards and slashes. [delegator, commitment_id, compound_amount]
		DelegatorCompounded(T::AccountId, T::CommitmentId, BalanceFor<T, I>),
		/// A committer compounded his accrued rewards and slashes. [committer, commitment_id, compound_amount]
		CommitterCompounded(T::AccountId, T::CommitmentId, BalanceFor<T, I>),
		/// V11 migration started (clearing deprecated MetricsEraAverage storage)
		V11MigrationStarted,
		/// V11 migration completed (clearing deprecated MetricsEraAverage storage)
		V11MigrationCompleted,
	}

	// Errors inform users that something went wrong.
	#[pallet::error]
	pub enum Error<T, I = ()> {
		PoolNameMustBeUnique,
		MissingMetricTotal,
		ProcessorNeverCommitted,
		InvalidMetric,
		CalculationOverflow,
		PoolNotFound,
		RewardUpdateInvalid,
		AlreadyOfferedBacking,
		AlreadyBacking,
		NoBackingOfferFound,
		AlreadyDelegating,
		CooldownAlreadyStarted,
		BelowMinCooldownPeriod,
		AboveMaxCooldownPeriod,
		CooldownPeriodCannotDecrease,
		CommissionCannotIncrease,
		CommittedMetricCannotDecrease,
		BelowMinDelegation,
		DelegationCooldownMustBeShorterThanCommitment,
		MaxDelegationRatioExceeded,
		MaxMetricCommitmentExceeded,
		ZeroMetricsForValidPools,
		MinStakeSubceeded,
		InsufficientBalance,
		CooldownNotStarted,
		CooldownNotEnded,
		NotDelegating,
		CommitmentNotFound,
		CommitmentScoreNotFound,
		NewCommitmentNotFound,
		AlreadyCommitted,
		NoManagerBackingCommitment,
		NoOwnerOfCommitmentId,
		InternalError,
		InternalErrorReadingOutdated,
		MaxStakeMetricRatioExceeded,
		CommitmentInCooldown,
		RedelegateBlocked,
		AlreadyDelegatingToRedelegationCommitter,
		RedelegationCommitterCooldownCannotBeShorter,
		RedelegationCommitmentMetricsCannotBeLess,
		AutoCompoundNotAllowed,
		CannotCommit,
		StaleDelegationMustBeEnded,
		EndStaleDelegationsFirst,
		CannotKickout,
		AlreadySlashed,
		NotSlashable,
	}

	#[pallet::hooks]
	impl<T: Config<I>, I: 'static> Hooks<BlockNumberFor<T>> for Pallet<T, I>
	where
		BalanceFor<T, I>: From<u128>,
	{
		fn on_initialize(block_number: BlockNumberFor<T>) -> frame_support::weights::Weight {
			let mut weight = T::DbWeight::get().reads(1);

			weight += crate::migration::migrate::<T, I>();

			// The pallet initializes its cycle tracking on the first block transition (block 1 → block 2), so the epoch_start will be 2.
			let current_cycle = Self::current_cycle();
			let epoch_start = current_cycle.epoch_start;

			let diff = block_number.saturating_sub(epoch_start);
			if epoch_start == Zero::zero() {
				// First time initialization - set to calculated epoch
				let initial_epoch = diff / T::Epoch::get();
				CurrentCycle::<T, I>::put(Cycle {
					epoch: initial_epoch,
					epoch_start: block_number,
				});
				weight = weight.saturating_add(T::DbWeight::get().writes(1));
			} else {
				// Check if we're at an epoch boundary
				if diff % T::Epoch::get() == Zero::zero() {
					let (last_epoch, current_epoch) = CurrentCycle::<T, I>::mutate(|cycle| {
						// Increment the sequential epoch
						let last_epoch = cycle.epoch;
						cycle.epoch = cycle.epoch.saturating_add(One::one());
						cycle.epoch_start = block_number;
						(last_epoch, cycle.epoch)
					});
					weight = weight.saturating_add(T::DbWeight::get().writes(1));

					// Calculate target token supply before inflating
					let target_token_supply = T::TargetStakedTokenSupply::get()
						.mul_floor(T::Currency::total_issuance().saturated_into::<u128>());

					// Handle inflation-based reward distribution on new epoch
					{
						weight = weight.saturating_add(T::DbWeight::get().reads(1));
						let inflation_amount: BalanceFor<T, I> = T::InflationPerEpoch::get();

						let (stake_backed_amount, mut imbalance) = if !inflation_amount.is_zero() {
							// Mint new tokens into distribution account
							let mut imbalance =
								<T::Currency as Balanced<T::AccountId>>::issue(inflation_amount);

							// Calculate split of stake-based and compute-based amount
							let stake_backed_amount: BalanceFor<T, I> =
								T::InflationStakedComputeRation::get()
									.mul_floor(inflation_amount.saturated_into::<u128>())
									.saturated_into();
							let compute_based_amount = T::InflationMetricsRation::get()
								.mul_ceil(inflation_amount.saturated_into::<u128>())
								.saturated_into();
							let compute_imbalance = imbalance
								.extract(stake_backed_amount.saturating_add(compute_based_amount));
							let resolve_result =
								T::Currency::resolve(&Self::account_id(), compute_imbalance);
							if let Err(credit) = resolve_result {
								imbalance = imbalance.merge(credit);
							}

							// Store compute-based rewards
							if !compute_based_amount.is_zero() {
								ComputeBasedRewards::<T, I>::mutate(|r| {
									r.mutate(
										current_epoch,
										|v| {
											*v = compute_based_amount;
										},
										false,
									);
								});

								weight = weight.saturating_add(T::DbWeight::get().writes(1));
							}

							(stake_backed_amount, imbalance)
						} else {
							(Zero::zero(), Default::default())
						};

						// Store stake-based rewards split by pool (to avoid redoing this in every stake-based-reward-claiming heartbeat)
						let mut unused_amount: BalanceFor<T, I> = Zero::zero();

						for (pool_id, pool) in MetricPools::<T, I>::iter() {
							// we have to use the pool's total compute from the last_epoch since only this one is a completed rolling sum
							let target_weight_per_compute =
								U256::from(target_token_supply.saturated_into::<u128>())
									.saturating_mul(U256::from(PER_TOKEN_DECIMALS))
									.saturating_mul(U256::from(
										T::TargetCooldownPeriod::get().saturated_into::<u128>(),
									))
									.checked_div(U256::from(
										T::MaxCooldownPeriod::get().saturated_into::<u128>(),
									))
									.unwrap_or(Zero::zero())
									.saturating_mul(U256::from(FIXEDU128_DECIMALS))
									.checked_div(U256::from(
										pool.total.get(last_epoch).into_inner(),
									))
									.unwrap_or(Zero::zero());
							StakeBasedRewards::<T, I>::mutate(pool_id, |r| {
								// before overwriting the second-to-last epoch's budget, we gather the soon stale_budget to refund it as part of imbalance
								let stale_budget = r.get(last_epoch.saturating_sub(One::one()));
								unused_amount = unused_amount.saturating_add(
									stale_budget.total.saturating_sub(stale_budget.distributed),
								);
								r.mutate(
									current_epoch,
									|v| {
										*v = RewardBudget::new(
											pool.reward
												.get(last_epoch)
												.mul_floor(
													stake_backed_amount.saturated_into::<u128>(),
												)
												.into(),
											target_weight_per_compute,
										);
									},
									false,
								);
							});
							weight = weight.saturating_add(T::DbWeight::get().reads_writes(2, 1));
						}

						if !unused_amount.is_zero() {
							if let Ok(unused) = <T::Currency as Balanced<T::AccountId>>::withdraw(
								&Self::account_id(),
								unused_amount,
								Precision::Exact,
								Preservation::Preserve,
								Fortitude::Polite,
							) {
								imbalance = imbalance.merge(unused);
							}
						}

						T::InflationHandler::on_unbalanced(imbalance);
						weight = weight.saturating_add(T::DbWeight::get().writes(2));
					}
				}
			}

			weight
		}
	}

	#[pallet::call]
	impl<T: Config<I>, I: 'static> Pallet<T, I>
	where
		BlockNumberFor<T>: One,
		BalanceFor<T, I>: From<u128>,
	{
		#[pallet::call_index(0)]
		#[pallet::weight(T::WeightInfo::create_pool(config.len() as u32))]
		pub fn create_pool(
			origin: OriginFor<T>,
			name: MetricPoolName,
			reward: Perquintill,
			config: MetricPoolConfigValues,
		) -> DispatchResultWithPostInfo {
			T::CreateModifyPoolOrigin::ensure_origin(origin)?;

			let (pool_id, pool_state) = Self::do_create_pool(name, reward, config)?;

			Self::deposit_event(Event::<T, I>::PoolCreated(pool_id, pool_state));

			Ok(Pays::No.into())
		}

		/// Modifies a pool's parameter.
		#[pallet::call_index(1)]
		#[pallet::weight(new_config.as_ref().map(|config| match config { ModifyMetricPoolConfig::Replace(c) => T::WeightInfo::modify_pool_replace_config(c.len() as u32), ModifyMetricPoolConfig::Update(ops) => T::WeightInfo::modify_pool_update_config(max(ops.add.len(), ops.remove.len()) as u32) }).unwrap_or(T::WeightInfo::modify_pool_same_config()))]
		pub fn modify_pool(
			origin: OriginFor<T>,
			pool_id: PoolId,
			new_name: Option<MetricPoolName>,
			new_reward_from_epoch: Option<(EpochOf<T>, Perquintill)>,
			new_config: Option<ModifyMetricPoolConfig>,
		) -> DispatchResultWithPostInfo {
			T::CreateModifyPoolOrigin::ensure_origin(origin)?;

			<MetricPools<T, I>>::try_mutate(pool_id, |pool| -> Result<(), Error<T, I>> {
				let p = pool.as_mut().ok_or(Error::<T, I>::PoolNotFound)?;

				if let Some(name) = new_name {
					if let Some(maybe_conflicting_pool_id) = Self::metric_pool_lookup(name) {
						if maybe_conflicting_pool_id != pool_id {
							Err(Error::<T, I>::PoolNameMustBeUnique)?
						}
					}

					<MetricPoolLookup<T, I>>::remove(p.name);
					<MetricPoolLookup<T, I>>::insert(name, pool_id);
					p.name = name;
				}

				// we use current epoch - 1 to be sure no rewards are overwritten that still are used for calculations/claiming
				let previous_epoch = Self::current_cycle().epoch.saturating_sub(One::one());

				if let Some((epoch, reward)) = new_reward_from_epoch {
					p.reward
						.set(previous_epoch, epoch, reward)
						.map_err(|_| Error::<T, I>::RewardUpdateInvalid)?;
				}

				match new_config {
					Some(ModifyMetricPoolConfig::Replace(config)) => {
						p.config = config;
					},
					Some(ModifyMetricPoolConfig::Update(ops)) => {
						let add_keys: Vec<MetricPoolConfigName> = ops
							.add
							.clone()
							.into_iter()
							.map(|(config_name, _, _)| (config_name))
							.collect();
						p.config = BoundedVec::truncate_from(
							p.config
								.clone()
								.into_iter()
								.filter(|(config_name, _, _)| {
									!add_keys.contains(config_name)
										&& !ops.remove.contains(config_name)
								})
								.chain(ops.add.into_iter())
								.collect::<Vec<_>>(),
						);
					},
					None => {},
				}

				Ok(())
			})?;
			Ok(Pays::No.into())
		}

		/// Offers backing to a manager.
		///
		/// An account can only have one outstanding offer and only offer if not already owner of a `commitment`.
		///
		/// NOTE: Only upon acceptance, the offering account will receive a new `commitment_id`. This makes the invariant hold that behind each `commitment_id` (commitment-position/commitment-NFT) there is a commitment backing a manager.
		#[pallet::call_index(3)]
		#[pallet::weight(T::WeightInfo::offer_backing())]
		pub fn offer_backing(
			origin: OriginFor<T>,
			manager: T::AccountId,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;

			// This will be softened later when we support reassigning different manager_ids to commitments
			ensure!(
				T::CommitmentIdProvider::commitment_id_for(&who).ok().is_none(),
				Error::<T, I>::AlreadyBacking
			);

			let manager_id = T::ManagerIdProvider::manager_id_for(&manager)?;

			// it's allowed to overwrite this with a different offer receiver (manager_id) than before
			BackingOffers::<T, I>::insert(&who, manager_id);

			Self::deposit_event(Event::<T, I>::BackingOffered(who, manager_id));

			Ok(().into())
		}

		#[pallet::call_index(4)]
		#[pallet::weight(T::WeightInfo::withdraw_backing_offer())]
		pub fn withdraw_backing_offer(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;

			let manager_id =
				BackingOffers::<T, I>::take(&who).ok_or(Error::<T, I>::NoBackingOfferFound)?;

			Self::deposit_event(Event::<T, I>::BackingOfferWithdrew(who, manager_id));

			Ok(().into())
		}

		#[pallet::call_index(5)]
		#[pallet::weight(T::WeightInfo::accept_backing_offer())]
		pub fn accept_backing_offer(
			origin: OriginFor<T>,
			committer: T::AccountId,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			let manager_id = T::ManagerIdProvider::manager_id_for(&who)?;

			let offer_manager_id =
				BackingOffers::<T, I>::take(committer).ok_or(Error::<T, I>::NoBackingOfferFound)?;
			ensure!(manager_id == offer_manager_id, Error::<T, I>::NoBackingOfferFound);

			// technically this call allows to reuse a commitment_id NFT here, but the mechanism to create commitment IDs in this pallet does not yet allow committers to swap which managers they back (they can do offer-accept-flow at most once)
			let (commitment_id, created) = Self::do_get_or_create_commitment_id(&who)?;
			if created {
				// we always emit this if extrinsic call succeeds, but it's likely to change in the future so we already emit this separate event for the first time a commitment_id-NFT is created
				Self::deposit_event(Event::<T, I>::CommitmentCreated(who, commitment_id));
			}

			<Backings<T, I>>::insert(commitment_id, manager_id);
			<BackingLookup<T, I>>::insert(manager_id, commitment_id);

			Self::deposit_event(Event::<T, I>::BackingAccepted(commitment_id, manager_id));

			Ok(().into())
		}

		/// Commits compute and stakes, defines commission for receiving delegations.
		///
		/// This is called by the committer that should already be backing a manager, the he offered and got accepted to be backing the manager's compute.
		#[pallet::call_index(6)]
		#[pallet::weight(T::WeightInfo::commit_compute())]
		pub fn commit_compute(
			origin: OriginFor<T>,
			amount: BalanceFor<T, I>,
			cooldown_period: BlockNumberFor<T>,
			commitment: BoundedVec<ComputeCommitment, <T as Config<I>>::MaxPools>,
			commission: Perbill,
			allow_auto_compound: bool,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;

			let commitment_id = T::CommitmentIdProvider::commitment_id_for(&who)?;

			Self::validate_max_metric_store_commitments(commitment_id, commitment)?;

			// only call this AFTER storing commitment since it's a requirement for stake_for
			Self::stake_for(&who, amount, cooldown_period, commission, allow_auto_compound)?;

			// Validate max_stake_metric_ratio with new total commitment stake (after `CommitmentStake` was increased)
			Self::validate_max_stake_metric_ratio(commitment_id)?;

			Self::deposit_event(Event::<T, I>::ComputeCommitted(commitment_id));

			Ok(().into())
		}

		#[pallet::call_index(7)]
		#[pallet::weight(T::WeightInfo::stake_more())]
		pub fn stake_more(
			origin: OriginFor<T>,
			extra_amount: BalanceFor<T, I>,
			cooldown_period: Option<BlockNumberFor<T>>,
			commitment: Option<BoundedVec<ComputeCommitment, <T as Config<I>>::MaxPools>>,
			commission: Option<Perbill>,
			allow_auto_compound: Option<bool>,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;

			let commitment_id = T::CommitmentIdProvider::commitment_id_for(&who)?;

			if let Some(commitment) = commitment {
				Self::validate_max_metric_store_commitments(commitment_id, commitment)?;
			}

			Self::stake_more_for(
				&who,
				extra_amount,
				cooldown_period,
				commission,
				allow_auto_compound,
			)?;

			// Validate max_stake_metric_ratio with new total commitment stake (after `CommitmentStake` was increased)
			Self::validate_max_stake_metric_ratio(commitment_id)?;

			Self::deposit_event(Event::<T, I>::StakedMore(commitment_id));

			Ok(().into())
		}

		#[pallet::call_index(8)]
		#[pallet::weight(T::WeightInfo::cooldown_compute_commitment())]
		pub fn cooldown_compute_commitment(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			let commitment_id = T::CommitmentIdProvider::commitment_id_for(&who)?;
			Self::cooldown_commitment_for(commitment_id)?;
			Self::deposit_event(Event::<T, I>::ComputeCommitmentCooldownStarted(commitment_id));
			Ok(().into())
		}

		#[pallet::call_index(9)]
		#[pallet::weight(T::WeightInfo::end_compute_commitment())]
		pub fn end_compute_commitment(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			// the commitment_id does not get destroyed and might be recycled with upcoming features
			let commitment_id = T::CommitmentIdProvider::commitment_id_for(&who)?;
			let compound_amount = Self::end_commitment_for(&who, commitment_id, true)?;

			Self::deposit_event(Event::<T, I>::ComputeCommitmentEnded(
				commitment_id,
				compound_amount,
			));
			Ok(().into())
		}

		#[pallet::call_index(10)]
		#[pallet::weight(T::WeightInfo::delegate())]
		pub fn delegate(
			origin: OriginFor<T>,
			committer: T::AccountId,
			amount: BalanceFor<T, I>,
			cooldown_period: BlockNumberFor<T>,
			allow_auto_compound: bool,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			let commitment_id = T::CommitmentIdProvider::commitment_id_for(&committer)?;

			Self::delegate_for(&who, commitment_id, amount, cooldown_period, allow_auto_compound)?;

			// Validate max_stake_metric_ratio with new total commitment stake (after `CommitmentStake` was increased)
			Self::validate_max_stake_metric_ratio(commitment_id)?;

			Self::deposit_event(Event::<T, I>::Delegated(who, commitment_id));

			Ok(().into())
		}

		#[pallet::call_index(11)]
		#[pallet::weight(T::WeightInfo::cooldown_delegation())]
		pub fn cooldown_delegation(
			origin: OriginFor<T>,
			committer: T::AccountId,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			let commitment_id = T::CommitmentIdProvider::commitment_id_for(&committer)?;
			Self::cooldown_delegation_for(&who, commitment_id)?;
			Self::deposit_event(Event::<T, I>::DelegationCooldownStarted(who, commitment_id));
			Ok(().into())
		}

		/// Redelegates from one commitment to another if allowed.
		///
		/// This are the rules that make a redelegation valid:
		/// - The new commitment must have higher own stake than the current one.
		/// - The new commitment must have higher cooldown than the current one.
		/// - The new commitment must have the required free capacity to accommodate the redelegated stake.
		/// - The new commitment is not in cooldown (since delegators cannot initially delegate to a commitment in cooldown too).
		///
		/// After each redelegation, the same blocking period as for initial delegation restarts, not allowing another immediate redelegation for [`T::RedelegationBlockingPeriod`] epochs.
		/// - The blocking period is waved if the delegator redelegates from a commitment that is in cooldown, in this case an immediate switch is always possible.
		///
		/// Note that it is not mandatory but possible that the delegator is in cooldown when redelegating.
		#[pallet::call_index(12)]
		#[pallet::weight(T::WeightInfo::redelegate())]
		pub fn redelegate(
			origin: OriginFor<T>,
			old_committer: T::AccountId,
			new_committer: T::AccountId,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			let old_commitment_id = T::CommitmentIdProvider::commitment_id_for(&old_committer)?;
			let new_commitment_id = T::CommitmentIdProvider::commitment_id_for(&new_committer)?;

			Self::redelegate_for(&who, old_commitment_id, new_commitment_id)?;

			Self::deposit_event(Event::<T, I>::Redelegated(
				who,
				old_commitment_id,
				new_commitment_id,
			));

			Ok(().into())
		}

		#[pallet::call_index(13)]
		#[pallet::weight(T::WeightInfo::end_delegation())]
		pub fn end_delegation(
			origin: OriginFor<T>,
			committer: T::AccountId,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			let commitment_id = T::CommitmentIdProvider::commitment_id_for(&committer)?;

			let reward_amount = Self::end_delegation_for(&who, commitment_id, true, false)?;

			Self::deposit_event(Event::<T, I>::DelegationEnded(who, commitment_id, reward_amount));
			Ok(().into())
		}

		#[pallet::call_index(14)]
		#[pallet::weight(T::WeightInfo::kick_out())]
		pub fn kick_out(
			origin: OriginFor<T>,
			delegator: T::AccountId,
			committer: T::AccountId,
		) -> DispatchResultWithPostInfo {
			let _who = ensure_signed(origin)?;
			let commitment_id = T::CommitmentIdProvider::commitment_id_for(&committer)?;

			let reward_amount = Self::end_delegation_for(&delegator, commitment_id, true, true)?;

			Self::deposit_event(Event::<T, I>::KickedOut(delegator, commitment_id, reward_amount));

			Ok(Pays::No.into())
		}

		#[pallet::call_index(15)]
		#[pallet::weight(T::WeightInfo::slash())]
		pub fn slash(origin: OriginFor<T>, committer: T::AccountId) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			let commitment_id = T::CommitmentIdProvider::commitment_id_for(&committer)?;

			Self::do_slash(commitment_id, &who)?;

			Self::deposit_event(Event::<T, I>::Slashed(commitment_id));

			Ok(().into())
		}

		/// Force-unstakes a commitment, removing all delegations and the commitment's own stake.
		///
		/// This is a operator-only operation that bypasses normal cooldown and validation checks.
		#[pallet::call_index(17)]
		#[pallet::weight(T::WeightInfo::force_end_commitment())]
		pub fn force_end_commitment(
			origin: OriginFor<T>,
			commitment_id: T::CommitmentId,
		) -> DispatchResultWithPostInfo {
			T::OperatorOrigin::ensure_origin(origin)?;

			Self::force_end_commitment_for(commitment_id);

			Ok(Pays::No.into())
		}

		/// Withdraws accrued rewards and slashes for a delegator.
		///
		/// The caller must be a delegator to the specified commitment.
		#[pallet::call_index(18)]
		#[pallet::weight(T::WeightInfo::withdraw_delegation())]
		pub fn withdraw_delegation(
			origin: OriginFor<T>,
			committer: T::AccountId,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			let commitment_id = T::CommitmentIdProvider::commitment_id_for(&committer)?;

			let reward_amount = Self::withdraw_delegation_for(&who, commitment_id)?;

			Self::deposit_event(Event::<T, I>::DelegatorWithdrew(
				who,
				commitment_id,
				reward_amount,
			));

			Ok(().into())
		}

		/// Withdraws accrued rewards and slashes for a committer.
		///
		/// The caller must be the owner of the specified commitment.
		#[pallet::call_index(19)]
		#[pallet::weight(T::WeightInfo::withdraw_commitment())]
		pub fn withdraw_commitment(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			let commitment_id = T::CommitmentIdProvider::commitment_id_for(&who)?;

			let reward_amount = Self::withdraw_committer_for(&who, commitment_id)?;

			Self::deposit_event(Event::<T, I>::CommitterWithdrew(
				who,
				commitment_id,
				reward_amount,
			));

			Ok(().into())
		}

		/// Delegate more stake for the caller's commitment.
		///
		/// The caller must be the owner of a commitment.
		#[pallet::call_index(20)]
		#[pallet::weight(T::WeightInfo::delegate_more())]
		pub fn delegate_more(
			origin: OriginFor<T>,
			committer: T::AccountId,
			extra_amount: BalanceFor<T, I>,
			cooldown_period: Option<BlockNumberFor<T>>,
			allow_auto_compound: Option<bool>,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			let commitment_id = T::CommitmentIdProvider::commitment_id_for(&committer)?;

			Self::delegate_more_for(
				&who,
				commitment_id,
				extra_amount,
				cooldown_period,
				allow_auto_compound,
			)?;

			// Validate max_stake_metric_ratio with new total commitment stake (after `CommitmentStake` was increased)
			Self::validate_max_stake_metric_ratio(commitment_id)?;

			Self::deposit_event(Event::<T, I>::DelegatedMore(who, commitment_id));

			Ok(().into())
		}

		/// Compound accrued delegation rewards back into delegation.
		///
		/// If some `delegator` is provided, this attempts to compound for a different delegator, otherwise it compounds for caller.
		///
		/// The caller or `delegator` must be a delegator to the specified commitment.
		#[pallet::call_index(21)]
		#[pallet::weight(T::WeightInfo::compound_delegation())]
		pub fn compound_delegation(
			origin: OriginFor<T>,
			committer: T::AccountId,
			delegator: Option<T::AccountId>,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			let commitment_id = T::CommitmentIdProvider::commitment_id_for(&committer)?;

			let delegator = delegator.unwrap_or(who.clone());
			let delegation =
				Self::delegations(&delegator, commitment_id).ok_or(Error::<T, I>::NotDelegating)?;
			// even without auto-compound, the delegator himself can always compound
			ensure!(
				who == delegator || delegation.stake.allow_auto_compound,
				Error::<T, I>::AutoCompoundNotAllowed
			);

			let compound_amount = Self::compound_delegator(&delegator, commitment_id)?;

			Self::deposit_event(Event::<T, I>::DelegatorCompounded(
				delegator,
				commitment_id,
				compound_amount,
			));

			Ok(().into())
		}

		/// Compound accrued stake rewards back into stake.
		///
		/// If some `committer` is provided, this attempts to compound for a different committer, otherwise it compounds for caller.
		///
		/// The caller or `committer` must be the owner of a commitment.
		#[pallet::call_index(22)]
		#[pallet::weight(T::WeightInfo::compound_stake())]
		pub fn compound_stake(
			origin: OriginFor<T>,
			committer: Option<T::AccountId>,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;

			let committer = committer.unwrap_or(who.clone());

			let commitment_id = T::CommitmentIdProvider::commitment_id_for(&committer)
				.map_err(|_| Error::<T, I>::NoOwnerOfCommitmentId)?;

			let stake = Self::commitments(commitment_id)
				.ok_or(Error::<T, I>::CommitmentNotFound)?
				.stake
				.ok_or(Error::<T, I>::CommitmentNotFound)?;
			// even without auto-compound, the committer himself can always compound
			ensure!(
				who == committer || stake.allow_auto_compound,
				Error::<T, I>::AutoCompoundNotAllowed
			);

			let compound_amount = Self::compound_committer(&committer, commitment_id)?;

			Self::deposit_event(Event::<T, I>::CommitterCompounded(
				committer,
				commitment_id,
				compound_amount,
			));

			Ok(().into())
		}
	}

	impl<T: Config<I>, I: 'static> Pallet<T, I> {
		pub fn account_id() -> T::AccountId {
			T::PalletId::get().into_account_truncating()
		}
	}

	impl<T: Config<I>, I: 'static> Pallet<T, I>
	where
		BalanceFor<T, I>: From<u128>,
	{
		fn do_create_pool(
			name: MetricPoolName,
			reward_ratio: Perquintill,
			config: MetricPoolConfigValues,
		) -> Result<(PoolId, MetricPoolFor<T>), DispatchError> {
			if Self::metric_pool_lookup(name).is_some() {
				Err(Error::<T, I>::PoolNameMustBeUnique)?
			}
			let pool_id = LastMetricPoolId::<T, I>::try_mutate::<_, Error<T, I>, _>(|id| {
				*id = id.checked_add(1).ok_or(Error::<T, I>::CalculationOverflow)?;
				Ok(*id)
			})?;

			let pool = MetricPool {
				config,
				name,
				reward: ProvisionalBuffer::new(reward_ratio),
				total: SlidingBuffer::new(Zero::zero()),
				total_with_bonus: SlidingBuffer::new(Zero::zero()),
			};
			MetricPools::<T, I>::insert(pool_id, &pool);
			<MetricPoolLookup<T, I>>::insert(name, pool_id);

			Ok((pool_id, pool))
		}

		pub(crate) fn do_claim(
			claim_epoch: EpochOf<T>,
			claim_epoch_metric_sums: &[(PoolId, (Metric, Metric))],
		) -> Result<BalanceFor<T, I>, Error<T, I>>
		where
			BalanceFor<T, I>: IsType<u128>,
		{
			let total_reward = Self::compute_based_rewards().get(claim_epoch);
			if total_reward.is_zero() {
				return Ok(Zero::zero());
			}

			let mut total_reward_ratio: Perquintill = Zero::zero();
			for (pool_id, (_, metric_with_bonus_sum)) in claim_epoch_metric_sums {
				let pool = <MetricPools<T, I>>::get(pool_id).ok_or(Error::<T, I>::PoolNotFound)?;
				let reward_ratio = pool.reward.get(claim_epoch);

				// Weight reward according to processor compute relative to total compute for pool identified by `pool_id`.
				// The total compute is taken from the latest completed global epoch, which is simple `claim_epoch = epoch - 1`.
				let pool_total: FixedU128 = pool.total_with_bonus.get(claim_epoch);

				// check if we would divide by zero building rational
				let compute_weighted_reward_ratio = if pool_total.is_zero() {
					// can happen if we got commits for this pool but all of them committed metric 0
					Zero::zero()
				} else {
					reward_ratio.saturating_mul(Perquintill::from_rational(
						metric_with_bonus_sum.into_inner(),
						pool_total.into_inner(),
					))
				};

				total_reward_ratio =
					total_reward_ratio.saturating_add(compute_weighted_reward_ratio);
			}

			let reward: BalanceFor<T, I> =
				total_reward_ratio.mul_floor::<u128>(total_reward.into()).into();

			Ok(reward)
		}

		/// Helper to only commit compute for current processor epoch by providing benchmarked results for a (sub)set of metrics.
		///
		/// Metrics are specified with the `pool_id` and **unknown `pool_id`'s are silently skipped.**
		///
		/// See [`Self::commit`] for how this is used to commit metrics and claim for past commits.
		pub(crate) fn do_commit(
			processor: &T::AccountId,
			manager: &(T::AccountId, T::ManagerId),
			metrics: &[MetricInput],
			pool_ids: &[PoolId],
			cycle: CycleFor<T>,
		) -> (BalanceFor<T, I>, bool, bool)
		where
			BalanceFor<T, I>: IsType<u128>,
		{
			let current_block = <frame_system::Pallet<T>>::block_number();

			let active = Processors::<T, I>::mutate(processor, |p_| {
				let p: &mut ProcessorState<_, _, _> = p_.get_or_insert_with(|| {
					// this is the very first commit so create a `ProcessorState` aligning with the current block -> individual epoch start to avoid congestion of claim calls
					let epoch_offset =
						current_block.saturating_sub(cycle.epoch_start) % T::Epoch::get();
					let warmup_end =
						current_block.saturating_add(<T as Config<I>>::WarmupPeriod::get());
					ProcessorState::initial(epoch_offset, warmup_end)
				});

				// check if warmup passed and updated status if so
				if let ProcessorStatus::WarmupUntil(b) = p.status {
					if current_block >= b {
						p.status = ProcessorStatus::Active;
					}
				}

				p.committed = cycle.epoch;
				matches!(p.status, ProcessorStatus::Active)
			});

			let manager_id = manager.1;
			let maybe_previous_epoch_metric_sums = if !metrics.is_empty() {
				Self::commit_new_metrics(processor, manager_id, metrics, active, cycle)
			} else {
				Self::reuse_metrics(processor, manager_id, active, cycle)
			};

			let mut result: BalanceFor<T, I> = Zero::zero();
			let mut metrics_reward_claimed = false;
			let mut staked_compute_reward_claimed = false;

			if let Some(previous_epoch_metric_sums) = maybe_previous_epoch_metric_sums {
				let last_epoch = cycle.epoch.saturating_sub(One::one());
				let metric_rewards =
					Self::do_claim(last_epoch, previous_epoch_metric_sums.as_slice())
						.unwrap_or_default();
				result = result.saturating_add(metric_rewards);
				metrics_reward_claimed = true;
				let Some(commitment_id) = Self::backing_lookup(manager_id) else {
					return (result, metrics_reward_claimed, staked_compute_reward_claimed);
				};
				let bonus = Commitments::<T, I>::mutate(commitment_id, |commitment| {
					let Some(commitment) = commitment.as_mut() else {
						return Zero::zero();
					};
					if commitment.last_scoring_epoch >= cycle.epoch {
						return Zero::zero();
					}

					commitment.last_scoring_epoch = cycle.epoch;

					// distribute for LAST epoch
					// use heartbeat for distribution even if not active (whatever processor heartbeats should distribute)
					let bonus = Self::distribute(last_epoch, commitment_id, commitment, pool_ids)
						.unwrap_or_default();
					_ = Self::score(
						last_epoch,
						cycle.epoch,
						commitment_id,
						commitment,
						previous_epoch_metric_sums.as_slice(),
					);

					bonus
				});
				result = result.saturating_add(bonus);
				staked_compute_reward_claimed = true;
			}

			(result, metrics_reward_claimed, staked_compute_reward_claimed)
		}

		fn update_metrics_epoch_sum(
			manager_id: T::ManagerId,
			pool_id: PoolId,
			metric: Metric,
			epoch: EpochOf<T>,
			bonus: bool,
		) -> Option<(PoolId, (Metric, Metric))> {
			let metric_with_bonus = if bonus {
				let bonus =
					FixedU128::from_inner(T::BusyWeightBonus::get().mul_floor(metric.into_inner()));
				metric.saturating_add(bonus)
			} else {
				metric
			};
			// sum totals
			<MetricPools<T, I>>::mutate(pool_id, |pool| {
				if let Some(pool) = pool.as_mut() {
					pool.add(epoch, metric);
					pool.add_bonus(epoch, metric_with_bonus);
				}
			});
			<MetricsEpochSum<T, I>>::mutate(manager_id, pool_id, |sum| {
				let prev_epoch = sum.epoch;
				sum.mutate(
					epoch,
					|(metric_sum, metric_with_bonus_sum)| {
						*metric_sum = metric_sum.saturating_add(metric);
						*metric_with_bonus_sum =
							metric_with_bonus_sum.saturating_add(metric_with_bonus);
					},
					false,
				);
				if prev_epoch < epoch && epoch - prev_epoch == One::one() {
					Some((pool_id, sum.get(prev_epoch)))
				} else {
					None
				}
			})
		}

		fn commit_new_metrics(
			processor: &T::AccountId,
			manager_id: T::ManagerId,
			metrics: &[MetricInput],
			active: bool,
			cycle: CycleFor<T>,
		) -> Option<Vec<(PoolId, (Metric, Metric))>> {
			let epoch = cycle.epoch;

			let mut prev_metrics_sum: Vec<(PoolId, (Metric, Metric))> = vec![];
			for (pool_id, numerator, denominator) in metrics {
				let Some(metric) = FixedU128::checked_from_rational(
					*numerator,
					if denominator.is_zero() { One::one() } else { *denominator },
				) else {
					continue;
				};
				let before = Metrics::<T, I>::get(processor, pool_id);
				let first_in_epoch = before
					.map(|m| {
						// first value committed for `epoch` wins
						m.epoch < epoch
					})
					.unwrap_or(true);
				if first_in_epoch {
					// insert even if not active for tracability before warmup ended
					Metrics::<T, I>::insert(processor, pool_id, MetricCommit { epoch, metric });
					if active {
						if let Some(prev_sum) = Self::update_metrics_epoch_sum(
							manager_id, *pool_id, metric, epoch, false,
						) {
							prev_metrics_sum.push(prev_sum);
						}
					}
				}
			}
			if prev_metrics_sum.len() == metrics.len() {
				Some(prev_metrics_sum)
			} else {
				None
			}
		}

		fn reuse_metrics(
			processor: &T::AccountId,
			manager_id: T::ManagerId,
			active: bool,
			cycle: CycleFor<T>,
		) -> Option<Vec<(PoolId, (Metric, Metric))>> {
			let epoch = cycle.epoch;

			let mut to_update: Vec<(PoolId, MetricCommit<_>)> = vec![];
			for (pool_id, metric) in Metrics::<T, I>::iter_prefix(processor) {
				if epoch > metric.epoch && epoch - metric.epoch < T::MetricValidity::get() {
					to_update.push((pool_id, metric));
				}
			}
			let mut prev_metrics_sum: Vec<(PoolId, (Metric, Metric))> = vec![];
			let to_update_length = to_update.len();
			for (pool_id, commit) in to_update {
				// if we are here we now that this reused metric is "first_in_epoch" since we reuse maximally once per epoch
				Metrics::<T, I>::insert(
					processor,
					pool_id,
					MetricCommit { epoch, metric: commit.metric },
				);

				if active {
					if let Some(prev_sum) = Self::update_metrics_epoch_sum(
						manager_id,
						pool_id,
						commit.metric,
						epoch,
						true,
					) {
						prev_metrics_sum.push(prev_sum);
					}
				}
			}
			if prev_metrics_sum.len() == to_update_length {
				Some(prev_metrics_sum)
			} else {
				None
			}
		}

		/// Returns the manager id for the given manager account. If a manager id does not exist it is first created.
		pub fn do_get_or_create_commitment_id(
			committer: &T::AccountId,
		) -> Result<(T::CommitmentId, bool), DispatchError> {
			T::CommitmentIdProvider::commitment_id_for(committer)
				.map(|id| (id, false))
				.or_else::<DispatchError, _>(|_| {
					let id = NextCommitmentId::<T, I>::try_mutate::<_, Error<T, I>, _>(|id| {
						let new_id = *id;
						*id = id
							.checked_add(&1u128.into())
							.ok_or(Error::<T, I>::CalculationOverflow)?;
						Ok(new_id)
					})?;

					T::CommitmentIdProvider::create_commitment_id(id, committer)?;

					Ok((id, true))
				})
		}
	}
}
