#![cfg_attr(not(feature = "std"), no_std)]

pub use datastructures::*;
use frame_system::pallet_prelude::BlockNumberFor;
pub use pallet::*;
pub use traits::*;
pub use types::*;

pub(crate) use pallet::STORAGE_VERSION;

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
		AccountLookup, CommitmentIdProvider, ManagerIdProvider, MetricInput, PoolId,
	};
	use frame_support::{
		dispatch::DispatchResultWithPostInfo,
		pallet_prelude::*,
		traits::{
			Currency, EnsureOrigin, ExistenceRequirement, Get, InspectLockableCurrency,
			LockIdentifier,
		},
		PalletId, Parameter,
	};
	use frame_system::pallet_prelude::*;
	use frame_system::pallet_prelude::{BlockNumberFor, OriginFor};
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
		/// This is currently the important cycle on which the compute reward system operates, apart from the longer period for average of metrics, see [`T::Era`].
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
		/// Defines the duration of an era the number of epochs per era. Era duration = `T::Era * T::Epoch` blocks.
		///
		/// It is currently only used as the duration of the period storing the moving average of metrics that is retained.
		#[pallet::constant]
		type Era: Get<EraOf<Self>>;
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
		/// The minimum possible delegated amount towards a commitment. There is no maximum for the amount which a delegator can offer, but it's still limited by the [`Self::MaxDelegationRatio`].
		#[pallet::constant]
		type MinDelegation: Get<BalanceFor<Self, I>>;
		/// The maximum ratio `delegated_stake / commitment_total_stake = delegated_stake / (delegated_stake + committer_stake)`.
		#[pallet::constant]
		type MaxDelegationRatio: Get<Perquintill>;
		#[pallet::constant]
		type CooldownRewardRatio: Get<Perquintill>;
		/// The minimum stake by a committer.
		#[pallet::constant]
		type MinStake: Get<BalanceFor<Self, I>>;
		/// How long a processor needs to warm up before his metrics are respected for compute score and reward calculation.
		#[pallet::constant]
		type WarmupPeriod: Get<BlockNumberFor<Self>>;
		type Currency: InspectLockableCurrency<Self::AccountId, Moment = BlockNumberFor<Self>>;
		type Decimals: Get<BalanceFor<Self, I>>;
		/// The single lock indentifier used for the sum of all staked and delegated amounts.
		///
		/// We have to use the same lock identifier since we do not want the locks to overlap;
		/// Eventhough a staker can also delegate at the same time, the same funds contributing to total balance of an account is either delegated or staked, not both.
		#[pallet::constant]
		type LockIdentifier: Get<LockIdentifier>;
		type ManagerProviderForEligibleProcessor: AccountLookup<Self::AccountId>;
		#[pallet::constant]
		type InflationPerEpoch: Get<BalanceFor<Self, I>>;
		#[pallet::constant]
		type InflationStakedBackedRation: Get<Perquintill>;
		/// Origin that can create and modify pools
		type CreateModifyPoolOrigin: EnsureOrigin<Self::RuntimeOrigin>;
		/// Origin that can execute operational extrinsics
		type OperatorOrigin: EnsureOrigin<Self::RuntimeOrigin>;
		/// Weight Info for extrinsics.
		type WeightInfo: WeightInfo;
	}

	#[pallet::genesis_config]
	pub struct GenesisConfig<T: Config<I>, I: 'static = ()> {
		pub pools: Vec<(MetricPoolName, Perquintill, FixedU128, MetricPoolConfigValues)>,
		phantom: PhantomData<(T, I)>,
	}

	impl<T: Config<I>, I: 'static> Default for GenesisConfig<T, I> {
		fn default() -> Self {
			Self { pools: Default::default(), phantom: Default::default() }
		}
	}

	#[pallet::genesis_build]
	impl<T: Config<I>, I: 'static> BuildGenesisConfig for GenesisConfig<T, I> {
		fn build(&self) {
			for (name, reward_ratio, max_stake_metric_ratio, config) in self.pools.clone() {
				if let Err(e) = Pallet::<T, I>::do_create_pool(
					name,
					reward_ratio,
					max_stake_metric_ratio,
					config,
				) {
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

	#[pallet::storage]
	#[pallet::getter(fn reward_distribution_settings)]
	pub type RewardDistributionSettings<T: Config<I>, I: 'static = ()> =
		StorageValue<_, RewardDistributionSettingsFor<T, I>, OptionQuery>;

	/// The commitments of compute as a map `commitment_id` -> `pool_id` -> [`Stake`].
	///
	/// Metrics committable are limited by a ratio of what was measured as average in last completed era (see [`MetricsEraAverage`]).
	#[pallet::storage]
	#[pallet::getter(fn compute_commitments)]
	pub(super) type ComputeCommitments<T: Config<I>, I: 'static = ()> =
		StorageDoubleMap<_, Identity, T::CommitmentId, Identity, PoolId, Metric>;

	/// The relative commission taken by committers from delegator's reward as a map `commitment_id` -> [`Perbill`].
	#[pallet::storage]
	#[pallet::getter(fn commission)]
	pub(super) type Commission<T: Config<I>, I: 'static = ()> =
		StorageMap<_, Identity, T::CommitmentId, Perbill>;

	/// The measured metrics average over an era by pool and all of a manager's active devices as a map `manager_id` -> `pool_id` -> `sliding_buffer[block % (T::Era * T::Epoch) -> (metric, avg_count)]`.
	///
	/// The time unit in [`SlidingBuffer::epoch`] confusingly corresponds to an era for this storage structure!
	#[pallet::storage]
	#[pallet::getter(fn metrics_era_average)]
	pub(super) type MetricsEraAverage<T: Config<I>, I: 'static = ()> = StorageDoubleMap<
		_,
		Identity,
		T::ManagerId,
		Identity,
		PoolId,
		SlidingBuffer<EraOf<T>, (Metric, u32)>,
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

	/// Stakes by commitment as a map `commitment_id` -> [`Stake`].
	#[pallet::storage]
	#[pallet::getter(fn stakes)]
	pub(super) type Stakes<T: Config<I>, I: 'static = ()> =
		StorageMap<_, Identity, T::CommitmentId, StakeFor<T, I>>;

	/// Pool member state for commitments in their own delegation pool as a map `commitment_id` -> [`DelegationPoolMember`].
	#[pallet::storage]
	#[pallet::getter(fn self_delegation)]
	pub(super) type SelfDelegation<T: Config<I>, I: 'static = ()> =
		StorageMap<_, Identity, T::CommitmentId, DelegationPoolMemberFor<T, I>>;

	/// Pool member state as a map `commitment_id` -> `pool_id` -> [`StakingPoolMember`].
	#[pallet::storage]
	#[pallet::getter(fn staking_pool_members)]
	pub(super) type StakingPoolMembers<T: Config<I>, I: 'static = ()> = StorageDoubleMap<
		_,
		Identity,
		T::CommitmentId,
		Identity,
		PoolId,
		StakingPoolMemberFor<T, I>,
	>;

	/// Tracks a commitment's backing stake, inclusive committer stake and delegated stakes received as (total_amount, rewardable_amount).
	///
	/// - `total_amount`: Total staked including cooldown (used for slashing)
	/// - `rewardable_amount`: Amount eligible for rewards (used for reward calculations)
	#[pallet::storage]
	#[pallet::getter(fn commitment_stake)]
	pub(super) type CommitmentStake<T: Config<I>, I: 'static = ()> =
		StorageMap<_, Identity, T::CommitmentId, (BalanceFor<T, I>, BalanceFor<T, I>), ValueQuery>;

	/// Tracks the total stake of all commitments.
	#[pallet::storage]
	#[pallet::getter(fn total_stake)]
	pub type TotalStake<T: Config<I>, I: 'static = ()> =
		StorageValue<_, BalanceFor<T, I>, ValueQuery>;

	/// Delegations (delegated stakes) as a map `delegator` -> `commitment_id` -> [`Stake`].
	#[pallet::storage]
	#[pallet::getter(fn delegations)]
	pub(super) type Delegations<T: Config<I>, I: 'static = ()> =
		StorageDoubleMap<_, Twox64Concat, T::AccountId, Identity, T::CommitmentId, StakeFor<T, I>>;

	/// Pool member state by delegator as a map `delegator` -> `commitment_id` -> [`PoolMember`].
	#[pallet::storage]
	#[pallet::getter(fn delegation_pool_members)]
	pub(super) type DelegationPoolMembers<T: Config<I>, I: 'static = ()> = StorageDoubleMap<
		_,
		Twox64Concat,
		T::AccountId,
		Identity,
		T::CommitmentId,
		DelegationPoolMemberFor<T, I>,
	>;

	/// Tracks a delegator's total delegated stake. It excludes self-delegations by committers.
	#[pallet::storage]
	#[pallet::getter(fn delegator_total)]
	pub(super) type DelegatorTotal<T: Config<I>, I: 'static = ()> =
		StorageMap<_, Twox64Concat, T::AccountId, BalanceFor<T, I>, ValueQuery>;

	/// Tracks the total delegated stake by all delegators. It excludes self-delegations by committers.
	#[pallet::storage]
	#[pallet::getter(fn total_delegated)]
	pub type TotalDelegated<T: Config<I>, I: 'static = ()> =
		StorageValue<_, BalanceFor<T, I>, ValueQuery>;

	/// Storage for metric pools' staking metadata for constant-time reward distribution.
	#[pallet::storage]
	#[pallet::getter(fn staking_pools)]
	pub type StakingPools<T: Config<I>, I: 'static = ()> =
		StorageMap<_, Identity, PoolId, StakingPoolFor<T, I>, ValueQuery>;

	/// Storage for staking metadata for each provider's own pool for constant-time reward distribution among delegators to each provider and metric the provider committed to.
	///
	/// Delegatios are automatically transferred to a new holder of commitment_id when transferring commitment_id NFT.
	#[pallet::storage]
	#[pallet::getter(fn delegation_pools)]
	pub(super) type DelegationPools<T: Config<I>, I: 'static = ()> =
		StorageMap<_, Identity, T::CommitmentId, DelegationPoolFor<T, I>, ValueQuery>;

	/// The current epoch with sequential epoch number that increases every [`T::Epoch`] and the start of current epoch.
	#[pallet::storage]
	#[pallet::getter(fn current_cycle)]
	pub type CurrentCycle<T: Config<I>, I: 'static = ()> = StorageValue<_, CycleFor<T>, ValueQuery>;

	/// Storage for compute-based rewards that are not stake-backed, as a map `epoch` -> `reward`.
	#[pallet::storage]
	#[pallet::getter(fn compute_based_rewards)]
	pub type ComputeBasedRewards<T: Config<I>, I: 'static = ()> =
		StorageValue<_, SlidingBuffer<EpochOf<T>, BalanceFor<T, I>>, ValueQuery>;

	pub(crate) const STORAGE_VERSION: StorageVersion = StorageVersion::new(5);

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
		/// An account passed the cooldown and ended delegation. [delegator, commitment_id]
		DelegationEnded(T::AccountId, T::CommitmentId),
		/// A committer staked and committed compute provided by the manager he is backing. [commitment_id]
		ComputeCommitted(T::CommitmentId),
		/// A committer increased its stake. [commitment_id, extra_amount]
		StakedMore(T::CommitmentId, BalanceFor<T, I>),
		/// The cooldown for a commitment got started. [commitment_id]
		ComputeCommitmentCooldownStarted(T::CommitmentId),
		/// The cooldown for a commitment has ended. [commitment_id]
		ComputeCommitmentEnded(T::CommitmentId),
		/// A reward got distrubuted. [amount]
		Rewarded(BalanceFor<T, I>),
		/// A commitment got slahsed. [commitment_id, amount]
		Slashed(T::CommitmentId, BalanceFor<T, I>),
		/// A delegation was moved from one commitment to another. [delegator, old_commitment_id, new_commitment_id]
		Redelegated(T::AccountId, T::CommitmentId, T::CommitmentId),
		/// A delegator withdrew his accrued rewards and slashes. [delegator, commitment_id, reward_amount]
		DelegatorWithdrew(T::AccountId, T::CommitmentId, BalanceFor<T, I>),
		/// A committer withdrew his accrued rewards and slashes. [committer, commitment_id, reward_amount]
		CommitterWithdrew(T::AccountId, T::CommitmentId, BalanceFor<T, I>),
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
		NewCommitmentNotFound,
		AlreadyCommitted,
		NoMetricsAverage,
		NoManagerBackingCommitment,
		NoOwnerOfCommitmentId,
		InternalError,
		MaxStakeMetricRatioExceeded,
		CommitmentNotInCooldown,
		DelegatorInCooldown,
		RedelegationCommitterCooldownCannotBeShorter,
		RedelegationCommitmentMetricsCannotBeLess,
		AutoCompoundNotAllowed,
		CannotCommit,
		StaleDelegationMustBeEnded,
	}

	#[pallet::hooks]
	impl<T: Config<I>, I: 'static> Hooks<BlockNumberFor<T>> for Pallet<T, I>
	where
		BalanceFor<T, I>: From<u128>,
	{
		fn on_runtime_upgrade() -> frame_support::weights::Weight {
			crate::migration::migrate::<T, I>()
		}

		fn on_initialize(block_number: BlockNumberFor<T>) -> frame_support::weights::Weight {
			let mut weight = T::DbWeight::get().reads(1);

			// The pallet initializes its cycle tracking on the first block transition (block 1 → block 2), so the epoch_start and era_start will be 2.
			let epoch_start = Self::current_cycle().epoch_start;
			let era_start = Self::current_cycle().era_start;

			let diff = block_number.saturating_sub(epoch_start);
			let era_diff = block_number.saturating_sub(era_start);
			if epoch_start == Zero::zero() {
				// First time initialization - set to calculated epoch
				let initial_epoch = diff / T::Epoch::get();
				let initial_era = era_diff / (T::Era::get().saturating_mul(T::Epoch::get()));
				CurrentCycle::<T, I>::put(Cycle {
					epoch: initial_epoch,
					epoch_start: block_number,
					era: initial_era,
					era_start: block_number,
				});
				weight = weight.saturating_add(T::DbWeight::get().writes(1));
			} else {
				// Check if we're at an epoch boundary
				if diff % T::Epoch::get() == Zero::zero() {
					CurrentCycle::<T, I>::mutate(|cycle| {
						// Increment the sequential epoch
						cycle.epoch = cycle.epoch.saturating_add(One::one());
						cycle.epoch_start = block_number;

						// Check if we're at an era boundary, which we only can be when also at an epoch boundary
						if era_diff % (T::Era::get().saturating_mul(T::Epoch::get()))
							== Zero::zero()
						{
							// Increment the sequential epoch
							cycle.era = cycle.era.saturating_add(One::one());
							cycle.era_start = block_number;
						}
					});
					weight = weight.saturating_add(T::DbWeight::get().writes(1));

					// Handle inflation-based reward distribution on new epoch
					{
						weight = weight.saturating_add(T::DbWeight::get().reads(1));
						let inflation_amount: BalanceFor<T, I> = T::InflationPerEpoch::get();
						let current_epoch = Self::current_cycle().epoch;

						if !inflation_amount.is_zero() {
							// Mint new tokens into distribution account
							let imbalance = T::Currency::issue(inflation_amount);
							T::Currency::resolve_creating(
								&T::PalletId::get().into_account_truncating(),
								imbalance,
							);
							weight = weight.saturating_add(T::DbWeight::get().writes(1));

							// Calculate stake-backed amount
							let stake_backed_amount: BalanceFor<T, I> =
								T::InflationStakedBackedRation::get()
									.mul_floor(inflation_amount.saturated_into::<u128>())
									.saturated_into();
							let compute_based_amount =
								inflation_amount.saturating_sub(stake_backed_amount);

							// Distribute stake-backed rewards
							if !stake_backed_amount.is_zero() {
								if let Err(e) = Self::distribute(current_epoch, stake_backed_amount)
								{
									log::error!(
										target: LOG_TARGET,
										"Failed to distribute stake-backed rewards: {:?}",
										e
									);
								} else {
									weight = weight
										.saturating_add(T::DbWeight::get().reads_writes(10, 5));
								}
							}

							// Store compute-based rewards
							if !compute_based_amount.is_zero() {
								ComputeBasedRewards::<T, I>::mutate(|r| {
									r.mutate(current_epoch, |v| {
										*v = compute_based_amount;
									});
								});

								weight = weight.saturating_add(T::DbWeight::get().writes(1));
							}
						}
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
			max_stake_metric_ratio: Option<FixedU128>,
			config: MetricPoolConfigValues,
		) -> DispatchResultWithPostInfo {
			T::CreateModifyPoolOrigin::ensure_origin(origin)?;

			let (pool_id, pool_state) = Self::do_create_pool(
				name,
				reward,
				max_stake_metric_ratio.unwrap_or(Zero::zero()),
				config,
			)?;

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
			new_max_stake_metric_ratio: Option<FixedU128>,
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
				if let Some(r) = new_max_stake_metric_ratio {
					p.max_stake_metric_ratio = r;
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
			let manager_id = <Backings<T, I>>::get(commitment_id)
				.ok_or(Error::<T, I>::NoManagerBackingCommitment)?;
			let era = Self::current_cycle().era;
			let mut count = 0;
			for c in commitment {
				if <MetricPools<T, I>>::get(c.pool_id).is_none() {
					continue; // Pool not found, skip; this is not an internal error since maybe pool got deleted but older version of processor still supplies metric
				};
				let avg = <MetricsEraAverage<T, I>>::get(manager_id, c.pool_id)
					.ok_or(Error::<T, I>::NoMetricsAverage)?;
				let (avg_value, _) =
					avg.get(era.checked_sub(&One::one()).ok_or(Error::<T, I>::CannotCommit)?);
				ensure!(c.metric < avg_value, Error::<T, I>::MaxMetricCommitmentExceeded);
				let ratio = Perquintill::from_parts(((c.metric / avg_value).into_inner()) as u64);
				ensure!(
					ratio <= T::MaxMetricCommitmentRatio::get(),
					Error::<T, I>::MaxMetricCommitmentExceeded
				);

				ComputeCommitments::<T, I>::insert(commitment_id, c.pool_id, c.metric);
				count += 1;
			}
			ensure!(count > 0, Error::<T, I>::ZeroMetricsForValidPools);

			// commission can only be inserted once on commit, since stake_for below errors if already committe
			Commission::<T, I>::insert(commitment_id, commission);

			// only call this AFTER storing commitment since it's a requirement for stake_for
			Self::stake_for(&who, amount, cooldown_period, allow_auto_compound)?;

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
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;

			let commitment_id = T::CommitmentIdProvider::commitment_id_for(&who)?;

			Self::stake_more_for(&who, extra_amount)?;

			// Validate max_stake_metric_ratio with new total commitment stake (after `CommitmentStake` was increased)
			Self::validate_max_stake_metric_ratio(commitment_id)?;

			Self::deposit_event(Event::<T, I>::StakedMore(commitment_id, extra_amount));

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
			let reward = Self::end_commitment_for(&who, commitment_id)?;

			// Transfer reward to the caller if any
			if !reward.is_zero() {
				T::Currency::transfer(
					&T::PalletId::get().into_account_truncating(),
					&who,
					reward,
					ExistenceRequirement::KeepAlive,
				)?;
			}

			Commission::<T, I>::remove(commitment_id);
			Self::deposit_event(Event::<T, I>::ComputeCommitmentEnded(commitment_id));
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
		/// This are the rules to allow a redelegation:
		/// - The delegator must currently be delegating to a commitment that is in cooldown,
		/// - the delegator itself must not be in cooldown,
		/// - the new commitment must have higher own stake than the previous one.
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

			Self::end_delegation_for(&who, commitment_id, true)?;

			Self::deposit_event(Event::<T, I>::DelegationEnded(who, commitment_id));
			Ok(().into())
		}

		#[pallet::call_index(14)]
		#[pallet::weight(T::WeightInfo::reward())]
		pub fn reward(
			origin: OriginFor<T>,
			amount: BalanceFor<T, I>,
		) -> DispatchResultWithPostInfo {
			T::OperatorOrigin::ensure_origin(origin)?;

			Self::distribute(Self::current_cycle().epoch, amount)?;

			Self::deposit_event(Event::<T, I>::Rewarded(amount));

			Ok(Pays::No.into())
		}

		#[pallet::call_index(15)]
		#[pallet::weight(T::WeightInfo::slash())]
		pub fn slash(
			origin: OriginFor<T>,
			committer: T::AccountId,
			amount: BalanceFor<T, I>,
		) -> DispatchResultWithPostInfo {
			T::OperatorOrigin::ensure_origin(origin)?;

			let commitment_id = T::CommitmentIdProvider::commitment_id_for(&committer)?;
			Self::slash_delegation_pool(commitment_id, amount)?;

			Self::deposit_event(Event::<T, I>::Slashed(commitment_id, amount));

			Ok(Pays::No.into())
		}

		/// Force-unstakes a commitment, removing all delegations and the commitment's own stake.
		///
		/// This is a root-only operation that bypasses normal cooldown and validation checks.
		#[pallet::call_index(17)]
		#[pallet::weight(T::WeightInfo::force_end_commitment())]
		pub fn force_end_commitment(
			origin: OriginFor<T>,
			committer: T::AccountId,
		) -> DispatchResultWithPostInfo {
			T::OperatorOrigin::ensure_origin(origin)?;

			let commitment_id = T::CommitmentIdProvider::commitment_id_for(&committer)?;

			Self::force_end_commitment_for(commitment_id);

			Commission::<T, I>::remove(commitment_id);

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

			let reward = Self::withdraw_committer_accrued(commitment_id)?;

			// Transfer reward to the caller if any
			if !reward.is_zero() {
				T::Currency::transfer(
					&T::PalletId::get().into_account_truncating(),
					&who,
					reward,
					ExistenceRequirement::KeepAlive,
				)?;
			}

			Self::deposit_event(Event::<T, I>::CommitterWithdrew(who, commitment_id, reward));

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
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			let commitment_id = T::CommitmentIdProvider::commitment_id_for(&committer)?;

			Self::delegate_more_for(&who, commitment_id, extra_amount)?;

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
				who == delegator || delegation.allow_auto_compound,
				Error::<T, I>::AutoCompoundNotAllowed
			);

			Self::compound_delegator(&delegator, commitment_id)?;

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

			let stake = Self::stakes(commitment_id).ok_or(Error::<T, I>::CommitmentNotFound)?;
			// even without auto-compound, the committer himself can always compound
			ensure!(
				who == committer || stake.allow_auto_compound,
				Error::<T, I>::AutoCompoundNotAllowed
			);

			Self::compound_committer(&committer, commitment_id)?;

			Ok(().into())
		}
	}

	impl<T: Config<I>, I: 'static> Pallet<T, I> {
		fn do_create_pool(
			name: MetricPoolName,
			reward_ratio: Perquintill,
			max_stake_metric_ratio: FixedU128,
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
				max_stake_metric_ratio,
			};
			MetricPools::<T, I>::insert(pool_id, &pool);
			<MetricPoolLookup<T, I>>::insert(name, pool_id);

			Ok((pool_id, pool))
		}

		/// Validates that the max_stake_metric_ratio is not violated for all pools where the commitment has committed non-zero metrics.
		fn validate_max_stake_metric_ratio(
			commitment_id: T::CommitmentId,
		) -> Result<(), Error<T, I>> {
			let (_, rewardable_stake) = Self::commitment_stake(commitment_id);
			// Check existing commitments from storage
			for (pool_id, metric_value) in <ComputeCommitments<T, I>>::iter_prefix(commitment_id) {
				// Get the pool to check if it has max_stake_metric_ratio configured
				let Some(pool) = <MetricPools<T, I>>::get(pool_id) else {
					continue; // Pool not found, skip; this is not an internal error since maybe pool got deleted but older version of processor still supplies metric
				};

				// Skip if metric or configured max ratio is zero
				if metric_value.is_zero() || pool.max_stake_metric_ratio.is_zero() {
					continue;
				}

				// Calculate actual ratio: total_stake / metric
				let actual_ratio = FixedU128::checked_from_rational(
					rewardable_stake.saturated_into::<u128>(),
					metric_value.into_inner(),
				)
				.ok_or(Error::<T, I>::MaxStakeMetricRatioExceeded)?;

				// Check if actual ratio exceeds max allowed ratio
				ensure!(
					actual_ratio <= pool.max_stake_metric_ratio,
					Error::<T, I>::MaxStakeMetricRatioExceeded
				);
			}

			Ok(())
		}

		pub(crate) fn do_claim(
			processor: &T::AccountId,
			pool_ids: Vec<PoolId>,
		) -> Result<Option<BalanceFor<T, I>>, Error<T, I>>
		where
			BalanceFor<T, I>: IsType<u128>,
		{
			let Some(manager) = T::ManagerProviderForEligibleProcessor::lookup(processor) else {
				return Ok(None);
			};

			let Some(claim_epoch) = Self::can_claim(processor) else { return Ok(None) };

			let total_reward = Self::compute_based_rewards().get(claim_epoch);
			if total_reward.is_zero() {
				return Ok(None);
			}

			let mut p: ProcessorStateFor<T, I> =
				Processors::<T, I>::get(processor).ok_or(Error::<T, I>::ProcessorNeverCommitted)?;

			let mut total_reward_ratio: Perquintill = Zero::zero();
			for pool_id in pool_ids {
				// we allow partial metrics committed (for backwards compatibility)
				let Some(commit) = Metrics::<T, I>::get(processor, pool_id) else {
					continue;
				};

				// NOTE: we ensured previously in can_claim that p.committed is current processor's epoch - 1, so this validates recentness of individual metric commits
				if commit.epoch != p.committed {
					continue;
				}

				let pool = <MetricPools<T, I>>::get(pool_id).ok_or(Error::<T, I>::PoolNotFound)?;
				let reward_ratio = pool.reward.get(claim_epoch);

				// Weight reward according to processor compute relative to total compute for pool identified by `pool_id`.
				// The total compute is taken from the latest completed global epoch, which is simple `claim_epoch = epoch - 1`.
				let pool_total: FixedU128 = pool.total.get(claim_epoch);

				// check if we would divide by zero building rational
				let compute_weighted_reward_ratio = if pool_total.is_zero() {
					// can happen if we got commits for this pool but all of them committed metric 0
					Zero::zero()
				} else {
					reward_ratio.saturating_mul(Perquintill::from_rational(
						commit.metric.into_inner(),
						pool_total.into_inner(),
					))
				};

				total_reward_ratio =
					total_reward_ratio.saturating_add(compute_weighted_reward_ratio);
			}

			let reward: BalanceFor<T, I> = total_reward_ratio
				.mul_floor::<u128>(total_reward.into())
				.saturating_add(p.accrued.into())
				.into();

			// accrue
			#[allow(clippy::bind_instead_of_map)]
			let _ = T::Currency::transfer(
				&T::PalletId::get().into_account_truncating(),
				&manager,
				reward,
				ExistenceRequirement::KeepAlive,
			)
			.and_then(|_| {
				p.paid = p.paid.saturating_add(reward);
				p.accrued = Zero::zero();
				Ok(())
			})
			.or_else(|e| {
				log::warn!(
					target: LOG_TARGET,
					"Failed to distribute reward; accrueing instead for later pay out. {:?}",
					e
				);
				// amount remains in accrued
				p.accrued = p.accrued.saturating_add(reward);
				Ok::<_, DispatchError>(())
			});

			p.claimed = claim_epoch;

			Processors::<T, I>::insert(processor, p);

			Ok(Some(reward))
		}

		fn can_claim(processor: &T::AccountId) -> Option<EpochOf<T>> {
			let p = Processors::<T, I>::get(processor)?;
			let current_block = <frame_system::Pallet<T>>::block_number();
			(p.committed.saturating_add(One::one()) == Self::current_cycle().epoch
				&& p.claimed < p.committed
				&& match p.status {
					ProcessorStatus::WarmupUntil(b) => b <= current_block,
					ProcessorStatus::Active => true,
				})
			.then_some(p.committed)
		}

		/// Helper to only commit compute for current processor epoch by providing benchmarked results for a (sub)set of metrics.
		///
		/// Metrics are specified with the `pool_id` and **unknown `pool_id`'s are silently skipped.**
		///
		/// See [`Self::commit`] for how this is used to commit metrics and claim for past commits.
		pub(crate) fn do_commit(processor: &T::AccountId, metrics: &[MetricInput]) {
			let current_block = <frame_system::Pallet<T>>::block_number();
			// The global epoch number
			let cycle = Self::current_cycle();

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

			let manager_id = T::ManagerProviderForEligibleProcessor::lookup(processor)
				.map(|manager| T::ManagerIdProvider::manager_id_for(&manager).ok())
				.unwrap_or(None);
			if !metrics.is_empty() {
				Self::commit_new_metrics(processor, manager_id, metrics, active, cycle);
			} else {
				Self::reuse_metrics(processor, manager_id, active, cycle);
			}
		}

		fn update_era_average(
			manager_id: T::ManagerId,
			pool_id: &PoolId,
			metric: Metric,
			cycle: CycleFor<T>,
		) {
			<MetricsEraAverage<T, I>>::mutate(manager_id, pool_id, |avg_| {
				if avg_.is_none() {
					// creates the buffer with defaults, so 0 average and 0 avg_count
					*avg_ = Some(SlidingBuffer::new(Zero::zero()));
				}

				let avg = avg_.as_mut().unwrap();
				avg.mutate(cycle.era, |(v, c)| {
					*v = v.saturating_mul(FixedU128::from_u32(*c));
					*v = v.saturating_add(metric);
					*c += 1u32;
					*v = *v / FixedU128::from_u32(*c);
				});
			});
		}

		fn commit_new_metrics(
			processor: &T::AccountId,
			manager_id: Option<T::ManagerId>,
			metrics: &[MetricInput],
			active: bool,
			cycle: CycleFor<T>,
		) {
			let epoch = cycle.epoch;

			for (pool_id, numerator, denominator) in metrics {
				let Some(metric) = FixedU128::checked_from_rational(
					*numerator,
					if denominator.is_zero() { One::one() } else { *denominator },
				) else {
					continue;
				};
				let before = Metrics::<T, I>::get(processor, pool_id);
				let (first_in_epoch, first_in_era) = before
					.map(|m| {
						(
							// first value committed for `epoch` wins
							m.epoch < epoch,
							// if this is NOT the first ever written average, then
							// we have to be careful to not count a metric twice into average by same processor in one era
							// this is somehow possible by checking if last epoch committed in `Metrics` was in current or previous era (! not epoch)
							// However, this is imprecise under changing T::Epoch and T::Era, but it's good enough since we just would reuse or skip some metric values in average
							// `first_block_of_prev_commit_epoch = epoch_start - (current_epoch - prev_metric_commit_epoch) * EPOCH`
							cycle.epoch_start.saturating_sub(
								(cycle.epoch.saturating_sub(m.epoch))
									.saturating_mul(T::Epoch::get()),
							) < cycle.era_start,
						)
					})
					.unwrap_or((true, true));
				if first_in_epoch {
					// insert even if not active for tracability before warmup ended
					Metrics::<T, I>::insert(processor, pool_id, MetricCommit { epoch, metric });

					if active {
						// sum totals
						<MetricPools<T, I>>::mutate(pool_id, |pool| {
							if let Some(pool) = pool.as_mut() {
								pool.add(epoch, metric);
							}
						});

						if first_in_era {
							if let Some(manager_id) = manager_id {
								Self::update_era_average(manager_id, pool_id, metric, cycle);
							}
						}
					}
				}
			}
		}

		fn reuse_metrics(
			processor: &T::AccountId,
			manager_id: Option<T::ManagerId>,
			active: bool,
			cycle: CycleFor<T>,
		) {
			let epoch = cycle.epoch;

			let mut to_update: Vec<(PoolId, MetricCommit<_>)> = Vec::new();
			for (pool_id, metric) in Metrics::<T, I>::iter_prefix(processor) {
				if epoch > metric.epoch && epoch - metric.epoch < T::MetricValidity::get() {
					to_update.push((pool_id, metric));
				}
			}
			for (pool_id, commit) in to_update {
				// if this is NOT the first ever written average, then
				// we have to be careful to not count a metric twice into average by same processor in one era
				// this is somehow possible by checking if last epoch committed in `Metrics` was in current or previous era (! not epoch)
				// However, this is imprecise under changing T::Epoch and T::Era, but it's good enough since we just would reuse or skip some metric values in average
				// `first_block_of_prev_commit_epoch = epoch_start - (current_epoch - prev_metric_commit_epoch) * EPOCH`
				let first_in_era = cycle.epoch_start.saturating_sub(
					(cycle.epoch.saturating_sub(commit.epoch)).saturating_mul(T::Epoch::get()),
				) < cycle.era_start;

				Metrics::<T, I>::insert(
					processor,
					pool_id,
					MetricCommit { epoch, metric: commit.metric },
				);

				if active {
					// sum totals
					<MetricPools<T, I>>::mutate(pool_id, |pool| {
						if let Some(pool) = pool.as_mut() {
							pool.add(epoch, commit.metric);
						}
					});

					if first_in_era {
						if let Some(manager_id) = manager_id {
							Self::update_era_average(manager_id, &pool_id, commit.metric, cycle);
						}
					}
				}
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
