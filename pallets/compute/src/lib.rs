#![cfg_attr(not(feature = "std"), no_std)]

pub use datastructures::*;
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

pub type EpochOf<T, I> = <T as Config<I>>::BlockNumber;
pub type EraOf<T, I> = <T as Config<I>>::BlockNumber;

#[frame_support::pallet]
pub mod pallet {
	use core::ops::{Div, Rem};

	use acurast_common::{ManagerIdProvider, ManagerProvider, PoolId};
	use frame_support::{
		dispatch::DispatchResultWithPostInfo,
		pallet_prelude::*,
		traits::{
			tokens::Balance, Currency, Get, InspectLockableCurrency, LockIdentifier,
			LockableCurrency,
		},
		Parameter,
	};
	use frame_system::pallet_prelude::*;
	use frame_system::{
		ensure_root,
		pallet_prelude::{BlockNumberFor, OriginFor},
	};
	use sp_runtime::SaturatedConversion;
	use sp_runtime::{
		codec::{Codec, MaxEncodedLen},
		traits::{CheckedAdd, CheckedSub, One, Saturating, Zero},
		FixedU128, Perquintill,
	};
	use sp_std::cmp::max;
	use sp_std::prelude::*;

	use crate::{datastructures::ProvisionalBuffer, *};

	/// Configure the pallet by specifying the parameters and types on which it depends.
	#[pallet::config]
	pub trait Config<I: 'static = ()>: frame_system::Config {
		/// Because this pallet emits events, it depends on the runtime's definition of an event.
		type RuntimeEvent: From<Event<Self, I>>
			+ IsType<<Self as frame_system::Config>::RuntimeEvent>;
		type ManagerId: Member
			+ Parameter
			+ MaxEncodedLen
			+ MaybeSerializeDeserialize
			+ Copy
			+ CheckedAdd
			+ From<u128>;
		type ManagerProvider: ManagerProvider<Self::AccountId>;
		type ManagerIdProvider: ManagerIdProvider<Self::AccountId, Self::ManagerId>;
		/// How long an epoch lasts, which describes the length of the commit-reward cycle length for active processors.
		///
		/// Must be longer than a heartbeat interval and better include multiple heartbeats so there is a chance of recovery if one heartbeat is missed.
		///
		/// NOTE: if you change this value over runtime upgrade we need to make sure the old value is kept until a block `b` that fulfills
		/// - `b` is a future block **after** runtime upgrade happened
		/// - `EpochBase` is set to this value `b`.
		/// This is necessary so the epoch numbering is continuous, which is achieved by calculating current epoch based on [`Self::EpochBase`] as `(current_block - T::EpochBase::get()) / T::Epoch::get()`.
		#[pallet::constant]
		type Epoch: Get<EpochOf<Self, I>>;
		/// The first block to calculate with this epoch, can start with `0`.
		#[pallet::constant]
		type EpochBase: Get<EpochOf<Self, I>>;
		/// How long a era lasts, which describes the length of the reward cycle for staked-backed and unbacked compute rewards corresponding manager commitments.
		///
		/// This is the longest, overarching cycle on which the reward system operates.
		///
		/// It is also the length of the metric history (average/max) kept.
		#[pallet::constant]
		type Era: Get<EraOf<Self, I>>;
		#[pallet::constant]
		type MaxPools: Get<u32>;
		/// The maximum number of active delegations a manager can simultaneously have. Accepting further delegation offers will be blocked until someone enters cooldown.
		#[pallet::constant]
		type MaxMetricCommitmentRatio: Get<Perquintill>;
		/// The minimum cooldown period for delegators in number of blocks.
		#[pallet::constant]
		type MinCooldownPeriod: Get<<Self as Config<I>>::BlockNumber>;
		/// The maximum cooldown period for delegators in number of blocks. Delegator's weight is linear as [`Stake`]`::cooldown_period / MaxCooldownPeriod`.
		#[pallet::constant]
		type MaxCooldownPeriod: Get<<Self as Config<I>>::BlockNumber>;
		/// The minimum possible delegated amount to a manager. There is no maximum for the amount which a delegator can offer, but it's still limited by the [`Self::MaxDelegationRatio`].
		#[pallet::constant]
		type MinDelegation: Get<Self::Balance>;
		/// The maximum ratio of active (not-in-cooldown) delegations that a compute provider can have.
		#[pallet::constant]
		type MaxDelegationRatio: Get<Perquintill>;
		/// The minimum stake by a compute provider (manager).
		#[pallet::constant]
		type MinStake: Get<Self::Balance>;
		/// How long a processor needs to warm up before his metrics are respected for compute score and reward calculation.
		#[pallet::constant]
		type WarmupPeriod: Get<<Self as Config<I>>::BlockNumber>;
		type Balance: Parameter
			+ IsType<u128>
			+ Div
			+ Balance
			+ Zero
			+ MaybeSerializeDeserialize
			+ IsType<<<Self as Config<I>>::Currency as Currency<Self::AccountId>>::Balance>;
		type BlockNumber: Parameter
			+ Codec
			+ MaxEncodedLen
			+ Ord
			+ Rem<Output = Self::BlockNumber>
			+ Div<Output = Self::BlockNumber>
			+ CheckedAdd
			+ CheckedSub
			+ Saturating
			+ One
			+ Zero
			+ Copy
			+ Into<u128>
			+ IsType<BlockNumberFor<Self>>
			+ MaybeSerializeDeserialize;
		type Currency: LockableCurrency<Self::AccountId, Moment = BlockNumberFor<Self>>
			+ InspectLockableCurrency<Self::AccountId>;
		/// The single lock indentifier used for the sum of all staked and delegated amounts.
		///
		/// We have to use the same lock identifier since we do not want the locks to overlap;
		/// Eventhough a staker can also delegate at the same time, the same funds contributing to total balance of an account is either delegated or staked, not both.
		#[pallet::constant]
		type LockIdentifier: Get<LockIdentifier>;
		type ComputeRewardDistributor: ComputeRewardDistributor<Self, I>;
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
	impl<T: Config<I>, I: 'static> BuildGenesisConfig for GenesisConfig<T, I> {
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

