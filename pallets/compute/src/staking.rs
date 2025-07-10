use acurast_common::{CommitmentIdProvider, PoolId};
use frame_support::{
	pallet_prelude::*,
	sp_runtime::SaturatedConversion,
	traits::{Currency, Get, InspectLockableCurrency, LockableCurrency, WithdrawReasons},
};
use sp_runtime::{
	traits::{CheckedAdd, CheckedSub, Saturating, Zero},
	Perquintill,
};
use sp_std::vec::Vec;

use crate::{reward::PendingReward, *};

#[derive(Clone, PartialEq, Eq)]
enum StakeChange<Balance> {
	Add(Balance),
	Sub(Balance),
}

impl<T: Config<I>, I: 'static> Pallet<T, I> {
	pub fn distribute(epoch: EpochOf<T>, amount: BalanceFor<T, I>) -> Result<(), Error<T, I>> {
		for (pool_id, pool) in MetricPools::<T, I>::iter() {
			let a: u128 = amount.saturated_into();
			Self::distribute_top(
				pool_id,
				(pool.reward.get(epoch).mul_floor(a.saturated_into::<u128>()) as u128)
					.saturated_into(),
			)?;
		}

		Ok(())
	}

	fn distribute_top(pool_id: PoolId, amount: BalanceFor<T, I>) -> Result<(), Error<T, I>> {
		StakingPools::<T, I>::try_mutate(pool_id, |pool| {
			if !pool.reward_weight.is_zero() {
				pool.reward_per_token = pool
					.reward_per_token
					.checked_add(
						&amount
							.checked_mul(&T::Decimals::get())
							.ok_or(Error::<T, I>::CalculationOverflow)?
							.checked_div(&pool.reward_weight)
							.ok_or(Error::<T, I>::CalculationOverflow)?,
					)
					.ok_or(Error::<T, I>::CalculationOverflow)?;
			}

			Ok(())
		})?;

		Ok(())
	}

	/// Helper function that must be called before a delegator or committer is performing any operation on delegation pool that influences the commitment's derived weight in metric pool.
	fn distribute_down(commitment_id: T::CommitmentId) -> Result<(), Error<T, I>> {
		for (pool_id, _metric) in ComputeCommitments::<T, I>::iter_prefix(commitment_id) {
			// we have to accrue and distribute everything from "top pools" == (the metric pools the "delegation target" == commitment is in)
			let reward = Self::accrue_and_withdraw_commitment(pool_id, commitment_id)?;
			Self::reward_delegation_pool(commitment_id, reward)?;
		}

		Ok(())
	}

	fn reward_delegation_pool(
		commitment_id: T::CommitmentId,
		reward: PendingReward<BalanceFor<T, I>>,
	) -> Result<(), Error<T, I>> {
		if reward.is_zero() {
			return Ok(());
		}
		DelegationPools::<T, I>::try_mutate(commitment_id, |pool| -> Result<(), Error<T, I>> {
			if !pool.reward_weight.is_zero() {
				pool.reward_per_token = pool
					.reward_per_token
					.checked_add(
						&(reward.consume())
							.checked_mul(&T::Decimals::get())
							.ok_or(Error::<T, I>::CalculationOverflow)?
							.checked_div(&pool.reward_weight)
							.ok_or(Error::<T, I>::CalculationOverflow)?,
					)
					.ok_or(Error::<T, I>::CalculationOverflow)?;
			}
			Ok(())
		})
	}

	pub fn slash_delegation_pool(
		commitment_id: T::CommitmentId,
		amount: BalanceFor<T, I>,
	) -> Result<(), Error<T, I>> {
		DelegationPools::<T, I>::try_mutate(commitment_id, |pool| -> Result<(), Error<T, I>> {
			if !pool.slash_weight.is_zero() {
				pool.slash_per_token = pool
					.slash_per_token
					.checked_add(
						&amount
							.checked_mul(&T::Decimals::get())
							.ok_or(Error::<T, I>::CalculationOverflow)?
							.checked_div(&pool.slash_weight)
							.ok_or(Error::<T, I>::CalculationOverflow)?,
					)
					.ok_or(Error::<T, I>::CalculationOverflow)?;
			}
			Ok(())
		})
	}

	/// Stakes for the first time.
	pub fn stake_for(
		who: &T::AccountId,
		amount: BalanceFor<T, I>,
		cooldown_period: BlockNumberFor<T>,
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
		Self::lock_funds(&who, amount, LockReason::Staking)?;

		let commitment_id = T::CommitmentIdProvider::commitment_id_for(who)
			.map_err(|_| Error::<T, I>::NoOwnerOfCommitmentId)?;
		let stake = <Stakes<T, I>>::try_mutate(
			commitment_id,
			|state| -> Result<StakeFor<T, I>, Error<T, I>> {
				ensure!(state.is_none(), Error::<T, I>::AlreadyCommitted);
				let s = Stake::new(amount, cooldown_period, allow_auto_compound);
				*state = Some(s.clone());
				Ok(s)
			},
		)?;

		Self::update_self_delegation(commitment_id)?;
		Self::update_commitment_stake(commitment_id, StakeChange::Add(stake.amount))?;
		Self::update_total_stake(StakeChange::Add(stake.amount))?;

		Ok(())
	}

	fn update_self_delegation(commitment_id: T::CommitmentId) -> Result<(), Error<T, I>> {
		let stake = <Stakes<T, I>>::get(commitment_id).ok_or(Error::<T, I>::CommitmentNotFound)?;

		// committer (via commitment_id) also joins the delegation pool
		// reward_weight reduces during cooldown
		let reward_weight = stake
			.rewardable_amount
			.checked_mul(&stake.cooldown_period.saturated_into::<u128>().saturated_into())
			.ok_or(Error::<T, I>::CalculationOverflow)?
			/ T::MaxCooldownPeriod::get().saturated_into::<u128>().saturated_into();
		// slash weight remains always like initial, also during cooldown
		let slash_weight = stake
			.amount
			.checked_mul(&stake.cooldown_period.saturated_into::<u128>().saturated_into())
			.ok_or(Error::<T, I>::CalculationOverflow)?
			/ T::MaxCooldownPeriod::get().saturated_into::<u128>().saturated_into();

		let reward_debt = reward_weight
			.checked_mul(&DelegationPools::<T, I>::get(commitment_id).reward_per_token)
			.ok_or(Error::<T, I>::CalculationOverflow)?
			/ T::Decimals::get();
		let slash_debt = reward_weight
			.checked_mul(&DelegationPools::<T, I>::get(commitment_id).slash_per_token)
			.ok_or(Error::<T, I>::CalculationOverflow)?
			/ T::Decimals::get();
		<SelfDelegation<T, I>>::insert(
			commitment_id,
			DelegationPoolMember { reward_weight, slash_weight, reward_debt, slash_debt },
		);

		Ok(())
	}

	/// Stakes an extra (additional) amount towards an existing commitment.
	pub fn stake_more_for(
		who: &T::AccountId,
		extra_amount: BalanceFor<T, I>,
	) -> Result<(), Error<T, I>> {
		ensure!(!extra_amount.is_zero(), Error::<T, I>::MinStakeSubceeded);

		let commitment_id = T::CommitmentIdProvider::commitment_id_for(who)
			.map_err(|_| Error::<T, I>::NoOwnerOfCommitmentId)?;

		<Stakes<T, I>>::try_mutate(commitment_id, |stake_| -> Result<(), Error<T, I>> {
			let stake = stake_.as_mut().ok_or(Error::<T, I>::CommitmentNotFound)?;
			let prev_amount = stake.amount;
			let amount = prev_amount
				.checked_add(&extra_amount)
				.ok_or(Error::<T, I>::CalculationOverflow)?;

			// locking is on accounts (while all other storage points are relative to `commitment_id`)
			Self::lock_funds(&who, amount, LockReason::Staking)?;

			stake.amount = amount;

			Ok(())
		})?;

		Self::update_self_delegation(commitment_id)?;
		Self::distribute_down(commitment_id)?;
		Self::update_commitment_stake(commitment_id, StakeChange::Add(extra_amount))?;
		Self::update_total_stake(StakeChange::Add(extra_amount))?;

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
		let committer_stake =
			Stakes::<T, I>::get(&commitment_id).ok_or(Error::<T, I>::CommitmentNotFound)?;
		ensure!(
			cooldown_period <= committer_stake.cooldown_period,
			Error::<T, I>::DelegationCooldownMustBeShorterThanCommitment
		);

		Self::lock_funds(&who, amount, LockReason::Delegation(commitment_id))?;

		Self::distribute_down(commitment_id)?;
		Self::update_commitment_stake(commitment_id, StakeChange::Add(amount))?;
		Self::update_total_stake(StakeChange::Add(amount))?;

		// This check has to happen after `update_commitment_stake` to ensure the commitment_stake is updated what it would be after this call completed and is not rolled back
		ensure!(
			Self::delegation_ratio(commitment_id) <= T::MaxDelegationRatio::get(),
			Error::<T, I>::MaxDelegationRatioExceeded
		);

		let weight = amount
			.checked_mul(&cooldown_period.saturated_into::<u128>().saturated_into())
			.ok_or(Error::<T, I>::CalculationOverflow)?
			/ T::MaxCooldownPeriod::get().saturated_into::<u128>().saturated_into();
		let reward_debt = weight
			.checked_mul(&DelegationPools::<T, I>::get(commitment_id).reward_per_token)
			.ok_or(Error::<T, I>::CalculationOverflow)?
			/ T::Decimals::get();
		let slash_debt = weight
			.checked_mul(&DelegationPools::<T, I>::get(commitment_id).slash_per_token)
			.ok_or(Error::<T, I>::CalculationOverflow)?
			/ T::Decimals::get();

		ensure!(
			Delegations::<T, I>::get(&who, commitment_id).is_none(),
			Error::<T, I>::AlreadyDelegating
		);

		Delegations::<T, I>::insert(
			&who,
			commitment_id,
			Stake::new(amount, cooldown_period, allow_auto_compound),
		);
		DelegationPoolMembers::<T, I>::insert(
			&who,
			commitment_id,
			DelegationPoolMember {
				reward_weight: weight,
				slash_weight: weight,
				reward_debt,
				slash_debt,
			},
		);

		// UPDATE per pool weights and global TOTALS
		DelegationPools::<T, I>::try_mutate(commitment_id, |pool| {
			pool.reward_weight = pool
				.reward_weight
				.checked_add(&weight)
				.ok_or(Error::<T, I>::CalculationOverflow)?;
			pool.slash_weight = pool
				.slash_weight
				.checked_add(&weight)
				.ok_or(Error::<T, I>::CalculationOverflow)?;
			Ok(())
		})?;
		// delegator_total += amount
		<DelegatorTotal<T, I>>::try_mutate(who, |s| -> Result<(), Error<T, I>> {
			*s = s.checked_add(&amount).ok_or(Error::<T, I>::CalculationOverflow)?;
			Ok(())
		})?;
		<TotalDelegated<T, I>>::try_mutate(|s| -> Result<(), Error<T, I>> {
			*s = s.checked_add(&amount).ok_or(Error::<T, I>::CalculationOverflow)?;
			Ok(())
		})?;

		Ok(())
	}

	pub fn delegate_more_for(
		who: &T::AccountId,
		commitment_id: T::CommitmentId,
		extra_amount: BalanceFor<T, I>,
	) -> Result<(), Error<T, I>> {
		let old_delegation =
			Self::delegations(who, commitment_id).ok_or(Error::<T, I>::NotDelegating)?;

		let amount = old_delegation
			.amount
			.checked_add(&extra_amount)
			.ok_or(Error::<T, I>::CalculationOverflow)?;

		// TODO: improve this two calls to not unlock and lock the amount unnecessarily
		Self::end_delegation_for(who, commitment_id, false)?;
		Self::delegate_for(
			who,
			commitment_id,
			amount,
			old_delegation.cooldown_period,
			old_delegation.allow_auto_compound,
		)?;

        Ok(())
	}

    pub fn compound_delegator(
		who: &T::AccountId,
		commitment_id: T::CommitmentId,
	) -> Result<(), Error<T, I>> {
        Self::delegate_more_for(who, commitment_id, Self::withdraw_delegator(&who, commitment_id)?.consume())?;

        Ok(())
    }

	pub fn compound_committer(
		committer: &T::AccountId,
		commitment_id: T::CommitmentId,
	) -> Result<(), Error<T, I>> {
		// compound reward to the caller if any
		Self::stake_more_for(committer, Self::withdraw_committer(commitment_id)?.consume())?;

        Ok(())
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

	/// Helper function that allows to both increase and decrease amount in a pool. It accrues outstanding rewards/slashes before re-snapshotting the debts and adapting weights.
	///
	/// - Increase can happen on new delegator joining or staking more.
	/// - Decrease can happen on delegator leaving.
	fn update_commitment_stake(
		commitment_id: T::CommitmentId,
		change: StakeChange<BalanceFor<T, I>>,
	) -> Result<(), Error<T, I>> {
		let updated_commitment_stake = match change {
			StakeChange::Add(amount) => <CommitmentStake<T, I>>::try_mutate(
				commitment_id,
				|s| -> Result<BalanceFor<T, I>, Error<T, I>> {
					*s = s.checked_add(&amount).ok_or(Error::<T, I>::CalculationOverflow)?;
					Ok(s.clone())
				},
			),
			StakeChange::Sub(amount) => <CommitmentStake<T, I>>::try_mutate(
				commitment_id,
				|s| -> Result<BalanceFor<T, I>, Error<T, I>> {
					*s = s.checked_sub(&amount).ok_or(Error::<T, I>::CalculationOverflow)?;
					Ok(s.clone())
				},
			),
		}?;

		let commmitment_cooldown = Stakes::<T, I>::get(&commitment_id)
			.ok_or(Error::<T, I>::CommitmentNotFound)?
			.cooldown_period;
		for (pool_id, metric) in ComputeCommitments::<T, I>::iter_prefix(commitment_id) {
			// This is here solely to ensure we always work on a state where the commitment_id's reward is distributed.
			// Currently dropping reward since we assume we distributed down before coming here, panic in debug builds if not!
			#[cfg(debug_assertions)]
			if StakingPoolMembers::<T, I>::get(commitment_id, pool_id).is_some() {
				let _ = Self::accrue_and_withdraw_commitment(pool_id, commitment_id)?;
			}

			let prev_reward_weight = <StakingPoolMembers<T, I>>::get(commitment_id, pool_id)
				.map(|m| m.reward_weight)
				.unwrap_or(Zero::zero());

			// the following decimal correction is only correct if `T::Decimals` is smaller than 10^18
			const FIXEDU128_DECIMALS: u128 = 1_000_000_000_000_000_000;
			ensure!(
				T::Decimals::get().saturated_into::<u128>() < FIXEDU128_DECIMALS,
				Error::<T, I>::InternalError
			);
			let reward_weight: BalanceFor<T, I> = (((metric.into_inner()
				/ (FIXEDU128_DECIMALS / T::Decimals::get().saturated_into::<u128>()))
			.checked_mul(updated_commitment_stake.saturated_into::<u128>())
			.ok_or(Error::<T, I>::CalculationOverflow)?
				/ T::Decimals::get().saturated_into::<u128>())
			.checked_mul(commmitment_cooldown.saturated_into::<u128>().saturated_into())
			.ok_or(Error::<T, I>::CalculationOverflow)?
				/ T::MaxCooldownPeriod::get().saturated_into::<u128>())
			.saturated_into();

			let reward_debt = reward_weight
				.checked_mul(&StakingPools::<T, I>::get(pool_id).reward_per_token)
				.ok_or(Error::<T, I>::CalculationOverflow)?
				/ T::Decimals::get();

			match change {
				StakeChange::Add(_) => {
					let weight_diff = &reward_weight
						.checked_sub(&prev_reward_weight)
						.ok_or(Error::<T, I>::CalculationOverflow)?;
					StakingPools::<T, I>::try_mutate(pool_id, |pool| {
						pool.reward_weight = pool
							.reward_weight
							.checked_add(weight_diff)
							.ok_or(Error::<T, I>::CalculationOverflow)?;
						Ok(())
					})?;
				},
				StakeChange::Sub(_) => {
					let weight_diff = &prev_reward_weight
						.checked_sub(&reward_weight)
						.ok_or(Error::<T, I>::CalculationOverflow)?;
					StakingPools::<T, I>::try_mutate(pool_id, |pool| {
						pool.reward_weight = pool
							.reward_weight
							.checked_sub(weight_diff)
							.ok_or(Error::<T, I>::CalculationOverflow)?;
						Ok(())
					})?;
				},
			}

			<StakingPoolMembers<T, I>>::insert(
				commitment_id,
				pool_id,
				StakingPoolMember { reward_weight, reward_debt },
			);
		}

		Ok(())
	}

	/// It is guaranteed to withdraw reward only if the result is Ok. If non-zero `Ok(balance)` is returned, this case it has to be futher distributed!
	fn accrue_and_withdraw_commitment(
		pool_id: PoolId,
		commitment_id: T::CommitmentId,
	) -> Result<PendingReward<BalanceFor<T, I>>, Error<T, I>> {
		let pool = StakingPools::<T, I>::get(pool_id);

		StakingPoolMembers::<T, I>::try_mutate(&commitment_id, pool_id, |state_| {
			let state = state_.as_mut().ok_or(Error::<T, I>::CommitmentNotFound)?;
			let reward = state
				.reward_weight
				.checked_mul(&pool.reward_per_token)
				.ok_or(Error::<T, I>::CalculationOverflow)?
				/ T::Decimals::get()
					.checked_sub(&state.reward_debt)
					.ok_or(Error::<T, I>::CalculationOverflow)?;

			state.reward_debt = state
				.reward_weight
				.checked_mul(&pool.reward_per_token)
				.ok_or(Error::<T, I>::CalculationOverflow)?
				/ T::Decimals::get();
			Ok(PendingReward::new(reward))
		})
	}

	/// Applies commission to a reward, adding the commission to the committer's accrued balance
	/// and returning the remaining amount for the delegator.
	fn apply_commission(
		commitment_id: T::CommitmentId,
		reward: BalanceFor<T, I>,
	) -> Result<BalanceFor<T, I>, Error<T, I>> {
		let commission_rate = Commission::<T, I>::get(commitment_id).unwrap_or_default();
		let commission_amount = commission_rate.mul_floor(reward);
		let delegator_reward = reward
			.checked_sub(&commission_amount)
			.ok_or(Error::<T, I>::CalculationOverflow)?;

		// Add commission to committer's accrued reward
		if !commission_amount.is_zero() {
			Stakes::<T, I>::try_mutate(&commitment_id, |committer_stake_| {
				let committer_stake =
					committer_stake_.as_mut().ok_or(Error::<T, I>::CommitmentNotFound)?;
				committer_stake.accrued_reward = committer_stake
					.accrued_reward
					.checked_add(&commission_amount)
					.ok_or(Error::<T, I>::CalculationOverflow)?;
				Ok(())
			})?;
		}

		Ok(delegator_reward)
	}

	/// Accrues into the accrued balances for a delegation.
	fn accrue_delegator(
		who: &T::AccountId,
		commitment_id: T::CommitmentId,
	) -> Result<(), Error<T, I>> {
		Self::distribute_down(commitment_id)?;

		let pool = DelegationPools::<T, I>::get(commitment_id);

		DelegationPoolMembers::<T, I>::try_mutate(who, &commitment_id, |state_| {
			let state = state_.as_mut().ok_or(Error::<T, I>::NotDelegating)?;
			let reward = state
				.reward_weight
				.checked_mul(&pool.reward_per_token)
				.ok_or(Error::<T, I>::CalculationOverflow)?
				/ T::Decimals::get()
					.checked_sub(&state.reward_debt)
					.ok_or(Error::<T, I>::CalculationOverflow)?;
			let slash = state
				.slash_weight
				.checked_mul(&pool.slash_per_token)
				.ok_or(Error::<T, I>::CalculationOverflow)?
				/ T::Decimals::get()
					.checked_sub(&state.slash_debt)
					.ok_or(Error::<T, I>::CalculationOverflow)?;

			// Apply commission and get the delegator's portion
			let delegator_reward = Self::apply_commission(commitment_id, reward)?;

			Delegations::<T, I>::try_mutate(who, &commitment_id, |staker_| {
				let stake = staker_.as_mut().ok_or(Error::<T, I>::NotDelegating)?;
				stake.accrued_reward = stake
					.accrued_reward
					.checked_add(&delegator_reward)
					.ok_or(Error::<T, I>::CalculationOverflow)?;
				state.reward_debt = state
					.reward_weight
					.checked_mul(&pool.reward_per_token)
					.ok_or(Error::<T, I>::CalculationOverflow)?
					/ T::Decimals::get();
				stake.accrued_slash = stake
					.accrued_slash
					.checked_add(&slash)
					.ok_or(Error::<T, I>::CalculationOverflow)?;
				state.slash_debt = state
					.slash_weight
					.checked_mul(&pool.slash_per_token)
					.ok_or(Error::<T, I>::CalculationOverflow)?
					/ T::Decimals::get();
				Ok(())
			})
		})
	}

	/// Accrues into the accrued balances for a self delegation.
	///
	/// This is almost an exact copy of [`Self::accrue_delegator`] but it uses different storage maps for the self-delegations stored by committer (one key only).
	///
	/// TODO: this could be maybe simplified into one helper function since the types in storage are the same
	fn accrue_committer(commitment_id: T::CommitmentId) -> Result<(), Error<T, I>> {
		Self::distribute_down(commitment_id)?;

		let pool = DelegationPools::<T, I>::get(commitment_id);

		SelfDelegation::<T, I>::try_mutate(&commitment_id, |state_| {
			let state = state_.as_mut().ok_or(Error::<T, I>::CommitmentNotFound)?;
			let reward = state
				.reward_weight
				.checked_mul(&pool.reward_per_token)
				.ok_or(Error::<T, I>::CalculationOverflow)?
				/ T::Decimals::get()
					.checked_sub(&state.reward_debt)
					.ok_or(Error::<T, I>::CalculationOverflow)?;
			let slash = state
				.slash_weight
				.checked_mul(&pool.slash_per_token)
				.ok_or(Error::<T, I>::CalculationOverflow)?
				/ T::Decimals::get()
					.checked_sub(&state.slash_debt)
					.ok_or(Error::<T, I>::CalculationOverflow)?;

			Stakes::<T, I>::try_mutate(&commitment_id, |staker_| {
				let stake = staker_.as_mut().ok_or(Error::<T, I>::CommitmentNotFound)?;
				stake.accrued_reward = stake
					.accrued_reward
					.checked_add(&reward)
					.ok_or(Error::<T, I>::CalculationOverflow)?;
				state.reward_debt = state
					.reward_weight
					.checked_mul(&pool.reward_per_token)
					.ok_or(Error::<T, I>::CalculationOverflow)?
					/ T::Decimals::get();
				stake.accrued_slash = stake
					.accrued_slash
					.checked_add(&slash)
					.ok_or(Error::<T, I>::CalculationOverflow)?;
				state.slash_debt = state
					.slash_weight
					.checked_mul(&pool.slash_per_token)
					.ok_or(Error::<T, I>::CalculationOverflow)?
					/ T::Decimals::get();
				Ok(())
			})
		})
	}

	/// It is guaranteed to withdraw reward/slash only if the result is Ok.
	pub fn withdraw_delegator(
		who: &T::AccountId,
		commitment_id: T::CommitmentId,
	) -> Result<PendingReward<BalanceFor<T, I>>, Error<T, I>> {
		Self::accrue_delegator(who, commitment_id)?;

		Delegations::<T, I>::try_mutate(who, &commitment_id, |state_| {
			let state = state_.as_mut().ok_or(Error::<T, I>::CommitmentNotFound)?;
			let r = PendingReward::new(state.accrued_reward);
			// let s = PendingReward::new(state.accrued_slash);
			state.accrued_reward = Zero::zero();
			// state.accrued_slash = Zero::zero();
			Ok(r)
		})
	}

	/// It is guaranteed to withdraw reward/slash only if the result is Ok.
	pub fn withdraw_committer(
		commitment_id: T::CommitmentId,
	) -> Result<PendingReward<BalanceFor<T, I>>, Error<T, I>> {
		Self::accrue_committer(commitment_id)?;

		Stakes::<T, I>::try_mutate(&commitment_id, |state_| {
			let state = state_.as_mut().ok_or(Error::<T, I>::CommitmentNotFound)?;
			let r = PendingReward::new(state.accrued_reward);
			// let s = PendingReward::new(state.accrued_slash);
			state.accrued_reward = Zero::zero();
			// state.accrued_slash = Zero::zero();
			Ok(r)
		})
	}

	pub fn cooldown_stake_for(commitment_id: T::CommitmentId) -> Result<(), Error<T, I>> {
		let current_block = <frame_system::Pallet<T>>::block_number();

		let stake = <Stakes<T, I>>::try_mutate(
			commitment_id,
			|s| -> Result<StakeFor<T, I>, Error<T, I>> {
				let s = s.as_mut().ok_or(Error::<T, I>::CommitmentNotFound)?;
				ensure!(s.cooldown_started.is_none(), Error::<T, I>::CooldownAlreadyStarted);

				s.cooldown_started = Some(current_block);
				// this has to be calculated once and stored, so changes to `T::CooldownRewardRatio` config don't mess up totals
				s.rewardable_amount = T::CooldownRewardRatio::get()
					.mul_floor(s.amount.saturated_into::<u128>())
					.saturated_into();
				Ok(s.clone())
			},
		)?;
		// TODO maybe improve this to be stable under multiple reductions of rewardable_amount (currently never happens)
		let amount_diff = stake
			.amount
			.checked_sub(&stake.rewardable_amount)
			.ok_or(Error::<T, I>::CalculationOverflow)?;

		Self::update_self_delegation(commitment_id)?;
		Self::distribute_down(commitment_id)?;
		Self::update_commitment_stake(commitment_id, StakeChange::Sub(amount_diff))?;

		Ok(())
	}

	/// Starts the cooldown for a delegation.
	///
	/// The delegator's stake stays locked and slashable until the end of cooldown (or longer until he calls `end_delegation`).
	/// Rewards are going to be distributed at a reduced weight, so the commitment's reward_weight in metric pools is already decreased to reflect the reduction, but not so the slash_weight for continuing the ability to slash delegators up to their original stake and, until now, compounded rewards.
	pub fn cooldown_delegation_for(
		who: &T::AccountId,
		commitment_id: T::CommitmentId,
	) -> Result<BlockNumberFor<T>, Error<T, I>> {
		Self::accrue_delegator(who, commitment_id)?;

		// Special case: the commitment delegated to is in cooldown itself (started by committer), or even ended cooldown.
		// In this case the start of the delegator's cooldown is pretended to have occurred at the staker's start of cooldown,
		// as a lazy imitation of starting all delegator's cooldown together with commitment cooldown.
		let stake = <Stakes<T, I>>::get(commitment_id).ok_or(Error::<T, I>::CommitmentNotFound)?;
		let cooldown_start = if let Some(c) = stake.cooldown_started {
			c
		} else {
			<frame_system::Pallet<T>>::block_number()
		};

		let stake = <Delegations<T, I>>::try_mutate(
			who,
			commitment_id,
			|d| -> Result<StakeFor<T, I>, Error<T, I>> {
				let s = d.as_mut().ok_or(Error::<T, I>::NotDelegating)?;
				ensure!(s.cooldown_started.is_none(), Error::<T, I>::CooldownAlreadyStarted);
				s.cooldown_started = Some(cooldown_start);
				// this has to be calculated once and stored, so changes to `T::CooldownRewardRatio` config don't mess up totals
				s.rewardable_amount = T::CooldownRewardRatio::get()
					.mul_floor(s.amount.saturated_into::<u128>())
					.saturated_into();
				Ok(s.clone())
			},
		)?;
		// TODO maybe improve this to be stable under multiple reductions of rewardable_amount (currently never happens)
		let amount_diff = stake
			.amount
			.checked_sub(&stake.rewardable_amount)
			.ok_or(Error::<T, I>::CalculationOverflow)?;

		Self::distribute_down(commitment_id)?;
		Self::update_commitment_stake(commitment_id, StakeChange::Sub(amount_diff))?;

		let reward_weight_diff = DelegationPoolMembers::<T, I>::try_mutate(
			&who,
			commitment_id,
			|d| -> Result<BalanceFor<T, I>, Error<T, I>> {
				let m = d.as_mut().ok_or(Error::<T, I>::NotDelegating)?;
				let prev_reward_weight = m.reward_weight;
				m.reward_weight = stake
					.rewardable_amount
					.checked_mul(&stake.cooldown_period.saturated_into::<u128>().saturated_into())
					.ok_or(Error::<T, I>::CalculationOverflow)?
					/ T::MaxCooldownPeriod::get().saturated_into::<u128>().saturated_into();
				ensure!(m.reward_debt.is_zero(), Error::<T, I>::InternalError);
				m.reward_debt = m
					.reward_weight
					.checked_mul(&DelegationPools::<T, I>::get(commitment_id).reward_per_token)
					.ok_or(Error::<T, I>::CalculationOverflow)?
					/ T::Decimals::get();
				Ok(prev_reward_weight
					.checked_sub(&m.reward_weight)
					.ok_or(Error::<T, I>::CalculationOverflow)?)
			},
		)?;

		// UPDATE per pool weights (not yet global totals, only when ending delegation)
		DelegationPools::<T, I>::try_mutate(commitment_id, |pool| {
			pool.reward_weight = pool
				.reward_weight
				.checked_sub(&reward_weight_diff)
				.ok_or(Error::<T, I>::CalculationOverflow)?;
			Ok(())
		})?;

		Ok(cooldown_start)
	}

	pub fn redelegate_for(
		who: &T::AccountId,
		old_commitment_id: T::CommitmentId,
		new_commitment_id: T::CommitmentId,
	) -> Result<(), DispatchError> {
		// Check if the caller is a delegator to the old commitment
		let old_delegation = <Delegations<T, I>>::get(who, old_commitment_id)
			.ok_or(Error::<T, I>::NotDelegating)?;

		// Check if the old commitment is in cooldown but the delegator is not
		let old_commitment_stake =
			Self::stakes(old_commitment_id).ok_or(Error::<T, I>::CommitmentNotFound)?;

		// Check if old commitment is in cooldown
		ensure!(
			old_commitment_stake.cooldown_started.is_some(),
			Error::<T, I>::CommitmentNotInCooldown
		);

		// Check if delegator is NOT in cooldown
		ensure!(old_delegation.cooldown_started.is_none(), Error::<T, I>::DelegatorInCooldown);

		// Check if new committer has more own stake than the old one
		let old_cooldown = old_commitment_stake.cooldown_period;
		let new_cooldown = Self::stakes(new_commitment_id)
			.ok_or(Error::<T, I>::NewCommitmentNotFound)?
			.cooldown_period;
		let old_delegation =
			Self::delegations(who, old_commitment_id).ok_or(Error::<T, I>::NotDelegating)?;

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
		Self::end_delegation_for(who, old_commitment_id, false)?;
		Self::delegate_for(
			who,
			new_commitment_id,
			old_delegation.amount,
			old_delegation.cooldown_period,
			old_delegation.allow_auto_compound,
		)?;

		Ok(())
	}

	pub fn end_delegation_for(
		who: &T::AccountId,
		commitment_id: T::CommitmentId,
		check_cooldown: bool,
	) -> Result<BalanceFor<T, I>, Error<T, I>> {
		let current_block = <frame_system::Pallet<T>>::block_number();

		let stake = <Delegations<T, I>>::try_mutate(
			who,
			commitment_id,
			|s_| -> Result<StakeFor<T, I>, Error<T, I>> {
				let s = s_.as_mut().ok_or(Error::<T, I>::NotDelegating)?;
				if check_cooldown {
					let cooldown_start =
						s.cooldown_started.ok_or(Error::<T, I>::CooldownNotStarted)?;
					ensure!(
						cooldown_start.saturating_add(s.cooldown_period) >= current_block,
						Error::<T, I>::CooldownNotEnded
					);
				}

				Ok(s_.take().unwrap())
			},
		)?;

		let state = <DelegationPoolMembers<T, I>>::try_mutate(
			who,
			commitment_id,
			|s_| -> Result<DelegationPoolMemberFor<T, I>, Error<T, I>> {
				s_.as_mut().ok_or(Error::<T, I>::NotDelegating)?;
				Ok(s_.take().unwrap())
			},
		)?;

		Self::distribute_down(commitment_id)?;
		Self::update_commitment_stake(commitment_id, StakeChange::Sub(stake.rewardable_amount))?;
		Self::update_total_stake(StakeChange::Sub(stake.rewardable_amount))?;

		// UPDATE per pool and global TOTALS
		DelegationPools::<T, I>::try_mutate(commitment_id, |pool| {
			pool.reward_weight = pool
				.reward_weight
				.checked_sub(&state.reward_weight)
				.ok_or(Error::<T, I>::CalculationOverflow)?;
			pool.slash_weight = pool
				.slash_weight
				.checked_sub(&state.slash_weight)
				.ok_or(Error::<T, I>::CalculationOverflow)?;
			Ok(())
		})?;
		// delegator_total -= amount
		<DelegatorTotal<T, I>>::try_mutate(who, |s| -> Result<(), Error<T, I>> {
			*s = s.checked_sub(&stake.amount).ok_or(Error::<T, I>::CalculationOverflow)?;
			Ok(())
		})?;
		<TotalDelegated<T, I>>::try_mutate(|s| -> Result<(), Error<T, I>> {
			*s = s.checked_sub(&stake.amount).ok_or(Error::<T, I>::CalculationOverflow)?;
			Ok(())
		})?;

		Self::unlock_funds(&who, stake.amount);

		Ok(stake.amount)
	}

	pub fn unstake_for(
		who: &T::AccountId,
		commitment_id: T::CommitmentId,
	) -> Result<BalanceFor<T, I>, Error<T, I>> {
		let current_block = <frame_system::Pallet<T>>::block_number();

		let stake = <Stakes<T, I>>::try_mutate(
			commitment_id,
			|s_| -> Result<StakeFor<T, I>, Error<T, I>> {
				let s = s_.as_mut().ok_or(Error::<T, I>::CommitmentNotFound)?;
				let cooldown_start = s.cooldown_started.ok_or(Error::<T, I>::CooldownNotStarted)?;
				ensure!(
					cooldown_start.saturating_add(s.cooldown_period) >= current_block,
					Error::<T, I>::CooldownNotEnded
				);

				Ok(s_.take().unwrap())
			},
		)?;

		<SelfDelegation<T, I>>::remove(commitment_id);

		Self::distribute_down(commitment_id)?;
		Self::update_commitment_stake(commitment_id, StakeChange::Sub(stake.amount))?;
		Self::update_total_stake(StakeChange::Sub(stake.amount))?;

		Self::unlock_funds(who, stake.amount);

		// TODO clear all delegators? their cooldown must have ended (imposed by committer cooling down)

		Ok(stake.amount)
	}

	/// Force-unstakes a commitment by removing all delegations and the commitment's own stake.
	/// This bypasses normal cooldown and validation checks.
	pub fn force_unstake_for(commitment_id: T::CommitmentId) -> Result<(), DispatchError> {
		// Calculate total amounts to be removed for updating global totals
		let mut total_delegation_amount: BalanceFor<T, I> = Zero::zero();
		let mut total_stake_amount: BalanceFor<T, I> = Zero::zero();

		// Find all delegations to this commitment and remove them
		let all_delegations: Vec<(T::AccountId, T::CommitmentId, _)> =
			<Delegations<T, I>>::iter().collect();
		for (delegator, delegation_commitment_id, stake) in all_delegations {
			if delegation_commitment_id == commitment_id {
				// Update delegator totals
				<DelegatorTotal<T, I>>::try_mutate(&delegator, |s| -> Result<(), Error<T, I>> {
					*s = s.checked_sub(&stake.amount).ok_or(Error::<T, I>::CalculationOverflow)?;
					Ok(())
				})?;

				// Remove delegation pool member
				<DelegationPoolMembers<T, I>>::remove(&delegator, commitment_id);

				// Unlock funds for delegator
				Self::unlock_funds(&delegator, stake.amount);

				// Add to total delegation amount
				total_delegation_amount = total_delegation_amount
					.checked_add(&stake.amount)
					.ok_or(Error::<T, I>::CalculationOverflow)?;

				// Remove this specific delegation
				<Delegations<T, I>>::remove(&delegator, commitment_id);
			}
		}

		// Update total delegated
		<TotalDelegated<T, I>>::try_mutate(|s| -> Result<(), Error<T, I>> {
			*s = s
				.checked_sub(&total_delegation_amount)
				.ok_or(Error::<T, I>::CalculationOverflow)?;
			Ok(())
		})?;

		// Remove commitment's own stake
		if let Some(stake) = <Stakes<T, I>>::take(commitment_id) {
			total_stake_amount = stake.amount;
		}

		// Calculate total amount to remove from global stake
		let total_amount_to_remove = total_delegation_amount
			.checked_add(&total_stake_amount)
			.ok_or(Error::<T, I>::CalculationOverflow)?;

		// Update global totals
		if !total_amount_to_remove.is_zero() {
			<TotalStake<T, I>>::try_mutate(|s| -> Result<(), Error<T, I>> {
				*s = s
					.checked_sub(&total_amount_to_remove)
					.ok_or(Error::<T, I>::CalculationOverflow)?;
				Ok(())
			})?;
		}

		// Remove self-delegation
		<SelfDelegation<T, I>>::remove(commitment_id);

		// Remove compute commitments
		let _ = <ComputeCommitments<T, I>>::clear_prefix(commitment_id, u32::MAX, None);

		// Remove staking pool members
		let _ = <StakingPoolMembers<T, I>>::clear_prefix(commitment_id, u32::MAX, None);

		// Remove delegation pool
		<DelegationPools<T, I>>::remove(commitment_id);

		// Update commitment stake to zero (this should be last)
		<CommitmentStake<T, I>>::remove(commitment_id);

		Ok(())
	}

	fn delegation_ratio(commitment_id: T::CommitmentId) -> Perquintill {
		let denominator: u128 = <CommitmentStake<T, I>>::get(commitment_id).saturated_into();
		let reciprocal_nominator: u128 = <Stakes<T, I>>::get(commitment_id)
			.map(|s| s.amount)
			.unwrap_or(Zero::zero())
			.saturated_into();
		let nominator = denominator.saturating_sub(reciprocal_nominator);
		if denominator > 0 {
			Perquintill::from_rational(nominator, denominator)
		} else {
			Perquintill::zero()
		}
	}

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
					<Stakes<T, I>>::get(delegator_commitment_id).map(|s| s.amount)
				} else {
					None
				}
				.unwrap_or(Zero::zero());
				let delegated = <Delegations<T, I>>::get(who, commitment_id)
					.map(|d| d.amount)
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
			&who,
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
				&who,
				new_total_stake.saturated_into(),
				WithdrawReasons::all(),
			);
		}
	}
}
