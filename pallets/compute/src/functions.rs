use acurast_common::{CommitmentIdProvider, MetricInput, PoolId};
use frame_support::{
	dispatch::DispatchResult,
	traits::{fungible::Balanced, Currency, ExistenceRequirement, Get, Imbalance, IsType},
};
use frame_system::pallet_prelude::BlockNumberFor;
use sp_core::U256;
use sp_runtime::{
	traits::{AccountIdConversion, CheckedAdd, One, Zero},
	DispatchError, FixedPointNumber, FixedU128, Perquintill, SaturatedConversion, Saturating,
	Weight,
};
use sp_std::prelude::*;

use crate::{
	BalanceFor, BlockAuthorProvider, CollatorRewards, CommitMetricsInfo, Commitments,
	ComputeBasedRewards, Config, CurrentCycle, CycleFor, EpochOf, Error, InflationEnabled,
	InflationInfo, InflationInfoFor, LastMetricPoolId, Metric, MetricCommit, MetricPool,
	MetricPoolConfigValues, MetricPoolFor, MetricPoolLookup, MetricPoolName, MetricPoolUpdateInfo,
	MetricPools, Metrics, MetricsEpochSum, NextCommitmentId, Pallet, ProcessorState,
	ProcessorStatus, Processors, ProvisionalBuffer, RewardBudget, RewardContributionProvider,
	RewardInfo, SlidingBuffer, StakeBasedRewards, PER_TOKEN_DECIMALS,
};

impl<T: Config<I>, I: 'static> Pallet<T, I> {
	pub fn account_id() -> T::AccountId {
		T::PalletId::get().into_account_truncating()
	}

	pub fn advance_epoch(block_number: BlockNumberFor<T>) -> (EpochOf<T>, EpochOf<T>) {
		CurrentCycle::<T, I>::mutate(|cycle| {
			// Increment the sequential epoch
			let last_epoch = cycle.epoch;
			cycle.epoch = cycle.epoch.saturating_add(One::one());
			cycle.epoch_start = block_number;
			(last_epoch, cycle.epoch)
		})
	}

	pub fn inflate() -> InflationInfoFor<T, I> {
		let enabled = InflationEnabled::<T, I>::get();

		if !enabled {
			return InflationInfo::default();
		}

		let inflation_amount: BalanceFor<T, I> = T::InflationPerEpoch::get();

		if inflation_amount.is_zero() {
			return InflationInfo::default();
		}

		// Mint new tokens into distribution account
		let mut imbalance = <T::Currency as Balanced<T::AccountId>>::issue(inflation_amount);

		// Calculate split of stake-based and compute-based amount
		let inflation_amount = inflation_amount.saturated_into::<u128>();
		let stake_backed_amount: BalanceFor<T, I> = T::InflationStakedComputeRatio::get()
			.mul_floor(inflation_amount)
			.saturated_into();
		let compute_based_amount =
			T::InflationMetricsRatio::get().mul_floor(inflation_amount).saturated_into();
		let collators_amount =
			T::InflationCollatorsRatio::get().mul_floor(inflation_amount).saturated_into();
		let compute_imbalance = imbalance.extract(
			stake_backed_amount
				.saturating_add(compute_based_amount)
				.saturating_add(collators_amount),
		);
		let resolve_result = T::Currency::resolve(&Self::account_id(), compute_imbalance);
		if let Err(credit) = resolve_result {
			imbalance = imbalance.merge(credit);
		}

		InflationInfo {
			metrics_reward: compute_based_amount,
			staked_compute_reward: stake_backed_amount,
			collators_reward: collators_amount,
			credit: Some(imbalance),
		}
	}

	pub fn store_metrics_reward(amount: BalanceFor<T, I>, current_epoch: EpochOf<T>) -> Weight {
		let mut weight = Weight::default();
		if !amount.is_zero() {
			ComputeBasedRewards::<T, I>::mutate(|r| {
				r.mutate(
					current_epoch,
					|v| {
						*v = amount;
					},
					false,
				);
			});
			weight = weight.saturating_add(T::DbWeight::get().reads_writes(1, 1));
		}
		weight
	}

	pub fn store_collators_reward(amount: BalanceFor<T, I>) -> Weight {
		let mut weight = Weight::default();

		<CollatorRewards<T, I>>::set(amount);
		weight = weight.saturating_add(T::DbWeight::get().writes(1));

		weight
	}

	pub fn validate_metric_pools(
		pools: &[MetricPoolFor<T>],
		current_epoch: EpochOf<T>,
	) -> DispatchResult {
		let mut epochs_to_test = vec![current_epoch];
		epochs_to_test
			.extend(pools.iter().filter_map(|pool| pool.reward.next_epoch(current_epoch)));
		epochs_to_test.sort_unstable();
		epochs_to_test.dedup();
		for epoch in epochs_to_test {
			let mut reward_sum: Perquintill = Perquintill::zero();
			for metric_pool in pools {
				let reward = metric_pool.reward.get(epoch);
				reward_sum = reward_sum
					.checked_add(&reward)
					.ok_or(Error::<T, I>::InvalidTotalPoolRewards)?;
			}
		}

		Ok(())
	}
}