	/// Storage for pools' config and current total value over all active processors.
	#[pallet::storage]
	#[pallet::getter(fn metric_pools)]
	pub type MetricPools<T: Config<I>, I: 'static = ()> =
		StorageMap<_, Identity, PoolId, MetricPoolFor<T, I>>;

	/// The pool members, active and in warmup status.
	#[pallet::storage]
	#[pallet::getter(fn metrics)]
	pub(super) type Metrics<T: Config<I>, I: 'static = ()> =
		StorageDoubleMap<_, Twox64Concat, T::AccountId, Identity, PoolId, MetricCommitFor<T, I>>;

	/// The pool members, active and in warmup status.
	#[pallet::storage]
	#[pallet::getter(fn metric_pool_lookup)]
	pub(super) type MetricPoolLookup<T: Config<I>, I: 'static = ()> =
		StorageMap<_, Identity, MetricPoolName, PoolId>;

	/// The commitments of compute given by managers. They are limited by a ratio of what was measured as max in last completed era.
	#[pallet::storage]
	#[pallet::getter(fn compute_commitments)]
	pub(super) type ComputeCommitments<T: Config<I>, I: 'static = ()> =
		StorageDoubleMap<_, Identity, T::ManagerId, Identity, PoolId, Metric>;

	/// The preferences set by managers.
	#[pallet::storage]
	#[pallet::getter(fn preferences)]
	pub(super) type Preferences<T: Config<I>, I: 'static = ()> =
		StorageMap<_, Identity, T::ManagerId, ManagerPreferences>;

