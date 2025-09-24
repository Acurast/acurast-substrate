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
	traits::{AccountIdConversion, CheckedAdd, CheckedSub, Saturating, Zero},
	Perquintill, SaturatedConversion,
};
use sp_std::vec::Vec;

use crate::types::PER_TOKEN_DECIMALS;
use crate::*;

/// Helper trait for ceiling division that rounds up instead of down
trait CheckedDivCeil<Rhs = Self> {
	type Output;

	/// Performs ceiling division (rounding up) using checked arithmetic.
	/// Returns `None` if the division would overflow or if the divisor is zero.
	fn checked_div_ceil(&self, divisor: &Rhs) -> Option<Self::Output>;
}

// impl<P> CheckedDivCeil for P
// where
// 	P: CheckedDiv<Output = P> + CheckedAdd<Output = P> + CheckedSub<Output = P> + One + Zero + Copy,
// {
// 	type Output = P;

// 	fn checked_div_ceil(&self, divisor: &P) -> Option<P> {
// 		if divisor.is_zero() {
// 			return None;
// 		}

// 		// Formula: (numerator + divisor - 1) / divisor
// 		// This rounds up any remainder to the next integer
// 		let numerator_adjusted = self.checked_add(divisor)?.checked_sub(&P::one())?;

// 		numerator_adjusted.checked_div(divisor)
// 	}
// }

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
	Sub(Balance, Balance),
}

