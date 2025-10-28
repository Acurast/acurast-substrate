use acurast_common::{CommitmentIdProvider, PoolId};
use frame_support::{
	pallet_prelude::*,
	traits::{
		Currency, ExistenceRequirement, Get, InspectLockableCurrency, LockableCurrency,
		WithdrawReasons,
	},
};
use sp_core::{U256, U512};
use sp_runtime::{
	traits::{CheckedAdd, CheckedSub, Saturating, Zero},
	FixedU128, Perbill, Perquintill, SaturatedConversion, Vec,
};

use crate::types::{FIXEDU128_DECIMALS, PER_TOKEN_DECIMALS};
use crate::*;

/// Helper trait for ceiling division that rounds up instead of down
trait CheckedDivCeil<Rhs = Self> {
	type Output;

	/// Performs ceiling division (rounding up) using checked arithmetic.
	/// Returns `None` if the division would overflow or if the divisor is zero.
	fn checked_div_ceil(&self, divisor: &Rhs) -> Option<Self::Output>;
}

// Implement CheckedDivCeil for U256
impl CheckedDivCeil<U256> for U256 {
	type Output = U256;

	fn checked_div_ceil(&self, divisor: &U256) -> Option<U256> {
		if divisor.is_zero() {
			return None;
		}

		// Formula: (numerator + divisor - 1) / divisor
		// This rounds up any remainder to the next integer
		let numerator_adjusted = self.checked_add(divisor)?.checked_sub(U256::one())?;

		numerator_adjusted.checked_div(*divisor)
	}
}

// Implement CheckedDivCeil for U256
impl CheckedDivCeil<U512> for U512 {
	type Output = U512;

	fn checked_div_ceil(&self, divisor: &U512) -> Option<U512> {
		if divisor.is_zero() {
			return None;
		}

		// Formula: (numerator + divisor - 1) / divisor
		// This rounds up any remainder to the next integer
		let numerator_adjusted = self.checked_add(divisor)?.checked_sub(U512::one())?;

		numerator_adjusted.checked_div(*divisor)
	}
}

#[derive(Clone, PartialEq, Eq)]
enum StakeChange<Balance> {
	Add(Balance),
	Sub(Balance),
}