	/// The measured metrics average over an era by pool and all of a manager's active devices as a map `manager -> pool -> sliding_buffer[block % T::Era -> (metric, avg_count)]`.
	#[pallet::storage]
	#[pallet::getter(fn metrics_era_average)]
	pub(super) type MetricsEraAverage<T: Config<I>, I: 'static = ()> = StorageDoubleMap<
		_,
		Identity,
		T::ManagerId,
		Identity,
		PoolId,
		SlidingBuffer<EraOf<T, I>, (Metric, u32)>,
	>;

	/// Delegations as a map `delegator` -> `manager_id` -> [`Stake`].
	#[pallet::storage]
	#[pallet::getter(fn delegations)]
	pub(super) type Delegations<T: Config<I>, I: 'static = ()> =
		StorageDoubleMap<_, Twox64Concat, T::AccountId, Identity, T::ManagerId, StakeFor<T, I>>;

	/// State by manager holding stake and related state.
	///
	/// They are stored by manager account (instead of manager_id) since NOT automatically transferred to new holder of manager_id when transferring manager_id NFT.
	#[pallet::storage]
	#[pallet::getter(fn stakes)]
	pub(super) type Stakes<T: Config<I>, I: 'static = ()> =
		StorageMap<_, Twox64Concat, T::AccountId, StakeFor<T, I>>;

	/// Storage for the total stake of all compute providers.
	#[pallet::storage]
	#[pallet::getter(fn total_stake)]
	pub type TotalStake<T: Config<I>, I: 'static = ()> = StorageValue<_, T::Balance, ValueQuery>;

	/// Tracks an account's delegation totals.
	#[pallet::storage]
	#[pallet::getter(fn delegator_totals)]
	pub(super) type DelegatorTotals<T: Config<I>, I: 'static = ()> =
		StorageMap<_, Twox64Concat, T::AccountId, T::Balance, ValueQuery>;

	/// Tracks a manager_ids delegation totals.
	///
	/// Delegatios are automatically transferred to a new holder of manager_id when transferring manager_id NFT.
	#[pallet::storage]
	#[pallet::getter(fn delegatee_totals)]
	pub(super) type DelegateeTotals<T: Config<I>, I: 'static = ()> =
		StorageMap<_, Twox64Concat, T::ManagerId, DelegateeTotalFor<T, I>, ValueQuery>;

	pub(crate) const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