impl<T: Config<I>, I: 'static> Pallet<T, I>
where
	BalanceFor<T, I>: From<u128>,
{
	pub fn distribute(epoch: EpochOf<T>, amount: BalanceFor<T, I>) -> Result<(), Error<T, I>> {
		for (pool_id, pool) in MetricPools::<T, I>::iter() {
			let a: u128 = amount.saturated_into();
			Self::distribute_top(
				pool_id,
				pool.reward.get(epoch).mul_floor(a.saturated_into::<u128>()).saturated_into(),
			)?;
		}

		Ok(())
	}

	fn distribute_top(pool_id: PoolId, amount: BalanceFor<T, I>) -> Result<(), Error<T, I>> {
		StakingPools::<T, I>::try_mutate(pool_id, |pool| {
			if !pool.reward_weight.is_zero() {
				let extra = U256::from(amount.saturated_into::<u128>())
					.checked_mul(U256::from(PER_TOKEN_DECIMALS))
					.ok_or(Error::<T, I>::CalcDistributeTop1)?
					.checked_div(pool.reward_weight)
					.ok_or(Error::<T, I>::CalcDistributeTop2)?;
				// NOTE: reward_per_token is used as reward_per_weight
				pool.reward_per_token = pool
					.reward_per_token
					.checked_add(extra)
					.ok_or(Error::<T, I>::CalcDistributeTop3)?;
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
		reward: BalanceFor<T, I>,
	) -> Result<(), Error<T, I>> {
		if reward.is_zero() {
			return Ok(());
		}
		DelegationPools::<T, I>::try_mutate(commitment_id, |pool| -> Result<(), Error<T, I>> {
			if !pool.reward_weight.is_zero() {
				let extra = U256::from(reward.saturated_into::<u128>())
					.checked_mul(U256::from(PER_TOKEN_DECIMALS))
					.ok_or(Error::<T, I>::CalcRewardDelPool1)?
					.checked_div(pool.reward_weight)
					.ok_or(Error::<T, I>::CalcRewardDelPool2)?;
				// NOTE: reward_per_token is used as reward_per_weight
				pool.reward_per_token = pool
					.reward_per_token
					.checked_add(extra)
					.ok_or(Error::<T, I>::CalcRewardDelPool3)?;
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
				let extra = U256::from(amount.saturated_into::<u128>())
					.checked_mul(U256::from(PER_TOKEN_DECIMALS))
					.ok_or(Error::<T, I>::CalcSlashDelPool1)?
					.checked_div(pool.slash_weight)
					.ok_or(Error::<T, I>::CalcSlashDelPool2)?;
				pool.slash_per_token = pool
					.slash_per_token
					.checked_add(extra)
					.ok_or(Error::<T, I>::CalcSlashDelPool3)?;
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
		Self::lock_funds(who, amount, LockReason::Staking)?;

		let commitment_id = T::CommitmentIdProvider::commitment_id_for(who)
			.map_err(|_| Error::<T, I>::NoOwnerOfCommitmentId)?;

		let stake = <Stakes<T, I>>::try_mutate(
			commitment_id,
			|state| -> Result<StakeFor<T, I>, Error<T, I>> {
				ensure!(state.is_none(), Error::<T, I>::AlreadyCommitted);
				ensure!(
					Self::commitment_stake(commitment_id).0.is_zero(),
					Error::<T, I>::EndStaleDelegationsFirst
				);

				let s = Stake::new(
					amount,
					<frame_system::Pallet<T>>::block_number(),
					cooldown_period,
					allow_auto_compound,
				);
				*state = Some(s.clone());
				Ok(s)
			},
		)?;

		let d = Self::update_self_delegation(commitment_id)?;
		DelegationPools::<T, I>::try_mutate(commitment_id, |pool| {
			pool.reward_weight = pool
				.reward_weight
				.checked_add(d.reward_weight)
				.ok_or(Error::<T, I>::CalcUpdateSelfDel9)?;
			pool.slash_weight = pool
				.slash_weight
				.checked_add(d.slash_weight)
				.ok_or(Error::<T, I>::CalcUpdateSelfDel10)?;
			Ok(())
		})?;
		Self::update_commitment_stake(commitment_id, StakeChange::Add(stake.amount), &stake)?;
		Self::update_total_stake(StakeChange::Add(stake.amount))?;

		Ok(())
	}

	fn update_self_delegation(
		commitment_id: T::CommitmentId,
	) -> Result<DelegationPoolMemberFor<T, I>, Error<T, I>> {
		let stake = <Stakes<T, I>>::get(commitment_id).ok_or(Error::<T, I>::CommitmentNotFound)?;

		// committer (via commitment_id) also joins the delegation pool
		// reward_weight reduces during cooldown
		let reward_weight = U256::from(stake.rewardable_amount.saturated_into::<u128>())
			.checked_mul(U256::from(stake.cooldown_period.saturated_into::<u128>()))
			.ok_or(Error::<T, I>::CalcUpdateSelfDel1)?
			.checked_div(U256::from(T::MaxCooldownPeriod::get().saturated_into::<u128>()))
			.ok_or(Error::<T, I>::CalcUpdateSelfDel2)?;

		// slash weight remains always like initial, also during cooldown
		let slash_weight = U256::from(stake.amount.saturated_into::<u128>())
			.checked_mul(U256::from(stake.cooldown_period.saturated_into::<u128>()))
			.ok_or(Error::<T, I>::CalcUpdateSelfDel3)?
			.checked_div(U256::from(T::MaxCooldownPeriod::get().saturated_into::<u128>()))
			.ok_or(Error::<T, I>::CalcUpdateSelfDel4)?;

		let reward_debt_u256 = reward_weight
			.checked_mul(DelegationPools::<T, I>::get(commitment_id).reward_per_token)
			.ok_or(Error::<T, I>::CalcUpdateSelfDel5)?
			.checked_div_ceil(&U256::from(PER_TOKEN_DECIMALS))
			.ok_or(Error::<T, I>::CalcUpdateSelfDel6)?;
		let reward_debt: BalanceFor<T, I> = reward_debt_u256.as_u128().into();

		let slash_debt_u256 = slash_weight
			.checked_mul(DelegationPools::<T, I>::get(commitment_id).slash_per_token)
			.ok_or(Error::<T, I>::CalcUpdateSelfDel7)?
			.checked_div(U256::from(PER_TOKEN_DECIMALS))
			.ok_or(Error::<T, I>::CalcUpdateSelfDel8)?;
		let slash_debt: BalanceFor<T, I> = slash_debt_u256.as_u128().into();
		let d = DelegationPoolMember { reward_weight, slash_weight, reward_debt, slash_debt };
		<SelfDelegation<T, I>>::insert(commitment_id, &d);

		Ok(d)
	}

	/// Stakes an extra (additional) amount towards an existing commitment.
	pub fn stake_more_for(
		who: &T::AccountId,
		extra_amount: BalanceFor<T, I>,
	) -> Result<(), Error<T, I>> {
		ensure!(!extra_amount.is_zero(), Error::<T, I>::MinStakeSubceeded);

		let commitment_id = T::CommitmentIdProvider::commitment_id_for(who)
			.map_err(|_| Error::<T, I>::NoOwnerOfCommitmentId)?;

		Self::accrue_committer(commitment_id)?;

		let stake = <Stakes<T, I>>::try_mutate(
			commitment_id,
			|stake_| -> Result<StakeFor<T, I>, Error<T, I>> {
				let stake = stake_.as_mut().ok_or(Error::<T, I>::CommitmentNotFound)?;
				ensure!(stake.cooldown_started.is_none(), Error::<T, I>::CommitmentInCooldown);
				stake.amount = stake
					.amount
					.checked_add(&extra_amount)
					.ok_or(Error::<T, I>::CalcCommitStake1)?;
				stake.rewardable_amount = stake.amount;

				// locking is on accounts (while all other storage points are relative to `commitment_id`)
				Self::lock_funds(who, stake.amount, LockReason::Staking)?;

				Ok(stake.clone())
			},
		)?;

		let prev_d = <SelfDelegation<T, I>>::get(commitment_id).unwrap_or_default();
		let d = Self::update_self_delegation(commitment_id)?;
		let reward_weight_diff = d
			.reward_weight
			.checked_sub(prev_d.reward_weight)
			.ok_or(Error::<T, I>::CalcCommitStake2)?;
		let slash_weight_diff = d
			.slash_weight
			.checked_sub(prev_d.slash_weight)
			.ok_or(Error::<T, I>::CalcCommitStake3)?;
		DelegationPools::<T, I>::try_mutate(commitment_id, |pool| {
			pool.reward_weight = pool
				.reward_weight
				.checked_add(reward_weight_diff)
				.ok_or(Error::<T, I>::CalcCommitStake4)?;
			pool.slash_weight = pool
				.slash_weight
				.checked_add(slash_weight_diff)
				.ok_or(Error::<T, I>::CalcCommitStake5)?;
			Ok(())
		})?;

		Self::update_commitment_stake(commitment_id, StakeChange::Add(extra_amount), &stake)?;
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
			Stakes::<T, I>::get(commitment_id).ok_or(Error::<T, I>::CommitmentNotFound)?;
		ensure!(
			cooldown_period <= committer_stake.cooldown_period,
			Error::<T, I>::DelegationCooldownMustBeShorterThanCommitment
		);

		Self::lock_funds(who, amount, LockReason::Delegation(commitment_id))?;

		Self::distribute_down(commitment_id)?;
		Self::update_commitment_stake(commitment_id, StakeChange::Add(amount), &committer_stake)?;
		Self::update_total_stake(StakeChange::Add(amount))?;

		// This check has to happen after `update_commitment_stake` to ensure the commitment_stake is updated what it would be after this call completed and is not rolled back
		ensure!(
			Self::delegation_ratio(commitment_id) <= T::MaxDelegationRatio::get(),
			Error::<T, I>::MaxDelegationRatioExceeded
		);

		let weight_u256 = U256::from(amount.saturated_into::<u128>())
			.checked_mul(U256::from(cooldown_period.saturated_into::<u128>()))
			.ok_or(Error::<T, I>::CalcDelegateFor1)?
			.checked_div(U256::from(T::MaxCooldownPeriod::get().saturated_into::<u128>()))
			.ok_or(Error::<T, I>::CalcDelegateFor2)?;
		let weight = weight_u256;

		let reward_debt_u256 = weight_u256
			.checked_mul(DelegationPools::<T, I>::get(commitment_id).reward_per_token)
			.ok_or(Error::<T, I>::CalcDelegateFor3)?
			.checked_div_ceil(&U256::from(PER_TOKEN_DECIMALS))
			.ok_or(Error::<T, I>::CalcDelegateFor4)?;
		let reward_debt: BalanceFor<T, I> = reward_debt_u256.as_u128().into();

		let slash_debt_u256 = weight_u256
			.checked_mul(DelegationPools::<T, I>::get(commitment_id).slash_per_token)
			.ok_or(Error::<T, I>::CalcDelegateFor5)?
			.checked_div(U256::from(PER_TOKEN_DECIMALS))
			.ok_or(Error::<T, I>::CalcDelegateFor6)?;
		let slash_debt: BalanceFor<T, I> = slash_debt_u256.as_u128().into();

		ensure!(
			Delegations::<T, I>::get(who, commitment_id).is_none(),
			Error::<T, I>::AlreadyDelegating
		);

		Delegations::<T, I>::insert(
			who,
			commitment_id,
			Stake::new(
				amount,
				<frame_system::Pallet<T>>::block_number(),
				cooldown_period,
				allow_auto_compound,
			),
		);
		DelegationPoolMembers::<T, I>::insert(
			who,
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
			pool.reward_weight =
				pool.reward_weight.checked_add(weight).ok_or(Error::<T, I>::CalcDelegateFor7)?;
			pool.slash_weight =
				pool.slash_weight.checked_add(weight).ok_or(Error::<T, I>::CalcDelegateFor8)?;
			Ok(())
		})?;
		// delegator_total += amount
		<DelegatorTotal<T, I>>::try_mutate(who, |s| -> Result<(), Error<T, I>> {
			*s = s.checked_add(&amount).ok_or(Error::<T, I>::CalcDelegateFor9)?;
			Ok(())
		})?;
		<TotalDelegated<T, I>>::try_mutate(|s| -> Result<(), Error<T, I>> {
			*s = s.checked_add(&amount).ok_or(Error::<T, I>::CalcDelegateFor10)?;
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

		let committer_stake =
			Stakes::<T, I>::get(commitment_id).ok_or(Error::<T, I>::CommitmentNotFound)?;

		// We error out if the existing delegation was for a previous commitment that got ended and "replaced" by a new commitment by same committer.
		// In this case the delegator needs to end (or redelegate) his delegation first.
		ensure!(
			old_delegation.created >= committer_stake.created,
			Error::<T, I>::StaleDelegationMustBeEnded
		);

		let amount = old_delegation
			.amount
			.checked_add(&extra_amount)
			.ok_or(Error::<T, I>::CalcDelegateMoreFor1)?;

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
		Self::delegate_more_for(
			who,
			commitment_id,
			Self::withdraw_delegator_accrued(who, commitment_id)?,
		)?;

		Ok(())
	}

	pub fn compound_committer(
		committer: &T::AccountId,
		commitment_id: T::CommitmentId,
	) -> Result<(), Error<T, I>> {
		// compound reward to the caller if any
		Self::stake_more_for(committer, Self::withdraw_committer_accrued(commitment_id)?)?;

		Ok(())
	}

	fn update_total_stake(change: StakeChange<BalanceFor<T, I>>) -> Result<(), Error<T, I>> {
		match change {
			StakeChange::Add(amount) => {
				<TotalStake<T, I>>::try_mutate(|s| -> Result<(), Error<T, I>> {
					*s = s.checked_add(&amount).ok_or(Error::<T, I>::CalcUpdateTotalStake1)?;
					Ok(())
				})
			},
			StakeChange::Sub(amount, _) => {
				<TotalStake<T, I>>::try_mutate(|s| -> Result<(), Error<T, I>> {
					*s = s.checked_sub(&amount).ok_or(Error::<T, I>::CalcUpdateTotalStake2)?;
					Ok(())
				})
			},
		}
	}

	/// Helper function that updates both amount and rewardable_amount in CommitmentStake.
	/// Used when actual stake amounts change (new stake, ending delegation, etc.)
	fn update_commitment_stake(
		commitment_id: T::CommitmentId,
		change: StakeChange<BalanceFor<T, I>>,
		stake: &StakeFor<T, I>,
	) -> Result<BalanceFor<T, I>, Error<T, I>> {
		let (total_amount, updated_rewardable_amount) = match change {
			StakeChange::Add(amount) => <CommitmentStake<T, I>>::try_mutate(
				commitment_id,
				|(total_amount, rewardable_amount)| -> Result<(BalanceFor<T, I>, BalanceFor<T, I>), Error<T, I>> {
					*total_amount = total_amount
						.checked_add(&amount)
						.ok_or(Error::<T, I>::CalcUpdateCommitStake1)?;
					*rewardable_amount = rewardable_amount
						.checked_add(&amount)
						.ok_or(Error::<T, I>::CalcUpdateCommitStake2)?;
					Ok((*total_amount, *rewardable_amount))
				},
			),
			StakeChange::Sub(diff, rewardable_diff) => <CommitmentStake<T, I>>::try_mutate(
				commitment_id,
				|(total_amount, rewardable_amount)| -> Result<(BalanceFor<T, I>, BalanceFor<T, I>), Error<T, I>> {
					*total_amount = total_amount
						.checked_sub(&diff)
						.ok_or(Error::<T, I>::CalcUpdateCommitStake3)?;
					*rewardable_amount = rewardable_amount
						.checked_sub(&rewardable_diff)
						.ok_or(Error::<T, I>::CalcUpdateCommitStake4)?;
					Ok((*total_amount, *rewardable_amount))
				},
			),
		}?;
		Self::update_commitment_pool_weight(commitment_id, updated_rewardable_amount, stake)?;
		Ok(total_amount)
	}

	/// Updates pool weights for a commitment based on rewardable amount.
	fn update_commitment_pool_weight(
		commitment_id: T::CommitmentId,
		updated_rewardable_amount: BalanceFor<T, I>,
		stake: &StakeFor<T, I>,
	) -> Result<(), Error<T, I>> {
		let commmitment_cooldown = stake.cooldown_period;
		// the Compute Commitment might have ended but we make sure the compute commitments are still around
		for (pool_id, metric) in ComputeCommitments::<T, I>::iter_prefix(commitment_id) {
			// This is here solely to ensure we always work on a state where the commitment_id's reward is distributed.
			#[cfg(debug_assertions)]
			if StakingPoolMembers::<T, I>::get(commitment_id, pool_id).is_some() {
				ensure!(
					Self::accrue_and_withdraw_commitment(pool_id, commitment_id)?.is_zero(),
					Error::<T, I>::InternalError
				);
			}

			let prev_reward_weight = <StakingPoolMembers<T, I>>::get(commitment_id, pool_id)
				.map(|m| m.reward_weight)
				.unwrap_or(Zero::zero());

			// Convert calculations to use U256 for precision
			const FIXEDU128_DECIMALS: u128 = 1_000_000_000_000_000_000;

			// reward_weight = metric * amount * cooldown / max_cooldown
			let reward_weight = U256::from(metric.into_inner())
				.checked_mul(U256::from(updated_rewardable_amount.saturated_into::<u128>()))
				.ok_or(Error::<T, I>::CalcUpdateCommitPoolWeight1)?
				.checked_mul(U256::from(commmitment_cooldown.saturated_into::<u128>()))
				.ok_or(Error::<T, I>::CalcUpdateCommitPoolWeight2)?
				.checked_div(U256::from(T::MaxCooldownPeriod::get().saturated_into::<u128>()))
				.ok_or(Error::<T, I>::CalcUpdateCommitPoolWeight3)?
				.checked_div(U256::from(FIXEDU128_DECIMALS))
				.ok_or(Error::<T, I>::CalcUpdateCommitPoolWeight4)?;

			let reward_debt = reward_weight
				.checked_mul(StakingPools::<T, I>::get(pool_id).reward_per_token)
				.ok_or(Error::<T, I>::CalcUpdateCommitPoolWeight5)?
				.checked_div_ceil(&U256::from(PER_TOKEN_DECIMALS))
				.ok_or(Error::<T, I>::CalcUpdateCommitPoolWeight6)?;

			// Update pool weight based on difference between new and previous reward weight
			if reward_weight >= prev_reward_weight {
				let weight_diff = reward_weight
					.checked_sub(prev_reward_weight)
					.ok_or(Error::<T, I>::CalcUpdateCommitPoolWeight7)?;
				StakingPools::<T, I>::try_mutate(pool_id, |pool| {
					pool.reward_weight = pool
						.reward_weight
						.checked_add(weight_diff)
						.ok_or(Error::<T, I>::CalcUpdateCommitPoolWeight8)?;
					Ok(())
				})?;
			} else {
				let weight_diff = prev_reward_weight
					.checked_sub(reward_weight)
					.ok_or(Error::<T, I>::CalcUpdateCommitPoolWeight9)?;
				StakingPools::<T, I>::try_mutate(pool_id, |pool| {
					pool.reward_weight = pool
						.reward_weight
						.checked_sub(weight_diff)
						.ok_or(Error::<T, I>::CalcUpdateCommitPoolWeight10)?;
					Ok(())
				})?;
			}

			if reward_weight.is_zero() && reward_debt.is_zero() {
				<StakingPoolMembers<T, I>>::remove(commitment_id, pool_id);
			} else {
				<StakingPoolMembers<T, I>>::insert(
					commitment_id,
					pool_id,
					StakingPoolMember { reward_weight, reward_debt },
				);
			}
		}

		Ok(())
	}

	/// It is guaranteed to withdraw reward only if the result is Ok. If non-zero `Ok(balance)` is returned, this case it has to be futher distributed!
	fn accrue_and_withdraw_commitment(
		pool_id: PoolId,
		commitment_id: T::CommitmentId,
	) -> Result<BalanceFor<T, I>, Error<T, I>> {
		let pool = StakingPools::<T, I>::get(pool_id);

		StakingPoolMembers::<T, I>::try_mutate(commitment_id, pool_id, |state_| {
			let state = state_.as_mut().ok_or(Error::<T, I>::CommitmentNotFound)?;
			let reward_u256 = state
				.reward_weight
				.checked_mul(pool.reward_per_token)
				.ok_or(Error::<T, I>::CalcAccrueCommit1)?
				.checked_div(U256::from(PER_TOKEN_DECIMALS))
				.ok_or(Error::<T, I>::CalcAccrueCommit2)?;
			let reward_amount: BalanceFor<T, I> = reward_u256.as_u128().into();
			let prev_reward_debt_balance: BalanceFor<T, I> = state.reward_debt.as_u128().into();
			let reward = reward_amount.saturating_sub(prev_reward_debt_balance);

			state.reward_debt = state
				.reward_weight
				.checked_mul(pool.reward_per_token)
				.ok_or(Error::<T, I>::CalcAccrueCommit3)?
				.checked_div_ceil(&U256::from(PER_TOKEN_DECIMALS))
				.ok_or(Error::<T, I>::CalcAccrueCommit4)?;
			Ok(reward)
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
			.ok_or(Error::<T, I>::CalcApplyCommission1)?;

		// Add commission to committer's accrued reward
		if !commission_amount.is_zero() {
			Stakes::<T, I>::try_mutate(commitment_id, |committer_stake_| {
				let committer_stake =
					committer_stake_.as_mut().ok_or(Error::<T, I>::CommitmentNotFound)?;
				committer_stake.accrued_reward = committer_stake
					.accrued_reward
					.checked_add(&commission_amount)
					.ok_or(Error::<T, I>::CalcApplyCommission2)?;
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

		DelegationPoolMembers::<T, I>::try_mutate(who, commitment_id, |state_| {
			let state = state_.as_mut().ok_or(Error::<T, I>::NotDelegating)?;
			let reward_u256 = U256::from(state.reward_weight.saturated_into::<u128>())
				.checked_mul(U256::from(pool.reward_per_token.saturated_into::<u128>()))
				.ok_or(Error::<T, I>::CalcAccrueDelegator1)?
				.checked_div(U256::from(PER_TOKEN_DECIMALS))
				.ok_or(Error::<T, I>::CalcAccrueDelegator2)?;
			let reward: BalanceFor<T, I> = reward_u256.as_u128().into();
			let reward = reward.saturating_sub(state.reward_debt);

			let slash_u256 = U256::from(state.slash_weight.saturated_into::<u128>())
				.checked_mul(U256::from(pool.slash_per_token.saturated_into::<u128>()))
				.ok_or(Error::<T, I>::CalcAccrueDelegator3)?
				.checked_div(U256::from(PER_TOKEN_DECIMALS))
				.ok_or(Error::<T, I>::CalcAccrueDelegator4)?;
			let slash: BalanceFor<T, I> = slash_u256.as_u128().into();
			let slash = slash
				.checked_sub(&state.slash_debt)
				.ok_or(Error::<T, I>::CalcAccrueDelegator5)?;

			// Apply commission and get the delegator's portion
			let delegator_reward = Self::apply_commission(commitment_id, reward)?;

			Delegations::<T, I>::try_mutate(who, commitment_id, |staker_| {
				let stake = staker_.as_mut().ok_or(Error::<T, I>::NotDelegating)?;
				stake.accrued_reward = stake
					.accrued_reward
					.checked_add(&delegator_reward)
					.ok_or(Error::<T, I>::CalcError1)?;
				let reward_debt_u256 = U256::from(state.reward_weight.saturated_into::<u128>())
					.checked_mul(U256::from(pool.reward_per_token.saturated_into::<u128>()))
					.ok_or(Error::<T, I>::CalcError2)?
					.checked_div_ceil(&U256::from(PER_TOKEN_DECIMALS))
					.ok_or(Error::<T, I>::CalcError3)?;
				state.reward_debt = reward_debt_u256.as_u128().into();
				stake.accrued_slash =
					stake.accrued_slash.checked_add(&slash).ok_or(Error::<T, I>::CalcError4)?;
				let slash_debt_u256 = U256::from(state.slash_weight.saturated_into::<u128>())
					.checked_mul(U256::from(pool.slash_per_token.saturated_into::<u128>()))
					.ok_or(Error::<T, I>::CalcError5)?
					.checked_div(U256::from(PER_TOKEN_DECIMALS))
					.ok_or(Error::<T, I>::CalcError6)?;
				state.slash_debt = slash_debt_u256.as_u128().into();
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

		SelfDelegation::<T, I>::try_mutate(commitment_id, |state_| {
			let state = state_.as_mut().ok_or(Error::<T, I>::CommitmentNotFound)?;
			let reward_u256 = U256::from(state.reward_weight.saturated_into::<u128>())
				.checked_mul(U256::from(pool.reward_per_token.saturated_into::<u128>()))
				.ok_or(Error::<T, I>::CalcError7)?
				.checked_div(U256::from(PER_TOKEN_DECIMALS))
				.ok_or(Error::<T, I>::CalcError8)?;
			let reward_amount: BalanceFor<T, I> = reward_u256.as_u128().into();
			let reward = reward_amount.saturating_sub(state.reward_debt);
			let slash_u256 = U256::from(state.slash_weight.saturated_into::<u128>())
				.checked_mul(U256::from(pool.slash_per_token.saturated_into::<u128>()))
				.ok_or(Error::<T, I>::CalcError9)?
				.checked_div(U256::from(PER_TOKEN_DECIMALS))
				.ok_or(Error::<T, I>::CalcError10)?;
			let slash_amount: BalanceFor<T, I> = slash_u256.as_u128().into();
			let slash =
				slash_amount.checked_sub(&state.slash_debt).ok_or(Error::<T, I>::CalcError11)?;

			Stakes::<T, I>::try_mutate(commitment_id, |staker_| {
				let stake = staker_.as_mut().ok_or(Error::<T, I>::CommitmentNotFound)?;
				stake.accrued_reward =
					stake.accrued_reward.checked_add(&reward).ok_or(Error::<T, I>::CalcError12)?;
				let reward_debt_u256 = U256::from(state.reward_weight.saturated_into::<u128>())
					.checked_mul(U256::from(pool.reward_per_token.saturated_into::<u128>()))
					.ok_or(Error::<T, I>::CalcError13)?
					.checked_div_ceil(&U256::from(PER_TOKEN_DECIMALS))
					.ok_or(Error::<T, I>::CalcError14)?;
				state.reward_debt = reward_debt_u256.as_u128().into();
				stake.accrued_slash =
					stake.accrued_slash.checked_add(&slash).ok_or(Error::<T, I>::CalcError15)?;
				let slash_debt_u256 = U256::from(state.slash_weight.saturated_into::<u128>())
					.checked_mul(U256::from(pool.slash_per_token.saturated_into::<u128>()))
					.ok_or(Error::<T, I>::CalcError16)?
					.checked_div(U256::from(PER_TOKEN_DECIMALS))
					.ok_or(Error::<T, I>::CalcError17)?;
				state.slash_debt = slash_debt_u256.as_u128().into();
				Ok(())
			})
		})
	}

	/// It is guaranteed to withdraw reward/slash only if the result is Ok.
	fn withdraw_delegator_accrued(
		who: &T::AccountId,
		commitment_id: T::CommitmentId,
	) -> Result<BalanceFor<T, I>, Error<T, I>> {
		Self::accrue_delegator(who, commitment_id)?;

		Delegations::<T, I>::try_mutate(who, commitment_id, |state_| {
			let state = state_.as_mut().ok_or(Error::<T, I>::CommitmentNotFound)?;
			let r = state.accrued_reward;
			// let s = PendingReward::new(state.accrued_slash);
			state.accrued_reward = Zero::zero();
			// state.accrued_slash = Zero::zero();
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
		let distribution_account = T::PalletId::get().into_account_truncating();
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
		Self::accrue_committer(commitment_id)?;

		Stakes::<T, I>::try_mutate(commitment_id, |state_| {
			let state = state_.as_mut().ok_or(Error::<T, I>::CommitmentNotFound)?;
			let r = state.accrued_reward;
			// let s = PendingReward::new(state.accrued_slash);
			state.accrued_reward = Zero::zero();
			// state.accrued_slash = Zero::zero();
			Ok(r)
		})
	}

	pub fn cooldown_commitment_for(commitment_id: T::CommitmentId) -> Result<(), Error<T, I>> {
		let current_block = <frame_system::Pallet<T>>::block_number();

		Self::accrue_committer(commitment_id)?;

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
			.ok_or(Error::<T, I>::CalcError18)?;

		let prev_d =
			<SelfDelegation<T, I>>::get(commitment_id).ok_or(Error::<T, I>::InternalError)?;
		let d = Self::update_self_delegation(commitment_id)?;
		let reward_weight_diff = prev_d
			.reward_weight
			.checked_sub(d.reward_weight)
			.ok_or(Error::<T, I>::CalcError19)?;
		DelegationPools::<T, I>::try_mutate(commitment_id, |pool| {
			pool.reward_weight = pool
				.reward_weight
				.checked_sub(reward_weight_diff)
				.ok_or(Error::<T, I>::CalcError20)?;
			Ok(())
		})?;

		Self::update_commitment_stake(
			commitment_id,
			StakeChange::Sub(Zero::zero(), amount_diff),
			&stake,
		)?;

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
		let committer_stake =
			<Stakes<T, I>>::get(commitment_id).ok_or(Error::<T, I>::CommitmentNotFound)?;
		let cooldown_start = if let Some(c) = committer_stake.cooldown_started {
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

		// We error out if the existing delegation was for a previous commitment that got ended and "replaced" by a new commitment by same committer
		// In this case the delegator needs to end (or redelegate) his delegation first.
		ensure!(
			stake.created >= committer_stake.created,
			Error::<T, I>::StaleDelegationMustBeEnded
		);

		// TODO maybe improve this to be stable under multiple reductions of rewardable_amount (currently never happens)
		let amount_diff = stake
			.amount
			.checked_sub(&stake.rewardable_amount)
			.ok_or(Error::<T, I>::CalcError21)?;

		Self::update_commitment_stake(
			commitment_id,
			StakeChange::Sub(Zero::zero(), amount_diff),
			&stake,
		)?;

		let reward_weight_diff = DelegationPoolMembers::<T, I>::try_mutate(
			who,
			commitment_id,
			|d| -> Result<U256, Error<T, I>> {
				let m = d.as_mut().ok_or(Error::<T, I>::NotDelegating)?;
				let prev_reward_weight = m.reward_weight;
				let reward_weight = U256::from(stake.rewardable_amount.saturated_into::<u128>())
					.checked_mul(U256::from(stake.cooldown_period.saturated_into::<u128>()))
					.ok_or(Error::<T, I>::CalcError22)?
					.checked_div(U256::from(T::MaxCooldownPeriod::get().saturated_into::<u128>()))
					.ok_or(Error::<T, I>::CalcError23)?;
				m.reward_weight = reward_weight;

				let reward_debt = reward_weight
					.checked_mul(DelegationPools::<T, I>::get(commitment_id).reward_per_token)
					.ok_or(Error::<T, I>::CalcError24)?
					.checked_div_ceil(&U256::from(PER_TOKEN_DECIMALS))
					.ok_or(Error::<T, I>::CalcError25)?;
				m.reward_debt = reward_debt.as_u128().into();
				prev_reward_weight
					.checked_sub(m.reward_weight)
					.ok_or(Error::<T, I>::CalcError26)
			},
		)?;

		// UPDATE per pool weights (not yet global totals, only when ending delegation)
		DelegationPools::<T, I>::try_mutate(commitment_id, |pool| {
			pool.reward_weight = pool
				.reward_weight
				.checked_sub(reward_weight_diff)
				.ok_or(Error::<T, I>::CalcError27)?;
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
		let old_delegation =
			<Delegations<T, I>>::get(who, old_commitment_id).ok_or(Error::<T, I>::NotDelegating)?;

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
	) -> Result<(), Error<T, I>> {
		let current_block = <frame_system::Pallet<T>>::block_number();

		Self::withdraw_delegation_for(who, commitment_id)?;

		let stake = <Delegations<T, I>>::try_mutate(
			who,
			commitment_id,
			|s_| -> Result<StakeFor<T, I>, Error<T, I>> {
				let s = s_.as_mut().ok_or(Error::<T, I>::NotDelegating)?;
				if check_cooldown {
					if let Some(committer_stake) = Stakes::<T, I>::get(commitment_id) {
						// skip all cooldown checks if the existing delegation was for a previous commitment that got ended and "replaced" by a new commitment by same committer
						if s.created >= committer_stake.created {
							match (committer_stake.cooldown_started, s.cooldown_started) {
								(
									Some(committer_cooldown_start),
									Some(delegator_cooldown_start),
								) => {
									let first =
										if committer_cooldown_start < delegator_cooldown_start {
											committer_cooldown_start
										} else {
											delegator_cooldown_start
										};
									ensure!(
										first.saturating_add(s.cooldown_period) <= current_block,
										Error::<T, I>::CooldownNotEnded
									);
								},
								(Some(committer_cooldown_start), None) => {
									// inherit the committer's cooldown start
									ensure!(
										committer_cooldown_start.saturating_add(s.cooldown_period)
											<= current_block,
										Error::<T, I>::CooldownNotEnded
									);
								},
								(None, Some(delegator_cooldown_start)) => {
									ensure!(
										delegator_cooldown_start.saturating_add(s.cooldown_period)
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

		let udpated_commitment_stake = Self::update_commitment_stake(
			commitment_id,
			StakeChange::Sub(stake.amount, stake.rewardable_amount),
			&stake,
		)?;
		Self::update_total_stake(StakeChange::Sub(stake.amount, stake.rewardable_amount))?;

		// keep ComputeCommitments around for delegations that have not ended
		if udpated_commitment_stake.is_zero()
			&& Stakes::<T, I>::get(commitment_id)
				.map(|committer_stake| stake.created >= committer_stake.created)
				.unwrap_or(true)
		{
			let _ = <ComputeCommitments<T, I>>::clear_prefix(commitment_id, u32::MAX, None);
		}

		// UPDATE per pool and global TOTALS
		DelegationPools::<T, I>::try_mutate(commitment_id, |pool| {
			pool.reward_weight = pool
				.reward_weight
				.checked_sub(state.reward_weight)
				.ok_or(Error::<T, I>::CalcError28)?;
			pool.slash_weight = pool
				.slash_weight
				.checked_sub(state.slash_weight)
				.ok_or(Error::<T, I>::CalcError29)?;
			Ok(())
		})?;
		// delegator_total -= amount
		<DelegatorTotal<T, I>>::try_mutate(who, |s| -> Result<(), Error<T, I>> {
			*s = s.checked_sub(&stake.amount).ok_or(Error::<T, I>::CalcError30)?;
			Ok(())
		})?;
		<TotalDelegated<T, I>>::try_mutate(|s| -> Result<(), Error<T, I>> {
			*s = s.checked_sub(&stake.amount).ok_or(Error::<T, I>::CalcError31)?;
			Ok(())
		})?;

		Self::unlock_and_slash(who, &stake)?;

		Ok(())
	}

	pub fn end_commitment_for(
		who: &T::AccountId,
		commitment_id: T::CommitmentId,
	) -> Result<BalanceFor<T, I>, Error<T, I>> {
		let current_block = <frame_system::Pallet<T>>::block_number();

		let reward = Self::withdraw_committer_accrued(commitment_id)?;

		let stake = <Stakes<T, I>>::try_mutate(
			commitment_id,
			|s_| -> Result<StakeFor<T, I>, Error<T, I>> {
				let s = s_.as_mut().ok_or(Error::<T, I>::CommitmentNotFound)?;
				let cooldown_start = s.cooldown_started.ok_or(Error::<T, I>::CooldownNotStarted)?;
				ensure!(
					cooldown_start.saturating_add(s.cooldown_period) <= current_block,
					Error::<T, I>::CooldownNotEnded
				);

				Ok(s_.take().unwrap())
			},
		)?;

		<SelfDelegation<T, I>>::remove(commitment_id);

		let udpated_commitment_stake = Self::update_commitment_stake(
			commitment_id,
			StakeChange::Sub(stake.amount, stake.rewardable_amount),
			&stake,
		)?;
		Self::update_total_stake(StakeChange::Sub(stake.amount, stake.rewardable_amount))?;

		// keep ComputeCommitments around for delegations that have not ended
		if udpated_commitment_stake.is_zero() {
			let _ = <ComputeCommitments<T, I>>::clear_prefix(commitment_id, u32::MAX, None);
		}

		Self::unlock_and_slash(who, &stake)?;

		// Eventhough all delegator's cooldown has force-ended before this unstake is successful,
		// we cannot clear all delegators here because it would make this call non-constant in number of delegators.
		// We let them remain in the delegation pool and make sure ending delegation is tolerating the commitment already gone.

		Ok(reward)
	}

	fn unlock_and_slash(who: &T::AccountId, stake: &StakeFor<T, I>) -> Result<(), Error<T, I>> {
		Self::unlock_funds(who, stake.amount);
		// Transfer the already unlocked stake with accrued slash, be sure to fail hard on errors!a
		if !stake.accrued_slash.is_zero() {
			let distribution_account = T::PalletId::get().into_account_truncating();
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
					*s = s.saturating_sub(stake.amount);
				});

				// Remove delegation pool member
				<DelegationPoolMembers<T, I>>::remove(&delegator, commitment_id);

				// Unlock funds for delegator
				Self::unlock_funds(&delegator, stake.amount);

				// Add to total delegation amount
				total_delegation_amount = total_delegation_amount.saturating_add(stake.amount);

				// Remove this specific delegation
				<Delegations<T, I>>::remove(&delegator, commitment_id);
			}
		}

		// Update total delegated
		<TotalDelegated<T, I>>::mutate(|s| {
			*s = s.saturating_sub(total_delegation_amount);
		});

		// Remove commitment's own stake
		if let Some(stake) = <Stakes<T, I>>::take(commitment_id) {
			total_stake_amount = stake.amount;
		}

		// Calculate total amount to remove from global stake
		let total_amount_to_remove = total_delegation_amount.saturating_add(total_stake_amount);

		// Update global totals
		if !total_amount_to_remove.is_zero() {
			<TotalStake<T, I>>::mutate(|s| {
				*s = s.saturating_sub(total_amount_to_remove);
			});
		}
		let _ = Self::update_total_stake(StakeChange::Sub(
			total_amount_to_remove,
			total_amount_to_remove,
		));

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
	}

	fn delegation_ratio(commitment_id: T::CommitmentId) -> Perquintill {
		let (_, rewardable_amount) = <CommitmentStake<T, I>>::get(commitment_id);
		let denominator: u128 = rewardable_amount.saturated_into();
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
