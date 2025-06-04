#![cfg_attr(not(feature = "std"), no_std)]

use datastructures::*;
pub use pallet::*;
pub use traits::*;
pub use types::*;

pub(crate) use pallet::STORAGE_VERSION;

mod datastructures;
mod hooks;
mod migration;
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

#[frame_support::pallet]
pub mod pallet {
	use core::ops::{Div, Rem};

	use acurast_common::{ManagerIdProvider, PoolId};
	use frame_support::{
		dispatch::DispatchResultWithPostInfo,
		pallet_prelude::*,
		traits::{tokens::Balance, Currency, Get, InspectLockableCurrency, LockableCurrency},
		Parameter,
	};
	use frame_system::{
		ensure_root,
		pallet_prelude::{BlockNumberFor, OriginFor},
	};
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
		type ManagerId: Member + MaxEncodedLen + Copy + CheckedAdd + From<u128>;
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

	pub(crate) const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

	#[pallet::pallet]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T, I = ()>(PhantomData<(T, I)>);

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config<I>, I: 'static = ()> {
		/// PoolCreated. [pool_id, pool_state]
		PoolCreated(PoolId, MetricPoolFor<T, I>),
		/// A processor committed it's compute. [processor_account_id, processor_status]
		Committed(T::AccountId, ProcessorStatusFor<T, I>),
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

			let (epoch, active) = Processors::<T, I>::mutate(processor, |p_| {
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

				// The global epoch number
				let epoch = current_block.saturating_sub(T::EpochBase::get()) / T::Epoch::get();

				p.committed = epoch;
				(epoch, matches!(p.status, ProcessorStatus::Active))
			});

			for (pool_id, numerator, denominator) in metrics {
				let metric = FixedU128::from_rational(
					numerator,
					if denominator.is_zero() { One::one() } else { denominator },
				);
				let before = Metrics::<T, I>::get(processor, pool_id);
				// first value committed for `epoch` wins
				if before.map(|m| m.epoch < epoch).unwrap_or(true) {
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
				}
			}
		}
	}
}