	#[pallet::pallet]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T, I = ()>(PhantomData<(T, I)>);

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config<I>, I: 'static = ()> {
		/// PoolCreated. [pool_id, pool_state]
		PoolCreated(PoolId, MetricPoolFor<T, I>),
		/// An account started delegation to a manager. [delegator, manager_id]
		Delegated(T::AccountId, T::ManagerId),
		/// An account started the cooldown for an accepted delegation. [delegator, manager_id]
		DelegationCooldownStarted(T::AccountId, T::ManagerId),
		/// An account passed the cooldown and ended delegation. [delegator, manager_id]
		DelegationEnded(T::AccountId, T::ManagerId),
		/// A manager committed it's compute and staked to back his compute. [manager_id]
		ComputeCommitted(T::ManagerId),
		/// A manager increased it's stake. [manager_id, extra_amount]
		StakedMore(T::ManagerId, T::Balance),
		/// A manager started the cooldown for his commitment & stake. [manager_id]
		ComputeCommitmentCooldownStarted(T::ManagerId),
		/// A manager passed the cooldown for his commitment & stake. [manager_id]
		ComputeCommitmentEnded(T::ManagerId),
		/// A reward got distrubuted. [amount]
		Rewarded(T::Balance),
		/// A manager got slahsed. [manager_id, amount]
		Slashed(T::ManagerId, T::Balance),
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
		AlreadyDelegating,
		CooldownAlreadyStarted,
		BelowMinCooldownPeriod,
		AboveMaxCooldownPeriod,
		BelowMinDelegation,
		MaxDelegationRatioExceeded,
		MaxMetricCommitmentExceeded,
		CannotStakeLessOrEqual,
		MinStakeSubceeded,
		InsufficientBalance,
		CooldownNotStarted,
		CooldownNotEnded,
		NotDelegating,
		NotStaking,
		AlreadyCommitted,
		InternalErrorNotStaking,
		NoMetricsAverage,
	}

	#[pallet::hooks]
	impl<T: Config<I>, I: 'static> Hooks<BlockNumberFor<T>> for Pallet<T, I> {
		fn on_runtime_upgrade() -> frame_support::weights::Weight {
			crate::migration::migrate::<T, I>()
		}
	}

	#[pallet::call]
	impl<T: Config<I>, I: 'static> Pallet<T, I> {
		#[pallet::call_index(0)]
		#[pallet::weight(T::WeightInfo::create_pool(config.len() as u32))]
		pub fn create_pool(
			origin: OriginFor<T>,
			name: MetricPoolName,
			reward: Perquintill,
			config: MetricPoolConfigValues,
		) -> DispatchResultWithPostInfo {
			ensure_root(origin)?;

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
			new_reward_from_epoch: Option<(EpochOf<T, I>, Perquintill)>,
			new_config: Option<ModifyMetricPoolConfig>,
		) -> DispatchResultWithPostInfo {
			ensure_root(origin)?;

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

				let current_block = T::BlockNumber::from(<frame_system::Pallet<T>>::block_number());
				let current_epoch =
					(current_block.saturating_sub(T::EpochBase::get())) / T::Epoch::get();
				// we use current epoch - 1 to be sure no rewards are overwritten that still are used for calculations/claiming
				let previous_epoch = current_epoch.saturating_sub(One::one());

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

		#[pallet::call_index(2)]
		#[pallet::weight(T::WeightInfo::delegate())]
		pub fn delegate(
			origin: OriginFor<T>,
			manager: T::AccountId,
			amount: T::Balance,
			cooldown_period: T::BlockNumber,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			let manager_id = T::ManagerIdProvider::manager_id_for(&manager)?;

			ensure!(amount >= T::MinDelegation::get(), Error::<T, I>::BelowMinDelegation);
			ensure!(
				cooldown_period >= T::MinCooldownPeriod::get(),
				Error::<T, I>::BelowMinCooldownPeriod
			);
			ensure!(
				cooldown_period <= T::MaxCooldownPeriod::get(),
				Error::<T, I>::AboveMaxCooldownPeriod
			);
			ensure!(
				Self::delegation_ratio(&who, manager_id) >= T::MaxDelegationRatio::get(),
				Error::<T, I>::MaxDelegationRatioExceeded
			);

			Self::lock_funds(&who, amount, LockReason::Delegation(manager_id))?;

			let _ = T::ManagerIdProvider::owner_for(manager_id)?;

			Delegations::<T, I>::try_mutate::<_, _, _, Error<T, I>, _>(
				&who,
				manager_id,
				|stake| {
					if stake.is_some() {
						Err(Error::<T, I>::AlreadyDelegating)?;
					}
					*stake = Some(Stake {
						amount,
						accrued: Zero::zero(),
						cooldown_period,
						cooldown_started: None,
					});
					Ok(())
				},
			)?;

			Self::deposit_event(Event::<T, I>::Delegated(who, manager_id));

			Ok(().into())
		}

		#[pallet::call_index(3)]
		#[pallet::weight(T::WeightInfo::cooldown_delegation())]
		pub fn cooldown_delegation(
			origin: OriginFor<T>,
			manager: T::AccountId,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			let manager_id = T::ManagerIdProvider::manager_id_for(&manager)?;
			Self::cooldown_delegation_for(&who, manager_id)?;
			Self::deposit_event(Event::<T, I>::DelegationCooldownStarted(who, manager_id));
			Ok(().into())
		}

		#[pallet::call_index(4)]
		#[pallet::weight(T::WeightInfo::end_delegation())]
		pub fn end_delegation(
			origin: OriginFor<T>,
			manager: T::AccountId,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			let manager_id = T::ManagerIdProvider::manager_id_for(&manager)?;
			Self::end_delegation_for(&who, manager_id)?;
			Self::deposit_event(Event::<T, I>::DelegationEnded(who, manager_id));
			Ok(().into())
		}

		#[pallet::call_index(5)]
		#[pallet::weight(T::WeightInfo::commit_compute())]
		pub fn commit_compute(
			origin: OriginFor<T>,
			amount: T::Balance,
			cooldown_period: T::BlockNumber,
			commitment: BoundedVec<ComputeCommitment, <T as Config<I>>::MaxPools>,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;

			let manager_id = T::ManagerIdProvider::manager_id_for(&who)?;

			let first_stake = Self::stake_for(&who, amount)?;
			ensure!(first_stake, Error::<T, I>::AlreadyCommitted);

			let current_block = T::BlockNumber::from(<frame_system::Pallet<T>>::block_number());
			let era = current_block / T::Era::get();
			for c in commitment {
				let avg = <MetricsEraAverage<T, I>>::get(manager_id, c.pool_id)
					.ok_or(Error::<T, I>::NoMetricsAverage)?;
				let (avg_value, _) = avg.get(era);
				// TODO test if this is correct transformation with / 1_000u128
				ensure!(
					c.metric < avg_value
						&& Perquintill::from_parts(
							((c.metric / avg_value).into_inner() / 1_000u128) as u64
						) < T::MaxMetricCommitmentRatio::get(),
					Error::<T, I>::MaxMetricCommitmentExceeded
				);
			}

			Self::deposit_event(Event::<T, I>::ComputeCommitted(manager_id));

			Ok(().into())
		}

		#[pallet::call_index(6)]
		#[pallet::weight(T::WeightInfo::commit_compute())]
		pub fn stake_more(
			origin: OriginFor<T>,
			extra_amount: T::Balance,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;

			let manager_id = T::ManagerIdProvider::manager_id_for(&who)?;

			// we don't care if previously staked
			let _ = Self::stake_for(&who, extra_amount)?;

			Self::deposit_event(Event::<T, I>::StakedMore(manager_id, extra_amount));

			Ok(().into())
		}

		#[pallet::call_index(7)]
		#[pallet::weight(T::WeightInfo::cooldown_compute_commitment())]
		pub fn cooldown_compute_commitment(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			let manager_id = T::ManagerIdProvider::manager_id_for(&who)?;
			Self::cooldown_stake_for(&who)?;
			Self::deposit_event(Event::<T, I>::ComputeCommitmentCooldownStarted(manager_id));
			Ok(().into())
		}

		#[pallet::call_index(8)]
		#[pallet::weight(T::WeightInfo::end_compute_commitment())]
		pub fn end_compute_commitment(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			let manager_id = T::ManagerIdProvider::manager_id_for(&who)?;
			Self::unstake_for(&who)?;
			Self::deposit_event(Event::<T, I>::ComputeCommitmentEnded(manager_id));
			Ok(().into())
		}

		#[pallet::call_index(9)]
		#[pallet::weight(T::WeightInfo::reward())]
		pub fn reward(origin: OriginFor<T>, reward: T::Balance) -> DispatchResultWithPostInfo {
			ensure_root(origin)?;

			// TODO

			Self::deposit_event(Event::<T, I>::Rewarded(reward));

			Ok(Pays::No.into())
		}

		#[pallet::call_index(10)]
		#[pallet::weight(T::WeightInfo::slash())]
		pub fn slash(
			origin: OriginFor<T>,
			manager: T::AccountId,
			amount: T::Balance,
		) -> DispatchResultWithPostInfo {
			ensure_root(origin)?;

			let manager_id = T::ManagerIdProvider::manager_id_for(&manager)?;

			// TODO

			Self::deposit_event(Event::<T, I>::Slashed(manager_id, amount));

			Ok(Pays::No.into())
		}
	}

	impl<T: Config<I>, I: 'static> Pallet<T, I> {
		fn do_create_pool(
			name: MetricPoolName,
			reward_ratio: Perquintill,
			config: MetricPoolConfigValues,
		) -> Result<(PoolId, MetricPoolFor<T, I>), DispatchError> {
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
			};
			MetricPools::<T, I>::insert(&pool_id, &pool);
			<MetricPoolLookup<T, I>>::insert(name, pool_id);

			Ok((pool_id, pool))
		}

		pub(crate) fn do_claim(
			processor: &T::AccountId,
			pool_ids: Vec<PoolId>,
		) -> Result<Option<T::Balance>, Error<T, I>> {
			if let Some(claim_epoch) = Self::can_claim(&processor) {
				let mut p: ProcessorStateFor<T, I> = Processors::<T, I>::get(processor)
					.ok_or(Error::<T, I>::ProcessorNeverCommitted)?;

				let mut total_reward_ratio: Perquintill = Zero::zero();
				for pool_id in pool_ids {
					// we allow partial metrics committed (for backwards compatibility)
					let commit = if let Some(c) = Metrics::<T, I>::get(processor, pool_id) {
						c
					} else {
						continue;
					};

					// NOTE: we ensured previously in can_claim that p.committed is current processor's epoch - 1, so this validates recentness of individual metric commits
					if commit.epoch != p.committed {
						continue;
					}

					let pool =
						<MetricPools<T, I>>::get(pool_id).ok_or(Error::<T, I>::PoolNotFound)?;
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

				// if calculation step fail we swallow error (after logging)
				let reward =
					T::ComputeRewardDistributor::calculate_reward(total_reward_ratio, claim_epoch)
						.map_err(|e| {
							log::error!(
								target: LOG_TARGET,
								"Failed to calculate reward; skipping to not fail claim-commit. {:?}",
								e
							)
						})
						.ok()
						.unwrap_or(Zero::zero());

				// accrue
				match T::ComputeRewardDistributor::distribute_reward(
					&processor,
					reward.saturating_add(p.accrued),
				) {
					Ok(()) => {
						p.paid = p.paid.saturating_add(reward);
						p.accrued = Zero::zero();
					},
					Err(e) => {
						log::warn!(
							target: LOG_TARGET,
							"Failed to distribute reward; accrueing instead for later pay out. {:?}",
							e
						);
						// amount remains in accrued
						p.accrued = p.accrued.saturating_add(reward);
					},
				}

				p.claimed = claim_epoch;

				Processors::<T, I>::insert(processor, p);

				Ok(Some(reward))
			} else {
				Ok(None)
			}
		}

		fn can_claim(processor: &T::AccountId) -> Option<EpochOf<T, I>> {
			let current_block = T::BlockNumber::from(<frame_system::Pallet<T>>::block_number());

			if let Some(p) = Processors::<T, I>::get(processor) {
				let epoch = current_block.saturating_sub(T::EpochBase::get()) / T::Epoch::get();

				(p.committed.saturating_add(One::one()) == epoch
					&& p.claimed < p.committed
					&& match p.status {
						ProcessorStatus::WarmupUntil(b) => b <= current_block,
						ProcessorStatus::Active => true,
					})
				.then_some(p.committed)
			} else {
				None
			}
		}

		fn delegation_ratio(
			manager_account: &T::AccountId,
			manager_id: T::ManagerId,
		) -> Perquintill {
			let denominator: u128 = <DelegateeTotals<T, I>>::get(manager_id).amount.into();
			let nominator: u128 = <Stakes<T, I>>::get(manager_account)
				.map(|s| s.amount)
				.unwrap_or(T::Balance::zero())
				.into();
			if denominator > 0 {
				Perquintill::from_rational(nominator, denominator)
			} else {
				Perquintill::one()
			}
		}

		pub fn delegator_weight(state: &StakeFor<T, I>) -> T::Balance {
			(state
				.amount
				.saturated_into::<u128>()
				.saturating_mul(state.cooldown_period.into())
				/ T::MaxCooldownPeriod::get().saturated_into::<u128>())
			.into()
		}

		/// Helper to only commit compute for current processor epoch by providing benchmarked results for a (sub)set of metrics.
		///
		/// Metrics are specified with the `pool_id` and **unknown `pool_id`'s are silently skipped.**
		///
		/// See [`Self::commit`] for how this is used to commit metrics and claim for past commits.
		pub(crate) fn do_commit(
			processor: &T::AccountId,
			metrics: impl IntoIterator<Item = (PoolId, u128, u128)>,
		) {
			let current_block = T::BlockNumber::from(<frame_system::Pallet<T>>::block_number());

			// The global epoch number
			let epoch = current_block.saturating_sub(T::EpochBase::get()) / T::Epoch::get();

			let active = Processors::<T, I>::mutate(processor, |p_| {
				let p: &mut ProcessorState<_, _, _> = p_.get_or_insert_with(||
					// this is the very first commit so create a `ProcessorState` aligning with the current block -> individual epoch start to avoid congestion of claim calls
					ProcessorState {
                        // currently unused, see comment why we initialize this anyways
						epoch_offset: current_block.saturating_sub(T::EpochBase::get())
							% T::Epoch::get(),
						committed: Zero::zero(),
						claimed: Zero::zero(),
						status: ProcessorStatus::WarmupUntil(
                            current_block.saturating_add(<T as Config<I>>::WarmupPeriod::get()),
						),
                        accrued: Zero::zero(),
                        paid: Zero::zero(),
					});

				// check if warmup passed and updated status if so
				if let ProcessorStatus::WarmupUntil(b) = p.status {
					if current_block >= b {
						p.status = ProcessorStatus::Active;
					}
				}

				p.committed = epoch;
				matches!(p.status, ProcessorStatus::Active)
			});

			let era = current_block / T::Era::get();

			for (pool_id, numerator, denominator) in metrics {
				let metric = FixedU128::from_rational(
					numerator,
					if denominator.is_zero() { One::one() } else { denominator },
				);
				let before = Metrics::<T, I>::get(processor, pool_id);
				// first value committed for `epoch` wins
				if before.map(|m| m.epoch < epoch).unwrap_or(true) {
					let prev_metric_commit = Metrics::<T, I>::get(processor, pool_id);

					// insert even if not active for tracability before warmup ended
					Metrics::<T, I>::insert(processor, pool_id, MetricCommit { epoch, metric });

					if active {
						// sum totals
						<MetricPools<T, I>>::mutate(pool_id, |pool| {
							if let Some(pool) = pool.as_mut() {
								pool.add(epoch, metric);
							}
						});
					}

					// update moving average over all processors of a manager for era (not epoch!)
					// we have to be careful to not count a metric twice into average by same processor in one era
					// this is possible by checking if last epoch committed in `Metrics` was in current or previous era (! not epoch)
					let update_average = if let Some(c) = prev_metric_commit {
						let first_block_of_prev_commit_epoch = c
							.epoch
							.saturating_mul(T::Epoch::get())
							.saturating_add(T::EpochBase::get());
						if first_block_of_prev_commit_epoch / T::Era::get() != era {
							true
						} else {
							false
						}
					} else {
						false
					};

					if update_average {
						if let Ok(manager) = T::ManagerProvider::manager_of(processor) {
							if let Ok(manager_id) = T::ManagerIdProvider::manager_id_for(&manager) {
								<MetricsEraAverage<T, I>>::mutate(manager_id, pool_id, |avg_| {
									if avg_.is_none() {
										// creates the buffer with defaults, so 0 average and 0 avg_count
										*avg_ = Some(SlidingBuffer::new(Zero::zero()));
									}

									let avg = avg_.as_mut().unwrap();
									avg.mutate(epoch, |(v, c)| {
										*v = v.saturating_mul(FixedU128::from_u32(*c));
										*v = v.saturating_add(metric);
										*c += 1u32;
										*v = *v / FixedU128::from_u32(*c);
									});
								});
							}
						}
					}
				}
			}
		}
	}
}