impl<T: Config<I>, I: 'static> Pallet<T, I>
where
	BalanceFor<T, I>: From<u128>,
{
	pub fn store_staked_compute_reward(
		amount: BalanceFor<T, I>,
		current_epoch: EpochOf<T>,
		last_epoch: EpochOf<T>,
		target_token_supply: u128,
	) -> Weight {
		// Store stake-based rewards split by pool (to avoid redoing this in every stake-based-reward-claiming heartbeat)
		let mut weight = Weight::default();

		for (pool_id, pool) in MetricPools::<T, I>::iter() {
			// we have to use the pool's total compute from the last_epoch since only this one is a completed rolling sum
			let target_weight_per_compute = U256::from(target_token_supply)
				.saturating_mul(U256::from(PER_TOKEN_DECIMALS))
				.checked_div(U256::from(pool.total.get(last_epoch).into_inner()))
				.unwrap_or(Zero::zero())
				.saturating_mul(U256::from(
					T::TargetWeightPerComputeMultiplier::get().into_inner(),
				));
			StakeBasedRewards::<T, I>::mutate(pool_id, |r| {
				r.mutate(
					current_epoch,
					|v| {
						*v = RewardBudget::new(
							pool.reward
								.get(last_epoch)
								.mul_floor(amount.saturated_into::<u128>())
								.into(),
							target_weight_per_compute,
						);
					},
					false,
				);
			});
			weight = weight.saturating_add(T::DbWeight::get().reads_writes(2, 1));
		}

		weight
	}

	pub fn reward_collator(maybe_amount: Option<BalanceFor<T, I>>) -> Weight {
		let mut weight = Weight::default();

		let collators_reward = maybe_amount.unwrap_or_else(|| {
			weight = weight.saturating_add(T::DbWeight::get().reads(1));

			<CollatorRewards<T, I>>::get()
		});

		if !collators_reward.is_zero() {
			let epoch_length: BlockNumberFor<T> = T::Epoch::get();
			let collators_reward: u128 = collators_reward.saturated_into();
			let collators_reward_per_block: BalanceFor<T, I> =
				collators_reward.saturating_div(epoch_length.saturated_into()).into();
			weight = weight.saturating_add(T::DbWeight::get().reads(1));
			if let Some(author) = T::AuthorProvider::author() {
				_ = T::Currency::transfer(
					&Self::account_id(),
					&author,
					collators_reward_per_block,
					ExistenceRequirement::KeepAlive,
				);
				weight = weight.saturating_add(T::DbWeight::get().writes(1));
			}
		}

		weight
	}

	pub fn do_create_pool(
		name: MetricPoolName,
		reward_ratio: Perquintill,
		config: MetricPoolConfigValues,
	) -> Result<(PoolId, MetricPoolFor<T>), DispatchError> {
		let mut current_pools = MetricPools::<T, I>::iter().map(|(_, v)| v).collect::<Vec<_>>();
		let current_pools_count = current_pools.len().saturated_into::<u32>();
		if current_pools_count >= T::MaxPools::get() {
			return Err(Error::<T, I>::CannotCreatePool)?;
		}
		if Self::metric_pool_lookup(name).is_some() {
			return Err(Error::<T, I>::PoolNameMustBeUnique)?;
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

		current_pools.push(pool.clone());
		let current_epoch = Self::current_cycle().epoch;
		Self::validate_metric_pools(current_pools.as_slice(), current_epoch)?;

		Ok((pool_id, pool))
	}

	pub(crate) fn do_claim(
		claim_epoch: EpochOf<T>,
		claim_epoch_metric_sums: &[(PoolId, (Metric, Metric))],
		total_reward: BalanceFor<T, I>,
	) -> Result<BalanceFor<T, I>, Error<T, I>>
	where
		BalanceFor<T, I>: IsType<u128>,
	{
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

			total_reward_ratio = total_reward_ratio.saturating_add(compute_weighted_reward_ratio);
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
	) -> RewardInfo<BalanceFor<T, I>>
	where
		BalanceFor<T, I>: IsType<u128>,
	{
		let current_block = <frame_system::Pallet<T>>::block_number();

		Processors::<T, I>::mutate(processor, |p_| {
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
			let active = matches!(p.status, ProcessorStatus::Active);

			let manager_id = manager.1;
			let commit_metrics_info = if !metrics.is_empty() {
				Self::commit_new_metrics(processor, manager_id, metrics, active, cycle)
			} else {
				Self::reuse_metrics(processor, manager_id, active, cycle)
			};

			if !active {
				return RewardInfo::default();
			}

			let total_metric_reward =
				Self::compute_based_rewards().get(cycle.epoch.saturating_sub(One::one()));
			if let Some(reward_contribution) =
				commit_metrics_info.compute_reward_contribution(total_metric_reward)
			{
				p.reward_contribution = reward_contribution;
			}

			let Some(previous_epoch_metric_sums) = commit_metrics_info.previous_sums else {
				return RewardInfo::default();
			};

			Self::reward(
				previous_epoch_metric_sums.as_slice(),
				cycle,
				manager_id,
				pool_ids,
				total_metric_reward,
			)
		})
	}

	fn reward(
		previous_epoch_metric_sums: &[(PoolId, (Metric, Metric))],
		cycle: CycleFor<T>,
		manager_id: T::ManagerId,
		pool_ids: &[PoolId],
		total_metric_reward: BalanceFor<T, I>,
	) -> RewardInfo<BalanceFor<T, I>>
	where
		BalanceFor<T, I>: IsType<u128>,
	{
		let mut reward: BalanceFor<T, I> = Zero::zero();
		let last_epoch = cycle.epoch.saturating_sub(One::one());
		let metric_rewards =
			Self::do_claim(last_epoch, previous_epoch_metric_sums, total_metric_reward)
				.unwrap_or_default();
		reward = reward.saturating_add(metric_rewards);
		let Some(commitment_id) = Self::backing_lookup(manager_id) else {
			return RewardInfo {
				reward,
				metrics_reward_claimed: true,
				staked_compute_reward_claimed: false,
			};
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
				previous_epoch_metric_sums,
			);

			bonus
		});
		reward = reward.saturating_add(bonus);
		RewardInfo { reward, metrics_reward_claimed: true, staked_compute_reward_claimed: true }
	}

	fn update_metrics_epoch_sum(
		manager_id: T::ManagerId,
		pool_id: PoolId,
		metric: Metric,
		epoch: EpochOf<T>,
		bonus: bool,
	) -> MetricPoolUpdateInfo {
		let metric_with_bonus = if bonus {
			let bonus =
				FixedU128::from_inner(T::BusyWeightBonus::get().mul_floor(metric.into_inner()));
			metric.saturating_add(bonus)
		} else {
			metric
		};
		let prev_epoch = epoch.saturating_sub(One::one());
		// sum totals
		let prev_total: Option<(Metric, Perquintill)> =
			<MetricPools<T, I>>::mutate(pool_id, |pool: &mut Option<MetricPoolFor<T>>| {
				let pool = pool.as_mut()?;
				pool.add(epoch, metric);
				pool.add_bonus(epoch, metric_with_bonus);
				Some((pool.total.get(prev_epoch), pool.reward.get(prev_epoch)))
			});
		let mut result = MetricPoolUpdateInfo::new(pool_id, None, prev_total);
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
				result.epoch_sum = Some(sum.get(prev_epoch));
			}

			result
		})
	}

	fn commit_new_metrics(
		processor: &T::AccountId,
		manager_id: T::ManagerId,
		metrics: &[MetricInput],
		active: bool,
		cycle: CycleFor<T>,
	) -> CommitMetricsInfo {
		let epoch = cycle.epoch;

		let mut prev_metrics: Vec<(PoolId, Metric)> = vec![];
		let mut prev_metrics_sum: Vec<(PoolId, (Metric, Metric))> = vec![];
		let mut prev_pool_totals: Vec<(PoolId, (Metric, Perquintill))> = vec![];
		for (pool_id, numerator, denominator) in metrics {
			let Some(metric) = FixedU128::checked_from_rational(
				*numerator,
				if denominator.is_zero() { One::one() } else { *denominator },
			) else {
				continue;
			};
			let maybe_current_metric = Metrics::<T, I>::get(processor, pool_id);
			let first_in_epoch = maybe_current_metric
				.as_ref()
				.map(|m| {
					// first value committed for `epoch` wins
					m.epoch < epoch
				})
				.unwrap_or(true);
			if first_in_epoch {
				// insert even if not active for tracability before warmup ended
				Metrics::<T, I>::insert(processor, pool_id, MetricCommit { epoch, metric });
				if active {
					if let Some(prev_metric) = maybe_current_metric {
						prev_metrics.push((*pool_id, prev_metric.metric));
					}
					let update_info =
						Self::update_metrics_epoch_sum(manager_id, *pool_id, metric, epoch, false);
					if let Some(prev_sum) = update_info.epoch_sum {
						prev_metrics_sum.push((update_info.pool_id, prev_sum));
					}
					if let Some(prev_pool_total) = update_info.pool_total {
						prev_pool_totals.push((update_info.pool_id, prev_pool_total));
					}
				}
			}
		}

		CommitMetricsInfo {
			previous_sums: if !prev_metrics_sum.is_empty() { Some(prev_metrics_sum) } else { None },
			previous_metrics: if !prev_metrics.is_empty() { Some(prev_metrics) } else { None },
			previous_pool_totals: if !prev_pool_totals.is_empty() {
				Some(prev_pool_totals)
			} else {
				None
			},
		}
	}

	fn reuse_metrics(
		processor: &T::AccountId,
		manager_id: T::ManagerId,
		active: bool,
		cycle: CycleFor<T>,
	) -> CommitMetricsInfo {
		let epoch = cycle.epoch;

		let mut to_update: Vec<(PoolId, MetricCommit<_>)> = vec![];
		for (pool_id, metric) in Metrics::<T, I>::iter_prefix(processor) {
			if epoch > metric.epoch && epoch - metric.epoch < T::MetricValidity::get() {
				to_update.push((pool_id, metric));
			}
		}

		let mut prev_metrics: Vec<(PoolId, Metric)> = vec![];
		let mut prev_metrics_sum: Vec<(PoolId, (Metric, Metric))> = vec![];
		let mut prev_pool_totals: Vec<(PoolId, (Metric, Perquintill))> = vec![];

		for (pool_id, commit) in to_update {
			// if we are here we know that this reused metric is "first_in_epoch" since we reuse maximally once per epoch
			let metric_commit = MetricCommit { epoch, metric: commit.metric };
			Metrics::<T, I>::insert(processor, pool_id, metric_commit);

			if active {
				prev_metrics.push((pool_id, metric_commit.metric));
				let update_info =
					Self::update_metrics_epoch_sum(manager_id, pool_id, commit.metric, epoch, true);
				if let Some(prev_sum) = update_info.epoch_sum {
					prev_metrics_sum.push((update_info.pool_id, prev_sum));
				}
				if let Some(prev_pool_total) = update_info.pool_total {
					prev_pool_totals.push((update_info.pool_id, prev_pool_total));
				}
			}
		}

		CommitMetricsInfo {
			previous_sums: if !prev_metrics_sum.is_empty() { Some(prev_metrics_sum) } else { None },
			previous_metrics: if !prev_metrics.is_empty() { Some(prev_metrics) } else { None },
			previous_pool_totals: if !prev_pool_totals.is_empty() {
				Some(prev_pool_totals)
			} else {
				None
			},
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
					*id =
						id.checked_add(&1u128.into()).ok_or(Error::<T, I>::CalculationOverflow)?;
					Ok(new_id)
				})?;

				T::CommitmentIdProvider::create_commitment_id(id, committer)?;

				Ok((id, true))
			})
	}
}

impl<T: Config<I>, I: 'static> RewardContributionProvider<T::AccountId, BalanceFor<T, I>>
	for Pallet<T, I>
where
	BalanceFor<T, I>: IsType<u128>,
{
	fn reward_contribution_per_block_for(processor: &T::AccountId) -> Option<BalanceFor<T, I>> {
		let processor_state = Self::processors(processor)?;
		let epoch_length = T::Epoch::get().try_into().ok()?;
		Some(
			processor_state
				.reward_contribution
				.saturated_into::<u128>()
				.saturating_div(epoch_length)
				.into(),
		)
	}
}