impl<T: Config<I>, I: 'static> Pallet<T, I>
where
	BalanceFor<T, I>: From<u128>,
{
	/// Validates and stores compute commitments for a given commitment_id
	pub fn validate_max_metric_store_commitments(
		commitment_id: T::CommitmentId,
		commitment: impl IntoIterator<Item = ComputeCommitment>,
	) -> Result<(), Error<T, I>> {
		let manager_id = <Backings<T, I>>::get(commitment_id)
			.ok_or(Error::<T, I>::NoManagerBackingCommitment)?;

		let epoch = Self::current_cycle().epoch;

		let mut count: usize = 0;
		for c in commitment {
			let _ = <MetricPools<T, I>>::get(c.pool_id).ok_or(Error::<T, I>::PoolNotFound)?;
			let (metric_sum, _) = MetricsEpochSum::<T, I>::get(manager_id, c.pool_id)
				.get(epoch.checked_sub(&One::one()).ok_or(Error::<T, I>::CannotCommit)?);
			ensure!(c.metric <= metric_sum, Error::<T, I>::MaxMetricCommitmentExceeded);
			let ratio = Perquintill::from_parts(((c.metric / metric_sum).into_inner()) as u64);
			ensure!(
				ratio <= T::MaxMetricCommitmentRatio::get(),
				Error::<T, I>::MaxMetricCommitmentExceeded
			);

			// Check if there's an existing metric commitment and ensure it doesn't decrease
			<ComputeCommitments<T, I>>::try_mutate(
				commitment_id,
				c.pool_id,
				|old_metric| -> Result<(), Error<T, I>> {
					if let Some(old) = old_metric {
						// Ensure new metric is greater than or equal to old metric
						ensure!(c.metric >= *old, Error::<T, I>::CommittedMetricCannotDecrease);
					}
					*old_metric = Some(c.metric);
					Ok(())
				},
			)?;
			count += 1;
		}
		ensure!(count > 0, Error::<T, I>::ZeroMetricsForValidPools);

		Ok(())
	}

	/// Validates that the max_stake_metric_ratio is not violated for all pools where the commitment has committed non-zero metrics.
	pub fn validate_max_stake_metric_ratio(
		commitment_id: T::CommitmentId,
	) -> Result<(), Error<T, I>> {
		let epoch = Self::current_cycle().epoch;

		let reward_weight = Self::commitments(commitment_id)
			.ok_or(Error::<T, I>::CommitmentNotFound)?
			.weights
			.get(epoch)
			.total_reward_weight();

		// Check existing commitments from storage
		for (pool_id, metric) in <ComputeCommitments<T, I>>::iter_prefix(commitment_id) {
			// Get the pool to check if it has max_stake_metric_ratio configured
			let Some(_pool) = <MetricPools<T, I>>::get(pool_id) else {
				continue; // Pool not found, skip; this is not an internal error since maybe pool got deleted but older version of processor still supplies metric
			};

			ensure!(!metric.is_zero(), Error::<T, I>::MaxStakeMetricRatioExceeded);

			let target_weight_per_compute =
				Self::stake_based_rewards(pool_id).get(epoch).target_weight_per_compute;
			if target_weight_per_compute.is_zero() {
				continue;
			}

			// actual_weight_per_compute = target_weight_per_compute * metric
			let actual_weight_per_compute = reward_weight
				.checked_mul(U256::from(FIXEDU128_DECIMALS))
				.ok_or(Error::<T, I>::CalculationOverflow)?
				.checked_div(U256::from(metric.into_inner()))
				.ok_or(Error::<T, I>::CalculationOverflow)?;

			// Check if actual ratio exceeds max allowed ratio
			ensure!(
				actual_weight_per_compute <= target_weight_per_compute,
				Error::<T, I>::MaxStakeMetricRatioExceeded
			);
		}

		Ok(())
	}

	/// Update score for a specific commitment for the given epoch which should be the current epoch (it's passed for efficiency reasons).
	///
	/// Call only once per committer per epoch! This is not validated inside this function!
	pub fn score(
		last_epoch: EpochOf<T>,
		epoch: EpochOf<T>,
		commitment_id: T::CommitmentId,
		commitment: &CommitmentFor<T, I>,
		previous_epoch_metric_sums: &[(PoolId, (Metric, Metric))],
	) -> Result<(), Error<T, I>> {
		let weights = commitment.weights.get(last_epoch);
		let commitment_total_weight = weights.total_reward_weight();

		for (pool_id, (metric_sum, metric_with_bonus_sum)) in previous_epoch_metric_sums {
			// yes! it's correct to take this from current epoch, not last, because it got written in on_initialize of this epoch
			let target_weight_per_compute =
				StakeBasedRewards::<T, I>::get(pool_id).get(epoch).target_weight_per_compute;
			let bonus_sum = metric_with_bonus_sum
				.checked_sub(metric_sum)
				.ok_or(Error::<T, I>::CalculationOverflow)?;
			let committed_metric_sum = ComputeCommitments::<T, I>::get(commitment_id, pool_id)
				.ok_or(Error::<T, I>::CommitmentNotFound)?;

			// calculate min(committed_metric_sum, measured_metric_sum)
			// compare metrics WITHOUT BONI (there is no concept for metrics committed with bonis)
			let (commitment_bounded_metric_sum, bounded_bonus) =
				if *metric_sum < committed_metric_sum {
					// fall down to actual since commitment violated, and also don't give ANY boni
					(*metric_sum, Zero::zero())
				} else {
					// we give the bonus because commitment was hold

					// a committer cannot get more bonus than for the equivalent of busy devices summing up to the committed compute
					let max_bonus = FixedU128::from_inner(
						T::BusyWeightBonus::get().mul_floor(committed_metric_sum.into_inner()),
					);
					if bonus_sum > max_bonus {
						(committed_metric_sum, max_bonus)
					} else {
						(committed_metric_sum, bonus_sum)
					}
				};

			let score = {
				let score = U256::from(commitment_bounded_metric_sum.into_inner())
					.checked_mul(commitment_total_weight)
					.ok_or(Error::<T, I>::CalculationOverflow)?
					.checked_div(U256::from(FIXEDU128_DECIMALS))
					.ok_or(Error::<T, I>::CalculationOverflow)?
					.integer_sqrt();
				let score_limit = target_weight_per_compute
					.checked_mul(U256::from(commitment_bounded_metric_sum.into_inner()))
					.ok_or(Error::<T, I>::CalculationOverflow)?
					.checked_div(U256::from(FIXEDU128_DECIMALS))
					.ok_or(Error::<T, I>::CalculationOverflow)?
					.checked_div(U256::from(PER_TOKEN_DECIMALS))
					.ok_or(Error::<T, I>::CalculationOverflow)?;
				if score_limit < score {
					score_limit
				} else {
					score
				}
			};

			// bonus score dedicated to committer, not delegators
			let bonus_score = {
				// calculate from only reward_weight (reduced during cooldown) of committer (ignoring delegations' weights)
				let score = U256::from(bounded_bonus.into_inner())
					.checked_mul(weights.self_reward_weight)
					.ok_or(Error::<T, I>::CalculationOverflow)?
					.checked_div(U256::from(FIXEDU128_DECIMALS))
					.ok_or(Error::<T, I>::CalculationOverflow)?
					.integer_sqrt();
				let score_limit = target_weight_per_compute
					.checked_mul(U256::from(bounded_bonus.into_inner()))
					.ok_or(Error::<T, I>::CalculationOverflow)?
					.checked_div(U256::from(FIXEDU128_DECIMALS))
					.ok_or(Error::<T, I>::CalculationOverflow)?
					.checked_div(U256::from(PER_TOKEN_DECIMALS))
					.ok_or(Error::<T, I>::CalculationOverflow)?;
				if score_limit < score {
					score_limit
				} else {
					score
				}
			};

			let score_with_bonus =
				score.checked_add(bonus_score).ok_or(Error::<T, I>::CalculationOverflow)?;

			Scores::<T, I>::mutate(commitment_id, pool_id, |s| {
				s.set(epoch, (score, score_with_bonus));
			});

			StakeBasedRewards::<T, I>::try_mutate(pool_id, |r| -> Result<(), Error<T, I>> {
				r.mutate(
					epoch,
					|budget| {
						budget.total_score = budget.total_score.saturating_add(score_with_bonus);
					},
					false,
				);
				Ok(())
			})?;
		}

		Ok(())
	}

	/// Distribute for a specific commitment for the given epoch which should be not the current but last epoch (it's passed for efficiency reasons).
	///
	/// Call only once per committer per epoch! This is not validated inside this function!
	pub fn distribute(
		epoch: EpochOf<T>,
		commitment_id: T::CommitmentId,
		commitment: &mut CommitmentFor<T, I>,
		pool_ids: &[PoolId],
	) -> Result<BalanceFor<T, I>, Error<T, I>> {
		let committer_stake = commitment.stake.as_mut().ok_or(Error::<T, I>::CommitmentNotFound)?;

		let weights = commitment.weights.get(epoch);
		let commitment_total_weight = weights.total_reward_weight();

		let mut total_delegations_reward: BalanceFor<T, I> = Zero::zero();
		let mut total_committer_bonus: BalanceFor<T, I> = Zero::zero();
		for pool_id in pool_ids {
			StakeBasedRewards::<T, I>::try_mutate(pool_id, |r| -> Result<(), Error<T, I>> {
				let budget = r.get(epoch);
				if budget.total_score.is_zero() || budget.total.is_zero() {
					// nothing for this pool to distribute
					return Ok(());
				}

				let (score, bonus_score) = Self::scores(commitment_id, pool_id).get(epoch);

				// reward = score * budget.total / total_weighted_score
				let reward = score
					.checked_mul(U256::from(budget.total.saturated_into::<u128>()))
					.ok_or(Error::<T, I>::CalculationOverflow)?
					.checked_div(budget.total_score)
					.ok_or(Error::<T, I>::CalculationOverflow)?;
				// Handle bonus
				{
					// bonus_reward = score * budget.total / total_weighted_score
					// we have to repeat division budget.total/budget.total_score for precision reasons
					let bonus_reward = bonus_score
						.checked_mul(U256::from(budget.total.saturated_into::<u128>()))
						.ok_or(Error::<T, I>::CalculationOverflow)?
						.checked_div(budget.total_score)
						.ok_or(Error::<T, I>::CalculationOverflow)?
						.saturated_into::<u128>();
					total_committer_bonus = total_committer_bonus
						.checked_add(&bonus_reward.into())
						.ok_or(Error::<T, I>::CalculationOverflow)?;
				}

				// split epoch_reward between self and delegators
				let self_share = weights
					.self_reward_weight
					.checked_mul(reward)
					.ok_or(Error::<T, I>::CalculationOverflow)?
					.checked_div(commitment_total_weight)
					.ok_or(Error::<T, I>::CalculationOverflow)?;
				let delegations_share =
					reward.checked_sub(self_share).ok_or(Error::<T, I>::CalculationOverflow)?;

				// convert back to balance type
				let self_share_amount: BalanceFor<T, I> =
					self_share.saturated_into::<u128>().into();
				let delegations_share_amount: BalanceFor<T, I> =
					delegations_share.saturated_into::<u128>().into();

				// apply commission
				let commission_amount = commitment
					.commission
					.mul_floor(delegations_share_amount.saturated_into::<u128>())
					.into();
				let self_amount = self_share_amount
					.checked_add(&commission_amount)
					.ok_or(Error::<T, I>::CalculationOverflow)?;
				let delegations_amount = delegations_share_amount
					.checked_sub(&commission_amount)
					.ok_or(Error::<T, I>::CalculationOverflow)?;

				// add to commitment accrued rewards
				committer_stake.accrued_reward = committer_stake
					.accrued_reward
					.checked_add(&self_amount)
					.ok_or(Error::<T, I>::CalculationOverflow)?;

				// add to delegator pool rewards
				total_delegations_reward = total_delegations_reward
					.checked_add(&delegations_amount)
					.ok_or(Error::<T, I>::CalculationOverflow)?;

				// keep track of distributed
				// better recalculate epoch_reward from potential rounded shares
				let epoch_reward_amount = self_amount.saturating_add(delegations_amount);
				r.mutate(
					epoch,
					|v| {
						v.distributed = v.distributed.saturating_add(epoch_reward_amount);
					},
					false,
				);

				Ok(())
			})?;
		}

		// reward_delegation_pool
		if !weights.delegations_reward_weight.is_zero() && !total_delegations_reward.is_zero() {
			let extra = U256::from(total_delegations_reward.saturated_into::<u128>())
				.checked_mul(U256::from(PER_TOKEN_DECIMALS))
				.ok_or(Error::<T, I>::CalculationOverflow)?
				.checked_div(weights.delegations_reward_weight)
				.ok_or(Error::<T, I>::CalculationOverflow)?;
			// TODO add try_mutate to MemoryBuffer to make this not saturating
			commitment
				.pool_rewards
				.mutate(
					committer_stake.created,
					|r| {
						r.reward_per_weight = r.reward_per_weight.saturating_add(extra);
					},
					true,
				)
				.map_err(|_| Error::<T, I>::InternalError)?;
		}

		Ok(total_committer_bonus)
	}

	/// Commitments for the first time.
	pub fn stake_for(
		who: &T::AccountId,
		amount: BalanceFor<T, I>,
		cooldown_period: BlockNumberFor<T>,
		commission: Perbill,
		allow_auto_compound: bool,
	) -> Result<(), Error<T, I>> {
		ensure!(
			!amount.is_zero() && amount >= <T as Config<I>>::MinStake::get(),
			Error::<T, I>::MinStakeSubceeded
		);
		ensure!(
			cooldown_period >= T::MinCooldownPeriod::get(),
			Error::<T, I>::BelowMinCooldownPeriod
		);
		ensure!(
			cooldown_period <= T::MaxCooldownPeriod::get(),
			Error::<T, I>::AboveMaxCooldownPeriod
		);

		// locking is on accounts (while all other storage points are relative to `commitment_id`)
		Self::lock_funds(who, amount, LockReason::Staking)?;

		let commitment_id = T::CommitmentIdProvider::commitment_id_for(who)
			.map_err(|_| Error::<T, I>::NoOwnerOfCommitmentId)?;

		let epoch = Self::current_cycle().epoch;

		<Commitments<T, I>>::try_mutate(commitment_id, |c_| -> Result<(), Error<T, I>> {
			if let Some(c) = c_ {
				ensure!(c.stake.is_none(), Error::<T, I>::AlreadyCommitted);
			}

			let created = <frame_system::Pallet<T>>::block_number();
			let stake = Stake::new(amount, created, cooldown_period, allow_auto_compound);
			let self_reward_weight = U256::from(stake.rewardable_amount.saturated_into::<u128>())
				.checked_mul(U256::from(stake.cooldown_period.saturated_into::<u128>()))
				.ok_or(Error::<T, I>::CalculationOverflow)?
				.checked_div(U256::from(T::MaxCooldownPeriod::get().saturated_into::<u128>()))
				.ok_or(Error::<T, I>::CalculationOverflow)?;
			let self_slash_weight = self_reward_weight;
			// slash weight remains always like initial, also during cooldown
			// let self_slash_weight = U256::from(stake.amount.saturated_into::<u128>())
			// 	.checked_mul(U256::from(stake.cooldown_period.saturated_into::<u128>()))
			// 	.ok_or(Error::<T, I>::CalculationOverflow)?
			// 	.checked_div(U256::from(T::MaxCooldownPeriod::get().saturated_into::<u128>()))
			// 	.ok_or(Error::<T, I>::CalculationOverflow)?;

			Self::update_total_stake(StakeChange::Add(stake.amount))?;

			if let Some(c) = c_ {
				ensure!(c.stake.is_none(), Error::<T, I>::AlreadyCommitted);

				c.stake = Some(stake);
				c.commission = commission;
				c.delegations_total_amount = Zero::zero();
				c.delegations_total_rewardable_amount = Zero::zero();
				// completely reset, no memory needed
				c.weights = MemoryBuffer::new_with(
					epoch,
					CommitmentWeights {
						self_reward_weight,
						self_slash_weight,
						delegations_reward_weight: Zero::zero(),
						delegations_slash_weight: Zero::zero(),
					},
				);
				// do not reset, keep the old rewards around until next restaking
				// so we can still fulfill delegator's claims until next commitment (which is earliest after MinCooldownPeriod, long enough for delegators to make their claims).
				c.pool_rewards
					.set(created, Default::default())
					.map_err(|_| Error::<T, I>::InternalErrorReadingOutdated)?;
			} else {
				*c_ = Some(Commitment {
					stake: Some(stake),
					commission,
					delegations_total_amount: Zero::zero(),
					delegations_total_rewardable_amount: Zero::zero(),
					weights: MemoryBuffer::new_with(
						epoch,
						CommitmentWeights {
							self_reward_weight,
							self_slash_weight,
							delegations_reward_weight: Zero::zero(),
							delegations_slash_weight: Zero::zero(),
						},
					),
					pool_rewards: MemoryBuffer::new_with(created, Default::default()),
					last_scoring_epoch: Zero::zero(),
					last_slashing_epoch: Zero::zero(),
				});
			}

			Ok(())
		})?;

		Ok(())
	}

	/// Commitments an extra (additional) amount towards an existing commitment.
	pub fn stake_more_for(
		who: &T::AccountId,
		extra_amount: BalanceFor<T, I>,
		cooldown_period: Option<BlockNumberFor<T>>,
		commission: Option<Perbill>,
		allow_auto_compound: Option<bool>,
	) -> Result<(), Error<T, I>> {
		let commitment_id = T::CommitmentIdProvider::commitment_id_for(who)
			.map_err(|_| Error::<T, I>::NoOwnerOfCommitmentId)?;
		let epoch = Self::current_cycle().epoch;

		<Commitments<T, I>>::try_mutate(commitment_id, |c_| -> Result<(), Error<T, I>> {
			let c = c_.as_mut().ok_or(Error::<T, I>::CommitmentNotFound)?;
			let stake = c.stake.as_mut().ok_or(Error::<T, I>::CommitmentNotFound)?;
			ensure!(stake.cooldown_started.is_none(), Error::<T, I>::CommitmentInCooldown);
			if !extra_amount.is_zero() {
				stake.amount = stake
					.amount
					.checked_add(&extra_amount)
					.ok_or(Error::<T, I>::CalculationOverflow)?;
				stake.rewardable_amount = stake.amount;

				// locking is on accounts (while all other storage points are relative to `commitment_id`)
				Self::lock_funds(who, stake.amount, LockReason::Staking)?;
			}

			if let Some(cooldown_period) = cooldown_period {
				ensure!(
					cooldown_period >= stake.cooldown_period,
					Error::<T, I>::CooldownPeriodCannotDecrease
				);
				stake.cooldown_period = cooldown_period;
			}

			if let Some(commission) = commission {
				ensure!(commission <= c.commission, Error::<T, I>::CommissionCannotIncrease);
				c.commission = commission;
			}

			if let Some(allow_auto_compound) = allow_auto_compound {
				stake.allow_auto_compound = allow_auto_compound;
			}

			let self_reward_weight = U256::from(stake.rewardable_amount.saturated_into::<u128>())
				.checked_mul(U256::from(stake.cooldown_period.saturated_into::<u128>()))
				.ok_or(Error::<T, I>::CalculationOverflow)?
				.checked_div(U256::from(T::MaxCooldownPeriod::get().saturated_into::<u128>()))
				.ok_or(Error::<T, I>::CalculationOverflow)?;

			c.weights
				.mutate(
					epoch,
					|w| {
						w.self_reward_weight = self_reward_weight;
						w.self_slash_weight = self_reward_weight;
					},
					true,
				)
				.map_err(|_| Error::<T, I>::InternalErrorReadingOutdated)?;

			Self::update_total_stake(StakeChange::Add(extra_amount))?;

			Ok(())
		})?;

		Ok(())
	}

	pub fn delegate_for(
		who: &T::AccountId,
		commitment_id: T::CommitmentId,
		amount: BalanceFor<T, I>,
		cooldown_period: BlockNumberFor<T>,
		allow_auto_compound: bool,
	) -> Result<(), Error<T, I>> {
		ensure!(
			!amount.is_zero() && amount >= T::MinDelegation::get(),
			Error::<T, I>::BelowMinDelegation
		);
		ensure!(
			cooldown_period >= T::MinCooldownPeriod::get(),
			Error::<T, I>::BelowMinCooldownPeriod
		);
		ensure!(
			cooldown_period <= T::MaxCooldownPeriod::get(),
			Error::<T, I>::AboveMaxCooldownPeriod
		);

		let epoch = Self::current_cycle().epoch;
		let created = <frame_system::Pallet<T>>::block_number();
		<Commitments<T, I>>::try_mutate(commitment_id, |c_| -> Result<(), Error<T, I>> {
			let commitment = c_.as_mut().ok_or(Error::<T, I>::CommitmentNotFound)?;
			let committer_stake =
				commitment.clone().stake.ok_or(Error::<T, I>::CommitmentNotFound)?;

			ensure!(
				cooldown_period <= committer_stake.cooldown_period,
				Error::<T, I>::DelegationCooldownMustBeShorterThanCommitment
			);
			ensure!(
				committer_stake.cooldown_started.is_none(),
				Error::<T, I>::CommitmentInCooldown
			);

			Self::lock_funds(who, amount, LockReason::Delegation(commitment_id))?;

			let reward_weight = U256::from(amount.saturated_into::<u128>())
				.checked_mul(U256::from(cooldown_period.saturated_into::<u128>()))
				.ok_or(Error::<T, I>::CalculationOverflow)?
				.checked_div(U256::from(T::MaxCooldownPeriod::get().saturated_into::<u128>()))
				.ok_or(Error::<T, I>::CalculationOverflow)?;
			let slash_weight = reward_weight;

			let reward_debt = reward_weight
				.checked_mul(commitment.pool_rewards.get_latest(created).reward_per_weight)
				.ok_or(Error::<T, I>::CalculationOverflow)?
				.checked_div_ceil(&U256::from(PER_TOKEN_DECIMALS))
				.ok_or(Error::<T, I>::CalculationOverflow)?;
			let slash_debt = slash_weight
				.checked_mul(commitment.pool_rewards.get_latest(created).slash_per_weight)
				.ok_or(Error::<T, I>::CalculationOverflow)?
				.checked_div(U256::from(PER_TOKEN_DECIMALS))
				.ok_or(Error::<T, I>::CalculationOverflow)?;

			ensure!(
				Delegations::<T, I>::get(who, commitment_id).is_none(),
				Error::<T, I>::AlreadyDelegating
			);

			Delegations::<T, I>::insert(
				who,
				commitment_id,
				Delegation {
					stake: Stake::new(amount, created, cooldown_period, allow_auto_compound),
					reward_weight,
					slash_weight,
					reward_debt: reward_debt.as_u128().into(),
					slash_debt: slash_debt.as_u128().into(),
				},
			);

			// UPDATE per pool weights and global TOTALS
			commitment.delegations_total_amount =
				commitment.delegations_total_amount.saturating_add(amount);
			commitment.delegations_total_rewardable_amount =
				commitment.delegations_total_rewardable_amount.saturating_add(amount);
			commitment
				.weights
				.mutate(
					epoch,
					|w| {
						w.delegations_reward_weight =
							w.delegations_reward_weight.saturating_add(reward_weight);
						w.delegations_slash_weight =
							w.delegations_slash_weight.saturating_add(slash_weight)
					},
					true,
				)
				.map_err(|_| Error::<T, I>::InternalErrorReadingOutdated)?;

			Self::update_total_stake(StakeChange::Add(amount))?;
			// delegator_total += amount
			<DelegatorTotal<T, I>>::try_mutate(who, |s| -> Result<(), Error<T, I>> {
				*s = s.checked_add(&amount).ok_or(Error::<T, I>::CalculationOverflow)?;
				Ok(())
			})?;

			// This check has to happen after `delegations_reward_weight` was updated
			ensure!(
				Self::delegation_weight_ratio(epoch, commitment)? <= T::MaxDelegationRatio::get(),
				Error::<T, I>::MaxDelegationRatioExceeded
			);

			Ok(())
		})
	}

	pub fn delegate_more_for(
		who: &T::AccountId,
		commitment_id: T::CommitmentId,
		extra_amount: BalanceFor<T, I>,
		cooldown_period: Option<BlockNumberFor<T>>,
		allow_auto_compound: Option<bool>,
	) -> Result<(), Error<T, I>> {
		let old_delegation =
			Self::delegations(who, commitment_id).ok_or(Error::<T, I>::NotDelegating)?;

		let commitment =
			Self::commitments(commitment_id).ok_or(Error::<T, I>::CommitmentNotFound)?;

		let committer_stake = commitment.clone().stake.ok_or(Error::<T, I>::CommitmentNotFound)?;

		// We error out if the existing delegation was for a previous commitment that got ended and "replaced" by a new commitment by same committer.
		// In this case the delegator needs to end his delegation first.
		ensure!(
			old_delegation.stake.created >= committer_stake.created,
			Error::<T, I>::StaleDelegationMustBeEnded
		);

		// we don't check here if there is capacity for extra_amount since it will fail below in `delegate_for` if not
		let amount = old_delegation
			.stake
			.amount
			.checked_add(&extra_amount)
			.ok_or(Error::<T, I>::CalculationOverflow)?;

		let cooldown_period = if let Some(cooldown_period) = cooldown_period {
			ensure!(
				cooldown_period >= old_delegation.stake.cooldown_period,
				Error::<T, I>::CooldownPeriodCannotDecrease
			);
			cooldown_period
		} else {
			old_delegation.stake.cooldown_period
		};

		let allow_auto_compound =
			allow_auto_compound.unwrap_or(old_delegation.stake.allow_auto_compound);

		// TODO: improve this two calls to not unlock and lock the amount unnecessarily
		let reward = Self::end_delegation_for(who, commitment_id, false, false)?;
		let distribution_account = Self::account_id();
		if !reward.is_zero() {
			T::Currency::transfer(
				&distribution_account,
				who,
				reward,
				ExistenceRequirement::KeepAlive,
			)
			.map_err(|_| Error::<T, I>::InternalError)?;
		}
		Self::delegate_for(who, commitment_id, amount, cooldown_period, allow_auto_compound)?;
		Ok(())
	}

	pub fn compound_delegator(
		who: &T::AccountId,
		commitment_id: T::CommitmentId,
	) -> Result<BalanceFor<T, I>, Error<T, I>> {
		let compound_amount = Self::withdraw_delegation_for(who, commitment_id)?;
		Self::delegate_more_for(who, commitment_id, compound_amount, None, None)?;

		Ok(compound_amount)
	}

	pub fn compound_committer(
		committer: &T::AccountId,
		commitment_id: T::CommitmentId,
	) -> Result<BalanceFor<T, I>, Error<T, I>> {
		let compound_amount = Self::withdraw_committer_for(committer, commitment_id)?;
		Self::stake_more_for(committer, compound_amount, None, None, None)?;

		Ok(compound_amount)
	}

	fn update_total_stake(change: StakeChange<BalanceFor<T, I>>) -> Result<(), Error<T, I>> {
		match change {
			StakeChange::Add(amount) => {
				<TotalStake<T, I>>::try_mutate(|s| -> Result<(), Error<T, I>> {
					*s = s.checked_add(&amount).ok_or(Error::<T, I>::CalculationOverflow)?;
					Ok(())
				})
			},
			StakeChange::Sub(amount) => {
				<TotalStake<T, I>>::try_mutate(|s| -> Result<(), Error<T, I>> {
					*s = s.checked_sub(&amount).ok_or(Error::<T, I>::CalculationOverflow)?;
					Ok(())
				})
			},
		}
	}

	/// Accrues into the accrued balances for a delegation.
	fn accrue_delegator(
		who: &T::AccountId,
		commitment_id: T::CommitmentId,
	) -> Result<(), Error<T, I>> {
		let commitment =
			Self::commitments(commitment_id).ok_or(Error::<T, I>::CommitmentNotFound)?;

		Delegations::<T, I>::try_mutate(who, commitment_id, |d_| -> Result<(), Error<T, I>> {
			let d = d_.as_mut().ok_or(Error::<T, I>::NotDelegating)?;
			let reward_u256 = d
				.reward_weight
				.checked_mul(commitment.pool_rewards.get_latest(d.stake.created).reward_per_weight)
				.ok_or(Error::<T, I>::CalculationOverflow)?
				.checked_div(U256::from(PER_TOKEN_DECIMALS))
				.ok_or(Error::<T, I>::CalculationOverflow)?;
			let reward: BalanceFor<T, I> = reward_u256.as_u128().into();
			let reward = reward.saturating_sub(d.reward_debt);

			let slash_u256 = d
				.slash_weight
				.checked_mul(commitment.pool_rewards.get_latest(d.stake.created).slash_per_weight)
				.ok_or(Error::<T, I>::CalculationOverflow)?
				.checked_div(U256::from(PER_TOKEN_DECIMALS))
				.ok_or(Error::<T, I>::CalculationOverflow)?;
			let slash: BalanceFor<T, I> = slash_u256.as_u128().into();
			let slash =
				slash.checked_sub(&d.slash_debt).ok_or(Error::<T, I>::CalculationOverflow)?;

			d.stake.accrued_reward = d
				.stake
				.accrued_reward
				.checked_add(&reward)
				.ok_or(Error::<T, I>::CalculationOverflow)?;
			d.stake.accrued_slash = d
				.stake
				.accrued_slash
				.checked_add(&slash)
				.ok_or(Error::<T, I>::CalculationOverflow)?;

			d.reward_debt = d
				.reward_weight
				.checked_mul(commitment.pool_rewards.get_latest(d.stake.created).reward_per_weight)
				.ok_or(Error::<T, I>::CalculationOverflow)?
				.checked_div_ceil(&U256::from(PER_TOKEN_DECIMALS))
				.ok_or(Error::<T, I>::CalculationOverflow)?
				.as_u128()
				.into();
			d.slash_debt = d
				.slash_weight
				.checked_mul(commitment.pool_rewards.get_latest(d.stake.created).slash_per_weight)
				.ok_or(Error::<T, I>::CalculationOverflow)?
				.checked_div(U256::from(PER_TOKEN_DECIMALS))
				.ok_or(Error::<T, I>::CalculationOverflow)?
				.as_u128()
				.into();
			Ok(())
		})
	}

	/// It is guaranteed to withdraw reward/slash only if the result is Ok.
	fn withdraw_delegator_accrued(
		who: &T::AccountId,
		commitment_id: T::CommitmentId,
	) -> Result<BalanceFor<T, I>, Error<T, I>> {
		Self::accrue_delegator(who, commitment_id)?;

		Delegations::<T, I>::try_mutate(who, commitment_id, |d_| {
			let d = d_.as_mut().ok_or(Error::<T, I>::CommitmentNotFound)?;
			let r = d.stake.accrued_reward;
			// let s = PendingReward::new(state.accrued_slash);
			d.stake.accrued_reward = Zero::zero();
			// TODO maybe apply accrued_slash as far as possible to reward? so less get's applied to stake
			// state.accrued_slash = Zero::zero();
			d.stake.paid = d.stake.paid.saturating_add(r);
			Ok(r)
		})
	}

	/// It is guaranteed to withdraw reward/slash only if the result is Ok.
	pub fn withdraw_delegation_for(
		who: &T::AccountId,
		commitment_id: T::CommitmentId,
	) -> Result<BalanceFor<T, I>, Error<T, I>> {
		let reward = Self::withdraw_delegator_accrued(who, commitment_id)?;

		// Transfer reward to the caller if any
		let distribution_account = Self::account_id();
		if !reward.is_zero() {
			T::Currency::transfer(
				&distribution_account,
				who,
				reward,
				ExistenceRequirement::KeepAlive,
			)
			.map_err(|_| Error::<T, I>::InternalError)?;
		}

		Ok(reward)
	}

	/// It is guaranteed to withdraw reward/slash only if the result is Ok.
	pub fn withdraw_committer_for(
		who: &T::AccountId,
		commitment_id: T::CommitmentId,
	) -> Result<BalanceFor<T, I>, Error<T, I>> {
		let reward = Self::withdraw_committer_accrued(commitment_id)?;

		// Transfer reward to the caller if any
		let distribution_account = Self::account_id();
		if !reward.is_zero() {
			T::Currency::transfer(
				&distribution_account,
				who,
				reward,
				ExistenceRequirement::KeepAlive,
			)
			.map_err(|_| Error::<T, I>::InternalError)?;
		}

		Ok(reward)
	}

	/// It is guaranteed to withdraw reward/slash only if the result is Ok.
	pub fn withdraw_committer_accrued(
		commitment_id: T::CommitmentId,
	) -> Result<BalanceFor<T, I>, Error<T, I>> {
		Commitments::<T, I>::try_mutate(commitment_id, |c_| {
			let c = c_.as_mut().ok_or(Error::<T, I>::CommitmentNotFound)?;
			let stake = c.stake.as_mut().ok_or(Error::<T, I>::CommitmentNotFound)?;
			let r = stake.accrued_reward;
			// let s = PendingReward::new(state.accrued_slash);
			stake.accrued_reward = Zero::zero();
			// TODO maybe apply accrued_slash as far as possible to reward? so less get's applied to stake
			// state.accrued_slash = Zero::zero();
			stake.paid = stake.paid.saturating_add(r);
			Ok(r)
		})
	}

	pub fn cooldown_commitment_for(commitment_id: T::CommitmentId) -> Result<(), Error<T, I>> {
		let current_block = <frame_system::Pallet<T>>::block_number();
		let epoch = Self::current_cycle().epoch;

		<Commitments<T, I>>::try_mutate(commitment_id, |c_| -> Result<(), Error<T, I>> {
			let c = c_.as_mut().ok_or(Error::<T, I>::CommitmentNotFound)?;
			let stake = c.stake.as_mut().ok_or(Error::<T, I>::CommitmentNotFound)?;
			ensure!(stake.cooldown_started.is_none(), Error::<T, I>::CooldownAlreadyStarted);

			stake.cooldown_started = Some(current_block);
			// this has to be calculated once and stored, so changes to `T::CooldownRewardRatio` config don't mess up totals
			stake.rewardable_amount = T::CooldownRewardRatio::get()
				.mul_floor(stake.amount.saturated_into::<u128>())
				.into();

			let self_reward_weight = U256::from(stake.rewardable_amount.saturated_into::<u128>())
				.checked_mul(U256::from(stake.cooldown_period.saturated_into::<u128>()))
				.ok_or(Error::<T, I>::CalculationOverflow)?
				.checked_div(U256::from(T::MaxCooldownPeriod::get().saturated_into::<u128>()))
				.ok_or(Error::<T, I>::CalculationOverflow)?;
			c.weights
				.mutate(epoch, |w| w.self_reward_weight = self_reward_weight, true)
				.map_err(|_| Error::<T, I>::InternalErrorReadingOutdated)?;

			Ok(())
		})
	}

	/// Starts the cooldown for a delegation.
	///
	/// The delegator's stake stays locked and slashable until the end of cooldown (or longer until he calls `end_delegation`).
	/// Rewards are going to be distributed at a reduced weight, so the commitment's reward_weight in metric pools is already decreased to reflect the reduction, but not so the slash_weight for continuing the ability to slash delegators up to their original stake and, until now, compounded rewards.
	pub fn cooldown_delegation_for(
		who: &T::AccountId,
		commitment_id: T::CommitmentId,
	) -> Result<(), Error<T, I>> {
		Self::accrue_delegator(who, commitment_id)?;

		let epoch = Self::current_cycle().epoch;
		<Commitments<T, I>>::try_mutate(commitment_id, |c_| -> Result<(), Error<T, I>> {
			let commitment = c_.as_mut().ok_or(Error::<T, I>::CommitmentNotFound)?;
			let committer_stake =
				commitment.clone().stake.ok_or(Error::<T, I>::StaleDelegationMustBeEnded)?;

			// Special case: the commitment delegated to is in cooldown itself (started by committer), or even ended cooldown.
			// In this case the start of the delegator's cooldown is pretended to have occurred at the staker's start of cooldown,
			// as a lazy imitation of starting all delegator's cooldown together with commitment cooldown.

			let cooldown_start = if let Some(c) = committer_stake.cooldown_started {
				c
			} else {
				<frame_system::Pallet<T>>::block_number()
			};

			<Delegations<T, I>>::try_mutate(who, commitment_id, |d_| -> Result<(), Error<T, I>> {
				let d = d_.as_mut().ok_or(Error::<T, I>::NotDelegating)?;
				ensure!(d.stake.cooldown_started.is_none(), Error::<T, I>::CooldownAlreadyStarted);
				// We error out if the existing delegation was for a previous commitment that got ended and "replaced" by a new commitment by same committer
				// In this case the delegator needs to end (or redelegate) his delegation first.
				ensure!(
					d.stake.created >= committer_stake.created,
					Error::<T, I>::StaleDelegationMustBeEnded
				);

				d.stake.cooldown_started = Some(cooldown_start);
				// this has to be calculated once and stored, so changes to `T::CooldownRewardRatio` config don't mess up totals
				d.stake.rewardable_amount = T::CooldownRewardRatio::get()
					.mul_floor(d.stake.amount.saturated_into::<u128>())
					.saturated_into();
				let reward_weight = U256::from(d.stake.rewardable_amount.saturated_into::<u128>())
					.checked_mul(U256::from(d.stake.cooldown_period.saturated_into::<u128>()))
					.ok_or(Error::<T, I>::CalculationOverflow)?
					.checked_div(U256::from(T::MaxCooldownPeriod::get().saturated_into::<u128>()))
					.ok_or(Error::<T, I>::CalculationOverflow)?;

				let reward_weight_diff = d
					.reward_weight
					.checked_sub(reward_weight)
					.ok_or(Error::<T, I>::CalculationOverflow)?;
				d.reward_weight = reward_weight;

				commitment.delegations_total_rewardable_amount = commitment
					.delegations_total_rewardable_amount
					.saturating_sub(d.stake.amount.saturating_sub(d.stake.rewardable_amount));
				commitment
					.weights
					.mutate(
						epoch,
						|w| {
							w.delegations_reward_weight =
								w.delegations_reward_weight.saturating_sub(reward_weight_diff);
						},
						true,
					)
					.map_err(|_| Error::<T, I>::InternalErrorReadingOutdated)?;

				// feshly record reward_debt because of changed reward_weight!!
				d.reward_debt = d
					.reward_weight
					.checked_mul(
						commitment.pool_rewards.get_latest(d.stake.created).reward_per_weight,
					)
					.ok_or(Error::<T, I>::CalculationOverflow)?
					.checked_div_ceil(&U256::from(PER_TOKEN_DECIMALS))
					.ok_or(Error::<T, I>::CalculationOverflow)?
					.as_u128()
					.into();

				Ok(())
			})
		})
	}

	pub fn redelegate_for(
		who: &T::AccountId,
		old_commitment_id: T::CommitmentId,
		new_commitment_id: T::CommitmentId,
	) -> Result<(), DispatchError> {
		// Check if the caller is a delegator to the old commitment
		let old_delegation =
			<Delegations<T, I>>::get(who, old_commitment_id).ok_or(Error::<T, I>::NotDelegating)?;
		// Check that the caller is not already delegating to new commitment (nice for specific error but it would fail below in delegate_for otherwise)
		ensure!(
			Self::delegations(who, new_commitment_id).is_none(),
			Error::<T, I>::AlreadyDelegatingToRedelegationCommitter
		);

		// Check if the old commitment is in cooldown (if it ended we error out since it's rational to end delegation and decide fresh whom and with what parameters to delegate)
		let old_commitment_stake = Self::commitments(old_commitment_id)
			.ok_or(Error::<T, I>::CommitmentNotFound)?
			.stake
			.ok_or(Error::<T, I>::StaleDelegationMustBeEnded)?;

		// We error out if the existing delegation was for a previous commitment that got ended and "replaced" by a new commitment by same committer.
		// In this case the delegator needs to end his delegation first.
		ensure!(
			old_delegation.stake.created >= old_commitment_stake.created,
			Error::<T, I>::StaleDelegationMustBeEnded
		);

		// Only check RedelegationBlockingPeriod is respected if current committer is not in cooldown, otherwise allow immediate redelegation always
		if old_commitment_stake.cooldown_started.is_none() {
			// check if enough epochs have passed since last update (which lead to `created` field being reset)
			let blocks_since_created = old_delegation.stake.created;
			ensure!(
				blocks_since_created
					>= T::RedelegationBlockingPeriod::get().saturating_mul(T::Epoch::get()),
				Error::<T, I>::RedelegateBlocked
			);
		}

		// Check if new committer has more own stake than the old one
		let old_cooldown = old_commitment_stake.cooldown_period;
		let new_cooldown = Self::commitments(new_commitment_id)
			.ok_or(Error::<T, I>::NewCommitmentNotFound)?
			.stake
			.ok_or(Error::<T, I>::NewCommitmentNotFound)?
			.cooldown_period;

		ensure!(
			new_cooldown >= old_cooldown,
			Error::<T, I>::RedelegationCommitterCooldownCannotBeShorter
		);

		for (pool_id, old_metric) in ComputeCommitments::<T, I>::iter_prefix(old_commitment_id) {
			let new_metric = ComputeCommitments::<T, I>::get(new_commitment_id, pool_id)
				.ok_or(Error::<T, I>::CommitmentNotFound)?;
			ensure!(
				new_metric >= old_metric,
				Error::<T, I>::RedelegationCommitmentMetricsCannotBeLess
			);
		}

		// TODO: improve this two calls to not unlock and lock the amount unnecessarily
		let reward = Self::end_delegation_for(who, old_commitment_id, false, false)?;
		let distribution_account = Self::account_id();
		if !reward.is_zero() {
			T::Currency::transfer(
				&distribution_account,
				who,
				reward,
				ExistenceRequirement::KeepAlive,
			)
			.map_err(|_| Error::<T, I>::InternalError)?;
		}
		Self::delegate_for(
			who,
			new_commitment_id,
			old_delegation.stake.amount,
			old_delegation.stake.cooldown_period,
			old_delegation.stake.allow_auto_compound,
		)?;

		Ok(())
	}

	pub fn end_delegation_for(
		who: &T::AccountId,
		commitment_id: T::CommitmentId,
		check_cooldown: bool,
		attempt_kickout: bool,
	) -> Result<BalanceFor<T, I>, Error<T, I>> {
		let current_block = <frame_system::Pallet<T>>::block_number();
		let epoch = Self::current_cycle().epoch;

		let reward = Self::withdraw_delegation_for(who, commitment_id)?;

		<Commitments<T, I>>::try_mutate(commitment_id, |c_| -> Result<(), Error<T, I>> {
			let commitment = c_.as_mut().ok_or(Error::<T, I>::CommitmentNotFound)?;
			let delegation = <Delegations<T, I>>::try_mutate(
				who,
				commitment_id,
				|d_| -> Result<DelegationFor<T, I>, Error<T, I>> {
					let d = d_.as_mut().ok_or(Error::<T, I>::NotDelegating)?;
					if check_cooldown {
						if let Some(committer_stake) = commitment.stake.clone() {
							// skip all cooldown checks if the existing delegation was for a previous commitment that got ended and "replaced" by a new commitment by same committer
							if d.stake.created >= committer_stake.created {
								match (committer_stake.cooldown_started, d.stake.cooldown_started) {
									(
										Some(committer_cooldown_start),
										Some(delegator_cooldown_start),
									) => {
										let first = if committer_cooldown_start
											< delegator_cooldown_start
										{
											committer_cooldown_start
										} else {
											delegator_cooldown_start
										};
										ensure!(
											first.saturating_add(d.stake.cooldown_period)
												<= current_block,
											Error::<T, I>::CooldownNotEnded
										);
									},
									(Some(committer_cooldown_start), None) => {
										// inherit the committer's cooldown start
										ensure!(
											committer_cooldown_start
												.saturating_add(d.stake.cooldown_period)
												<= current_block,
											Error::<T, I>::CooldownNotEnded
										);
									},
									(None, Some(delegator_cooldown_start)) => {
										ensure!(
											delegator_cooldown_start
												.saturating_add(d.stake.cooldown_period)
												<= current_block,
											Error::<T, I>::CooldownNotEnded
										);
									},
									(None, None) => {
										Err(Error::<T, I>::CooldownNotStarted)?;
									},
								}
							}
						}
						// if the commitment is gone, which means the committer even passed his cooldown without the delegator taking action (like redelegating)
						// -> can immediately exit
					}

					Ok(d_.take().unwrap())
				},
			)?;

			if let Some(committer_stake) = commitment.stake.clone() {
				// skip if the existing delegation was for a previous commitment that got ended and "replaced" by a new commitment by same committer
				if delegation.stake.created >= committer_stake.created {
					if attempt_kickout {
						Err(Error::<T, I>::CannotKickout)?;
					}

					commitment.delegations_total_amount =
						commitment.delegations_total_amount.saturating_sub(delegation.stake.amount);
					commitment.delegations_total_rewardable_amount = commitment
						.delegations_total_rewardable_amount
						.saturating_sub(delegation.stake.rewardable_amount);
					commitment
						.weights
						.mutate(
							epoch,
							|w| {
								w.delegations_reward_weight = w
									.delegations_reward_weight
									.saturating_sub(delegation.reward_weight);
								w.delegations_slash_weight = w
									.delegations_slash_weight
									.saturating_sub(delegation.slash_weight);
							},
							true,
						)
						.map_err(|_| Error::<T, I>::InternalErrorReadingOutdated)?;
				}
			}

			Self::update_total_stake(StakeChange::Sub(delegation.stake.amount))?;
			// delegator_total -= amount
			<DelegatorTotal<T, I>>::try_mutate(who, |s| -> Result<(), Error<T, I>> {
				*s = s
					.checked_sub(&delegation.stake.amount)
					.ok_or(Error::<T, I>::CalculationOverflow)?;
				Ok(())
			})?;

			Self::unlock_and_slash(who, &delegation.stake)?;

			Ok(())
		})?;

		Ok(reward)
	}

	pub fn kickout_delegation(
		delegator: &T::AccountId,
		commitment_id: T::CommitmentId,
	) -> Result<BalanceFor<T, I>, Error<T, I>> {
		let reward = Self::end_delegation_for(delegator, commitment_id, false, true)?;
		let distribution_account = Self::account_id();
		if !reward.is_zero() {
			T::Currency::transfer(
				&distribution_account,
				delegator,
				reward,
				ExistenceRequirement::KeepAlive,
			)
			.map_err(|_| Error::<T, I>::InternalError)?;
		}

		Ok(reward)
	}

	pub fn end_commitment_for(
		who: &T::AccountId,
		commitment_id: T::CommitmentId,
		check_cooldown: bool,
	) -> Result<BalanceFor<T, I>, Error<T, I>> {
		let current_block = <frame_system::Pallet<T>>::block_number();
		let epoch = Self::current_cycle().epoch;

		let reward = Self::withdraw_committer_for(who, commitment_id)?;

		let stake = <Commitments<T, I>>::try_mutate(
			commitment_id,
			|c_| -> Result<StakeFor<T, I>, Error<T, I>> {
				let c = c_.as_mut().ok_or(Error::<T, I>::CommitmentNotFound)?;
				let committer_stake = c.stake.clone().ok_or(Error::<T, I>::CommitmentNotFound)?;
				if check_cooldown {
					let cooldown_start = committer_stake
						.cooldown_started
						.ok_or(Error::<T, I>::CooldownNotStarted)?;
					ensure!(
						cooldown_start.saturating_add(committer_stake.cooldown_period)
							<= current_block,
						Error::<T, I>::CooldownNotEnded
					);
				}

				c.delegations_total_amount = Zero::zero();
				c.delegations_total_rewardable_amount = Zero::zero();

				// reset weights inclusive memory (no more distributions will happen as soon as commitment was ended).
				c.weights = MemoryBuffer::new_with(epoch, Default::default());
				// do not reset pool_rewards so we can still fulfill delegator's claims until next commitment (which is earliest after MinCooldownPeriod, long enough for delegators to make their claims).

				// remove the compute commitment too
				let _ = <ComputeCommitments<T, I>>::clear_prefix(commitment_id, u32::MAX, None);

				Ok(c.stake.take().unwrap())
			},
		)?;

		Self::update_total_stake(StakeChange::Sub(stake.amount))?;

		Self::unlock_and_slash(who, &stake)?;

		// Eventhough all delegator's cooldown has force-ended before this unstake is successful,
		// we cannot clear all delegators here because it would make this call non-constant in number of delegators.
		// We let them remain in the delegation pool and make sure ending delegation is tolerating the commitment already gone.
		// The case of no remaining delegations is handled above by taking out the option with `c_.take()`.

		Ok(reward)
	}

	pub fn do_slash(commitment_id: T::CommitmentId) -> Result<(), Error<T, I>> {
		let epoch = Self::current_cycle().epoch;
		let last_epoch =
			epoch.checked_sub(&One::one()).ok_or(Error::<T, I>::CalculationOverflow)?;

		let commitment = Self::commitments(commitment_id).ok_or(Error::CommitmentNotFound)?;
        let committer_stake = commitment.stake.as_ref().ok_or(Error::<T, I>::CommitmentNotFound)?;

		// Check if already slashed in the last epoch to prevent double slashing
		ensure!(commitment.last_slashing_epoch < last_epoch, Error::<T, I>::AlreadySlashed);

		let manager_id = <Backings<T, I>>::get(commitment_id)
			.ok_or(Error::<T, I>::NoManagerBackingCommitment)?;

		// Calculate the total slash amount across all pools
		let mut total_slash_amount: BalanceFor<T, I> = Zero::zero();

		// Calculate total commitment stake (committer + delegations)
		let total_stake = committer_stake.amount
			.checked_add(&commitment.delegations_total_amount).ok_or(Error::<T, I>::CalculationOverflow)?;

		// Check all pools for which there are commitments
		for (pool_id, committed_metric) in <ComputeCommitments<T, I>>::iter_prefix(commitment_id) {
			// Get the pool to access its reward ratio
			let pool = <MetricPools<T, I>>::get(pool_id).ok_or(Error::<T, I>::PoolNotFound)?;

			// Get the pool's reward ratio for the last epoch
			let pool_reward_ratio = pool.reward.get(last_epoch);

			// Get actual metrics delivered in the last epoch
			let metric_epoch_sum = MetricsEpochSum::<T, I>::get(manager_id, pool_id);
			let (actual_metric_sum, _) = metric_epoch_sum.get(last_epoch);

			let missed_epochs: u128 =
				if actual_metric_sum.is_zero() && metric_epoch_sum.epoch < last_epoch {
					// we still now an "old" value since it got not overwritten and we can slash for all the epoch's missed
					last_epoch.saturating_sub(metric_epoch_sum.epoch).saturated_into::<u128>()
				} else {
					One::one()
				};

			// Calculate slash amount for this pool
			let pool_slash_amount: BalanceFor<T, I> =
				if let Some(unfulfilled) = committed_metric.checked_sub(&actual_metric_sum) {
					let unfulfilled_ratio = Perquintill::from_rational(
						unfulfilled.into_inner(),
						committed_metric.into_inner(),
					);

					// Calculate pool's share of base slash amount as ratio of total stake
					let pool_max_slash = pool_reward_ratio
						.mul_floor(T::BaseSlashAmount::get().mul_floor(total_stake.saturated_into::<u128>()));

					unfulfilled_ratio.mul_floor(pool_max_slash).saturating_mul(missed_epochs).into()
				} else {
					// Metrics fulfilled, no slash for this pool
					Zero::zero()
				};

			total_slash_amount = total_slash_amount
				.checked_add(&pool_slash_amount)
				.ok_or(Error::<T, I>::CalculationOverflow)?;
		}

		ensure!(!total_slash_amount.is_zero(), Error::<T, I>::NotSlashable);

		// Get the slash weights for this epoch
		let weights = commitment.weights.get(last_epoch);
		let total_slash_weight = weights.total_slash_weight();

		// If no slash weight, nothing to slash; technically never happens but avoids division-by-zero below
		if total_slash_weight.is_zero() {
			return Ok(());
		}

		// Calculate self share and delegations share based on slash weights
		let self_share_u256 = weights
			.self_slash_weight
			.checked_mul(U256::from(total_slash_amount.saturated_into::<u128>()))
			.ok_or(Error::<T, I>::CalculationOverflow)?
			.checked_div(total_slash_weight)
			.ok_or(Error::<T, I>::CalculationOverflow)?;

		let delegations_share_u256 = U256::from(total_slash_amount.saturated_into::<u128>())
			.checked_sub(self_share_u256)
			.ok_or(Error::<T, I>::CalculationOverflow)?;

		// Convert to balance types
		let self_slash_amount: BalanceFor<T, I> = self_share_u256.saturated_into::<u128>().into();
		let delegations_slash_amount: BalanceFor<T, I> =
			delegations_share_u256.saturated_into::<u128>().into();

		// Calculate slash increase on delegation pool slash_per_weight
		let slash_per_weight_increase =
			if !weights.delegations_slash_weight.is_zero() && !delegations_slash_amount.is_zero() {
				Some(
					delegations_share_u256
						.checked_mul(U256::from(PER_TOKEN_DECIMALS))
						.ok_or(Error::<T, I>::CalculationOverflow)?
						.checked_div(weights.delegations_slash_weight)
						.ok_or(Error::<T, I>::CalculationOverflow)?,
				)
			} else {
				None
			};

		<Commitments<T, I>>::try_mutate(commitment_id, |c_| -> Result<(), Error<T, I>> {
			let c = c_.as_mut().ok_or(Error::<T, I>::CommitmentNotFound)?;
			let stake = c.stake.as_mut().ok_or(Error::<T, I>::CommitmentNotFound)?;

			// Apply slash to committer
			stake.accrued_slash = stake
				.accrued_slash
				.checked_add(&self_slash_amount)
				.ok_or(Error::<T, I>::CalculationOverflow)?;

			// Apply slash to delegation pool if applicable
			if let Some(increase) = slash_per_weight_increase {
				c.pool_rewards
					.mutate(
						stake.created,
						|r| {
							r.slash_per_weight = r.slash_per_weight.saturating_add(increase);
						},
						true,
					)
					.map_err(|_| Error::<T, I>::InternalError)?;
			}

			// Set last_slashing_epoch to prevent double slashing
			c.last_slashing_epoch = last_epoch;

			Ok(())
		})?;

		Ok(())
	}

	fn unlock_and_slash(who: &T::AccountId, stake: &StakeFor<T, I>) -> Result<(), Error<T, I>> {
		Self::unlock_funds(who, stake.amount);
		// Transfer the already unlocked stake with accrued slash, be sure to fail hard on errors!a
		if !stake.accrued_slash.is_zero() {
			let distribution_account = Self::account_id();
			// Transfer the slashed amount from user to distribution account
			T::Currency::transfer(
				who,
				&distribution_account,
				stake.accrued_slash,
				ExistenceRequirement::AllowDeath,
			)
			.map_err(|_| Error::<T, I>::InternalError)?;
		}

		Ok(())
	}

	/// Force-unstakes a commitment by removing all delegations and the commitment's own stake.
	/// This bypasses normal cooldown and validation checks.
	pub fn force_end_commitment_for(commitment_id: T::CommitmentId) {
		// Calculate total amounts to be removed for updating global totals
		let mut total_delegation_amount: BalanceFor<T, I> = Zero::zero();
		let mut total_stake_amount: BalanceFor<T, I> = Zero::zero();

		// Find all delegations to this commitment and remove them
		let all_delegations: Vec<(T::AccountId, T::CommitmentId, _)> =
			<Delegations<T, I>>::iter().collect();
		for (delegator, delegation_commitment_id, stake) in all_delegations {
			if delegation_commitment_id == commitment_id {
				// Update delegator totals
				<DelegatorTotal<T, I>>::mutate(&delegator, |s| {
					*s = s.saturating_sub(stake.stake.amount);
				});

				// Unlock funds for delegator
				Self::unlock_funds(&delegator, stake.stake.amount);

				// Add to total delegation amount
				total_delegation_amount =
					total_delegation_amount.saturating_add(stake.stake.amount);

				// Remove this specific delegation
				<Delegations<T, I>>::remove(&delegator, commitment_id);
			}
		}

		// Remove commitment's own stake
		if let Some(c) = <Commitments<T, I>>::take(commitment_id) {
			if let Some(stake) = c.stake {
				total_stake_amount = stake.amount;
				if let Ok(committer) = T::CommitmentIdProvider::owner_for(commitment_id) {
					Self::unlock_funds(&committer, stake.amount);
				}
			}
		}

		// Calculate total amount to remove from global stake
		let total_amount_to_remove = total_delegation_amount.saturating_add(total_stake_amount);

		// Update global totals
		if !total_amount_to_remove.is_zero() {
			<TotalStake<T, I>>::mutate(|s| {
				*s = s.saturating_sub(total_amount_to_remove);
			});
		}
		let _ = Self::update_total_stake(StakeChange::Sub(total_amount_to_remove));

		// Remove compute commitments
		let _ = <ComputeCommitments<T, I>>::clear_prefix(commitment_id, u32::MAX, None);
	}

	pub fn delegation_weight_ratio(
		epoch: EpochOf<T>,
		c: &CommitmentFor<T, I>,
	) -> Result<Perquintill, Error<T, I>> {
		let w = c.weights.get_latest(epoch);
		let commitment_total_weight = w
			.self_slash_weight
			.checked_add(w.delegations_reward_weight)
			.ok_or(Error::<T, I>::CalculationOverflow)?;
		let nominator: u128 = w.delegations_reward_weight.saturated_into();
		let denominator: u128 = commitment_total_weight.saturated_into();
		if nominator > denominator {
			return Err(Error::<T, I>::MaxDelegationRatioExceeded);
		}
		Ok(if denominator > 0 {
			Perquintill::from_rational(nominator, denominator)
		} else {
			Perquintill::zero()
		})
	}

	// fn delegation_ratio(
	// 	epoch: EpochOf<T>,
	// 	c: &CommitmentFor<T, I>,
	// ) -> Result<Perquintill, Error<T, I>> {
	// 	let committer_stake = c.stake.clone().ok_or(Error::<T, I>::CommitmentNotFound)?;
	// 	let commitment_total_weight = committer_stake
	// 		.rewardable_amount
	// 		.checked_add(&c.delegations_total_rewardable_amount)
	// 		.ok_or(Error::<T, I>::CalculationOverflow)?;
	// 	let nominator: u128 = c.delegations_total_rewardable_amount.saturated_into();
	// 	let denominator: u128 = commitment_total_weight.saturated_into();
	// 	Ok(if denominator > 0 {
	// 		Perquintill::from_rational(nominator, denominator)
	// 	} else {
	// 		Perquintill::zero()
	// 	})
	// }

	/// Locks the new stake on the account. The account can have existing stake or delegations locked.
	///
	/// NOTE: we have to lock total stake not difference, so this helper function must be aware of all existing reasons for locking from the compute pallet, under [`T::LockIdentifier`].
	///
	/// This method ensures the new total is locked, respecting potential previous delegation locks for same commitment.
	pub fn lock_funds(
		who: &T::AccountId,
		amount: BalanceFor<T, I>,
		reason: LockReason<T::CommitmentId>,
	) -> Result<(), Error<T, I>> {
		let new_lock_total = match reason {
			LockReason::Staking => {
				let delegator_total = <DelegatorTotal<T, I>>::get(who);
				delegator_total.saturating_add(amount)
			},
			LockReason::Delegation(commitment_id) => {
				let staked = if let Ok(delegator_commitment_id) =
					T::CommitmentIdProvider::commitment_id_for(who)
				{
					<Commitments<T, I>>::get(delegator_commitment_id)
						.map(|c| c.stake)
						.unwrap_or(None)
						.map(|stake| stake.amount)
				} else {
					None
				}
				.unwrap_or(Zero::zero());
				let delegated = <Delegations<T, I>>::get(who, commitment_id)
					.map(|d| d.stake.amount)
					.unwrap_or(Zero::zero());
				let delegator_total = <DelegatorTotal<T, I>>::get(who);
				delegator_total
					.saturating_sub(delegated)
					.saturating_add(staked)
					.saturating_add(amount)
			},
		};

		// also reserved balance can be locked, therefore compare to total_balance
		if <T::Currency as Currency<T::AccountId>>::total_balance(who)
			< new_lock_total.saturated_into()
		{
			Err(Error::<T, I>::InsufficientBalance)?;
		}
		<T::Currency as LockableCurrency<T::AccountId>>::set_lock(
			T::LockIdentifier::get(),
			who,
			new_lock_total.saturated_into(),
			WithdrawReasons::all(),
		);
		Ok(())
	}

	/// Returns the staked amount to an account's usable balance (for the part that is not also
	/// reserved) by unlocking the amount.
	///
	/// Note that the free balance does not change.
	pub fn unlock_funds(who: &T::AccountId, amount: BalanceFor<T, I>) {
		let new_total_stake =
			<T::Currency as InspectLockableCurrency<T::AccountId>>::balance_locked(
				T::LockIdentifier::get(),
				who,
			)
			.saturating_sub(amount.saturated_into());

		if new_total_stake.is_zero() {
			<T::Currency as LockableCurrency<T::AccountId>>::remove_lock(
				T::LockIdentifier::get(),
				who,
			);
		} else {
			<T::Currency as LockableCurrency<T::AccountId>>::set_lock(
				T::LockIdentifier::get(),
				who,
				new_total_stake.saturated_into(),
				WithdrawReasons::all(),
			);
		}
	}
}
