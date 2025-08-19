#![cfg_attr(not(feature = "std"), no_std)]

pub use datastructures::*;
pub use pallet::*;
pub use reward::*;
pub use traits::*;
pub use types::*;

pub(crate) use pallet::STORAGE_VERSION;

mod datastructures;
mod hooks;
mod migration;
mod reward;
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

	use acurast_common::{CommitmentIdProvider, ManagerIdProvider, ManagerProvider, PoolId};
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
	use sp_runtime::{
		codec::{Codec, MaxEncodedLen},
		traits::{CheckedAdd, CheckedSub, One, Saturating, Zero},
		FixedU128, Perbill, Perquintill,
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
		type CommitmentId: Member
			+ Parameter
			+ MaxEncodedLen
			+ MaybeSerializeDeserialize
			+ Copy
			+ CheckedAdd
			+ Default
			+ From<u128>;
		type ManagerProvider: ManagerProvider<Self::AccountId>;
		type ManagerIdProvider: ManagerIdProvider<Self::AccountId, Self::ManagerId>;
		type CommitmentIdProvider: CommitmentIdProvider<Self::AccountId, Self::CommitmentId>;
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
		/// Defines the duration of an era, which represents the reward cycle for both stake-backed and unbacked compute rewards associated with commitments.
		///
		/// This is the longest, overarching cycle on which the reward system operates.
		///
		/// It also determines the length of the metric history (average and maximum) that is retained.
		#[pallet::constant]
		type Era: Get<EraOf<Self, I>>;
		#[pallet::constant]
		type MaxPools: Get<u32>;
		/// The maximum ratio `committed_metric / last_era_metric_averate`. This ratio limit is enforced separately by metric pool
		#[pallet::constant]
		type MaxMetricCommitmentRatio: Get<Perquintill>;
		/// The minimum cooldown period for delegators in number of blocks.
		#[pallet::constant]
		type MinCooldownPeriod: Get<<Self as Config<I>>::BlockNumber>;
		/// The maximum cooldown period for delegators in number of blocks. Delegator's weight is linear as [`Stake`]`::cooldown_period / MaxCooldownPeriod`.
		#[pallet::constant]
		type MaxCooldownPeriod: Get<<Self as Config<I>>::BlockNumber>;
		/// The minimum possible delegated amount towards a commitment. There is no maximum for the amount which a delegator can offer, but it's still limited by the [`Self::MaxDelegationRatio`].
		#[pallet::constant]
		type MinDelegation: Get<Self::Balance>;
		/// The maximum ratio `delegated_stake / commitment_total_stake = delegated_stake / (delegated_stake + committer_stake)`.
		#[pallet::constant]
		type MaxDelegationRatio: Get<Perquintill>;
		/// The minimum stake by a committer.
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
		type Decimals: Get<Self::Balance>;
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

	/// The measured metrics average over an era by pool and all of a manager's active devices as a map `manager_id` -> `pool_id` -> `sliding_buffer[block % T::Era -> (metric, avg_count)]`.
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

	/// Pool member state for commitments in their own delegation pool as a map `commitment_id` -> [`PoolMember`].
	#[pallet::storage]
	#[pallet::getter(fn self_delegation)]
	pub(super) type SelfDelegation<T: Config<I>, I: 'static = ()> =
		StorageMap<_, Identity, T::CommitmentId, PoolMemberFor<T, I>>;

	/// Pool member state as a map `commitment_id` -> `pool_id` -> [`PoolMember`].
	#[pallet::storage]
	#[pallet::getter(fn staking_pool_members)]
	pub(super) type StakingPoolMembers<T: Config<I>, I: 'static = ()> =
		StorageDoubleMap<_, Identity, T::CommitmentId, Identity, PoolId, PoolMemberFor<T, I>>;

	/// Tracks a commitment's backing stake, inclusive committer stake and delegated stakes received.
	#[pallet::storage]
	#[pallet::getter(fn commitment_stake)]
	pub(super) type CommitmentStake<T: Config<I>, I: 'static = ()> =
		StorageMap<_, Identity, T::CommitmentId, T::Balance, ValueQuery>;

	/// Tracks the total stake of all commitments.
	#[pallet::storage]
	#[pallet::getter(fn total_stake)]
	pub type TotalStake<T: Config<I>, I: 'static = ()> = StorageValue<_, T::Balance, ValueQuery>;

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
		PoolMemberFor<T, I>,
	>;

	/// Tracks a delegator's total delegated stake.
	#[pallet::storage]
	#[pallet::getter(fn delegator_total)]
	pub(super) type DelegatorTotal<T: Config<I>, I: 'static = ()> =
		StorageMap<_, Twox64Concat, T::AccountId, T::Balance, ValueQuery>;

	/// Tracks the total delegated stake by all delegators.
	#[pallet::storage]
	#[pallet::getter(fn total_delegated)]
	pub type TotalDelegated<T: Config<I>, I: 'static = ()> =
		StorageValue<_, T::Balance, ValueQuery>;

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
		StorageMap<_, Identity, T::CommitmentId, StakingPoolFor<T, I>, ValueQuery>;

	pub(crate) const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

	#[pallet::pallet]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T, I = ()>(PhantomData<(T, I)>);

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config<I>, I: 'static = ()> {
		/// PoolCreated. [pool_id, pool_state]
		PoolCreated(PoolId, MetricPoolFor<T, I>),
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
		/// An account started the cooldown for an accepted delegation. [delegator, commitment_id]
		DelegationCooldownStarted(T::AccountId, T::CommitmentId),
		/// An account passed the cooldown and ended delegation. [delegator, commitment_id]
		DelegationEnded(T::AccountId, T::CommitmentId),
		/// A committer staked and committed compute provided by the manager he is backing. [commitment_id]
		ComputeCommitted(T::CommitmentId),
		/// A committer increased it's stake. [commitment_id, extra_amount]
		StakedMore(T::CommitmentId, T::Balance),
		/// The cooldown for a commitment got started. [commitment_id]
		ComputeCommitmentCooldownStarted(T::CommitmentId),
		/// The cooldown for a commitment has ended. [commitment_id]
		ComputeCommitmentEnded(T::CommitmentId),
		/// A reward got distrubuted. [amount]
		Rewarded(T::Balance),
		/// A commitment got slahsed. [commitment_id, amount]
		Slashed(T::CommitmentId, T::Balance),
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
		MinStakeSubceeded,
		InsufficientBalance,
		CooldownNotStarted,
		CooldownNotEnded,
		NotDelegating,
		NotStaking,
		AlreadyCommitted,
		NoMetricsAverage,
		NoManagerBackingCommitment,
		NoOwnerOfCommitmentId,
		InternalError,
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

		/// Offers backing to a manager.
		///
		/// An account can only have one outstanding offer and only offer if not already owner of a `commitment`.
		///
		/// NOTE: Only upon acceptance, the offering account will receive a new `commitment_id`. This makes the invariant hold that behind each `commitment_id` (commitment-position/commitment-NFT) there is a commitment backing a manager.
		#[pallet::call_index(2)]
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

		#[pallet::call_index(3)]
		#[pallet::weight(T::WeightInfo::withdraw_backing_offer())]
		pub fn withdraw_backing_offer(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;

			let manager_id =
				BackingOffers::<T, I>::take(&who).ok_or(Error::<T, I>::NoBackingOfferFound)?;

			Self::deposit_event(Event::<T, I>::BackingOfferWithdrew(who, manager_id));

			Ok(().into())
		}

		#[pallet::call_index(4)]
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
		#[pallet::call_index(5)]
		#[pallet::weight(T::WeightInfo::commit_compute())]
		pub fn commit_compute(
			origin: OriginFor<T>,
			amount: T::Balance,
			cooldown_period: T::BlockNumber,
			commitment: BoundedVec<ComputeCommitment, <T as Config<I>>::MaxPools>,
			commission: Perbill,
			allow_auto_compound: bool,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;

			let commitment_id = T::CommitmentIdProvider::commitment_id_for(&who)?;
			let manager_id = <Backings<T, I>>::get(commitment_id)
				.ok_or(Error::<T, I>::NoManagerBackingCommitment)?;
			let current_block = T::BlockNumber::from(<frame_system::Pallet<T>>::block_number());
			let era = current_block / T::Era::get();
			for c in commitment {
				let avg = <MetricsEraAverage<T, I>>::get(manager_id, c.pool_id)
					.ok_or(Error::<T, I>::NoMetricsAverage)?;
				let (avg_value, _) = avg.get(era);
				ensure!(
					c.metric < avg_value
						&& Perquintill::from_parts(((c.metric / avg_value).into_inner()) as u64)
							< T::MaxMetricCommitmentRatio::get(),
					Error::<T, I>::MaxMetricCommitmentExceeded
				);

				ComputeCommitments::<T, I>::insert(&commitment_id, &c.pool_id, c.metric);
			}
			// commission can only be inserted once on commit, since stake_for below errors if already committe
			Commission::<T, I>::insert(&commitment_id, commission);

			// only call this AFTER storing commitment since it's a requirement for stake_for
			Self::stake_for(&who, amount, cooldown_period, allow_auto_compound)?;

			Self::deposit_event(Event::<T, I>::ComputeCommitted(commitment_id));

			Ok(().into())
		}

		#[pallet::call_index(6)]
		#[pallet::weight(T::WeightInfo::stake_more())]
		pub fn stake_more(
			origin: OriginFor<T>,
			extra_amount: T::Balance,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;

			let commitment_id = T::CommitmentIdProvider::commitment_id_for(&who)?;

			Self::stake_more_for(&who, extra_amount)?;

			Self::deposit_event(Event::<T, I>::StakedMore(commitment_id, extra_amount));

			Ok(().into())
		}

		#[pallet::call_index(7)]
		#[pallet::weight(T::WeightInfo::cooldown_compute_commitment())]
		pub fn cooldown_compute_commitment(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			let commitment_id = T::CommitmentIdProvider::commitment_id_for(&who)?;
			Self::cooldown_stake_for(commitment_id)?;
			Self::deposit_event(Event::<T, I>::ComputeCommitmentCooldownStarted(commitment_id));
			Ok(().into())
		}

		#[pallet::call_index(8)]
		#[pallet::weight(T::WeightInfo::end_compute_commitment())]
		pub fn end_compute_commitment(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			// the commitment_id does not get destroyed and might be recycled with upcoming features
			let commitment_id = T::CommitmentIdProvider::commitment_id_for(&who)?;
			Self::unstake_for(who, commitment_id)?;
			Commission::<T, I>::remove(commitment_id);
			Self::deposit_event(Event::<T, I>::ComputeCommitmentEnded(commitment_id));
			Ok(().into())
		}

		#[pallet::call_index(9)]
		#[pallet::weight(T::WeightInfo::delegate())]
		pub fn delegate(
			origin: OriginFor<T>,
			committer: T::AccountId,
			amount: T::Balance,
			cooldown_period: T::BlockNumber,
			allow_auto_compound: bool,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			let commitment_id = T::CommitmentIdProvider::commitment_id_for(&committer)?;

			Self::delegate_for(&who, commitment_id, amount, cooldown_period, allow_auto_compound)?;

			Self::deposit_event(Event::<T, I>::Delegated(who, commitment_id));

			Ok(().into())
		}

		#[pallet::call_index(10)]
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

		#[pallet::call_index(11)]
		#[pallet::weight(T::WeightInfo::end_delegation())]
		pub fn end_delegation(
			origin: OriginFor<T>,
			manager: T::AccountId,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			let commitment_id = T::CommitmentIdProvider::commitment_id_for(&manager)?;
			let _reward = Self::end_delegation_for(&who, commitment_id)?;
			// todo transfer reward

			Self::deposit_event(Event::<T, I>::DelegationEnded(who, commitment_id));
			Ok(().into())
		}

		#[pallet::call_index(12)]
		#[pallet::weight(T::WeightInfo::reward())]
		pub fn reward(origin: OriginFor<T>, amount: T::Balance) -> DispatchResultWithPostInfo {
			ensure_root(origin)?;

			let current_block = T::BlockNumber::from(<frame_system::Pallet<T>>::block_number());
			let current_era = current_block % T::Era::get();
			Self::distribute(current_era, amount)?;

			Self::deposit_event(Event::<T, I>::Rewarded(amount));

			Ok(Pays::No.into())
		}

		#[pallet::call_index(13)]
		#[pallet::weight(T::WeightInfo::slash())]
		pub fn slash(
			origin: OriginFor<T>,
			manager: T::AccountId,
			amount: T::Balance,
		) -> DispatchResultWithPostInfo {
			ensure_root(origin)?;

			let commitment_id = T::CommitmentIdProvider::commitment_id_for(&manager)?;
			Self::slash_delegation_pool(commitment_id, amount)?;

			Self::deposit_event(Event::<T, I>::Slashed(commitment_id, amount));

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
				// TODO here we want to distribute to metric pools instead
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
