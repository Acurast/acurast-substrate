use acurast_common::{CommitterIdProvider, PoolId};
use frame_support::{
	pallet_prelude::*,
    sp_runtime::SaturatedConversion,
	traits::{Currency, Get, InspectLockableCurrency, LockableCurrency, WithdrawReasons},
};
use sp_runtime::{traits::{CheckedAdd, CheckedSub, Saturating, Zero}, Perquintill};

use crate::*;

#[derive(Clone, PartialEq, Eq)]
enum StakeChange<Balance> {
	Add(Balance),
	Sub(Balance),
}

impl<T: Config<I>, I: 'static> Pallet<T, I> {
    pub fn distribute(epoch: EpochOf<T, I>, amount: T::Balance) -> Result<(), Error<T, I>> {
        for (pool_id, pool) in MetricPools::<T, I>::iter() {
            let a: u128 = amount.into();
            let reward: u64 = pool.reward.get(epoch).mul_floor(a as u64);
            Self::distribute_top(pool_id, (reward as u128).into())?;
        }

        Ok(())
    }

    fn distribute_top(pool_id: PoolId, amount: T::Balance) -> Result<(), Error<T, I>> {
        StakingPools::<T, I>::try_mutate(pool_id, |pool| {
            if !pool.weight.is_zero() {
                pool.reward_per_token = pool.reward_per_token.checked_add(&amount.checked_mul(&T::Decimals::get()).ok_or(Error::<T, I>::CalculationOverflow)?.checked_div(&pool.weight).ok_or(Error::<T, I>::CalculationOverflow)?).ok_or(Error::<T, I>::CalculationOverflow)?;
            }

            Ok(())
        })?;

        Ok(())
    }

    fn distribute_down(committer_id: T::CommitterId) -> Result<(), Error<T, I>> {
        for (pool_id, _metric) in ComputeCommitments::<T, I>::iter_prefix(committer_id) {
            // we have to accrue and distribute everything from "parent pools" == (the metric pools the "delegation target" == staker is in)
            let (r, s) = Self::withdraw_staker_accrued(pool_id, committer_id)?;
            if r > Zero::zero() {
                Self::reward_delegation_pool(committer_id, r)?;
            }
            if s > Zero::zero() {
                Self::slash_delegation_pool(committer_id, s)?;
            }
        }

        Ok(())
    }

    fn reward_delegation_pool(committer_id: T::CommitterId, amount: T::Balance) -> Result<(), Error<T, I>> {
        DelegationPools::<T, I>::try_mutate(committer_id, |pool|-> Result<(), Error<T, I>> {
            if !pool.weight.is_zero() {
                pool.reward_per_token = pool.reward_per_token.checked_add(&amount.checked_mul(&T::Decimals::get()).ok_or(Error::<T, I>::CalculationOverflow)?.checked_div(&pool.weight).ok_or(Error::<T, I>::CalculationOverflow)?).ok_or(Error::<T, I>::CalculationOverflow)?;
            }
            Ok(())
        })
    }

    fn slash_delegation_pool(committer_id: T::CommitterId, amount: T::Balance) -> Result<(), Error<T, I>> {
        DelegationPools::<T, I>::try_mutate(committer_id, |pool| -> Result<(), Error<T, I>> {
            if !pool.weight.is_zero() {
                pool.slash_per_token = pool.slash_per_token.checked_add(&amount.checked_mul(&T::Decimals::get()).ok_or(Error::<T, I>::CalculationOverflow)?.checked_div(&pool.weight).ok_or(Error::<T, I>::CalculationOverflow)?).ok_or(Error::<T, I>::CalculationOverflow)?;
            }
            Ok(())
        })
    }

	/// Stakes for the first time.
	pub fn stake_for(who: &T::AccountId, amount: T::Balance, cooldown_period: T::BlockNumber) -> Result<(), Error<T, I>> {
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

        // locking is on accounts (while all other storage points are relative to `committer_id`)
		Self::lock_funds(&who, amount, LockReason::Staking)?;

        let committer_id = T::CommitterIdProvider::committer_id_for(&who).map_err(|_| Error::<T, I>::NoOwnerOfCommitterId)?;
        let stake = <Stakes<T, I>>::try_mutate(committer_id, |state| -> Result<StakeFor<T, I>, Error<T, I>> {
            ensure!(state.is_none(), Error::<T, I>::AlreadyCommitted);
            let s = Stake::new(amount, cooldown_period);
            *state = Some(s.clone());
            Ok(s)
        })?;
        
        // update global total
        <TotalStake<T, I>>::try_mutate(|s| -> Result<(), Error<T, I>> {
            *s = s.checked_add(&amount).ok_or(Error::<T, I>::CalculationOverflow)?;
            Ok(())
        })?;

        Self::committer_change_stake(committer_id, StakeChange::Add(stake.amount))?;

        Ok(())
	}

    /// Stakes an extra (additional) amount towards an existing commitment.
	pub fn stake_more_for(who: &T::AccountId, extra_amount: T::Balance) -> Result<(), Error<T, I>> {
		ensure!(
			!extra_amount.is_zero(),
			Error::<T, I>::MinStakeSubceeded
		);

        let committer_id = T::CommitterIdProvider::committer_id_for(&who).map_err(|_| Error::<T, I>::NoOwnerOfCommitterId)?;
        <Stakes<T, I>>::try_mutate(committer_id, |stake_| -> Result<(), Error<T, I>> {
            let stake = stake_.as_mut().ok_or(Error::<T, I>::NotStaking)?;
            let prev_amount = stake.amount;
            let amount = prev_amount.checked_add(&extra_amount).ok_or(Error::<T, I>::CalculationOverflow)?;
            
            // locking is on accounts (while all other storage points are relative to `committer_id`)
            Self::lock_funds(&who, amount, LockReason::Staking)?;

            stake.amount = amount;

            // update global total
            let amount_diff =
                &amount.checked_sub(&prev_amount).ok_or(Error::<T, I>::CalculationOverflow)?;
            <TotalStake<T, I>>::try_mutate(|s| -> Result<(), Error<T, I>> {
                *s = s.checked_add(amount_diff).ok_or(Error::<T, I>::CalculationOverflow)?;
                Ok(())
            })
        })?;

        Self::committer_change_stake(committer_id, StakeChange::Add(extra_amount))?;

        Ok(())
	}

    pub fn delegate_for(who: &T::AccountId, committer_id: T::CommitterId, amount: T::Balance, cooldown_period: T::BlockNumber) -> Result<(), Error<T, I>> {
        ensure!(!amount.is_zero() && amount >= T::MinDelegation::get(), Error::<T, I>::BelowMinDelegation);
        ensure!(
            cooldown_period >= T::MinCooldownPeriod::get(),
            Error::<T, I>::BelowMinCooldownPeriod
        );
        ensure!(
            cooldown_period <= T::MaxCooldownPeriod::get(),
            Error::<T, I>::AboveMaxCooldownPeriod
        );
        ensure!(
            Self::delegation_ratio(committer_id) <= T::MaxDelegationRatio::get(),
            Error::<T, I>::MaxDelegationRatioExceeded
        );
        let committer_stake = Stakes::<T, I>::get(&committer_id).ok_or(Error::<T, I>::NotStaking)?;
        ensure!(
            cooldown_period <= committer_stake.cooldown_period,
            Error::<T, I>::DelegationCooldownMustBeShorterThanCommitter
        );

        Self::lock_funds(&who, amount, LockReason::Delegation(committer_id))?;

        Self::distribute_down(committer_id)?;

        Self::committer_change_stake(committer_id, StakeChange::Add(amount))?;
        
        let weight = amount.checked_mul(&cooldown_period.saturated_into::<u128>().into()).ok_or(Error::<T, I>::CalculationOverflow)?
                / T::MaxCooldownPeriod::get().saturated_into::<u128>().into();
        let reward_debt = weight.checked_mul(&DelegationPools::<T, I>::get(committer_id).reward_per_token).ok_or(Error::<T, I>::CalculationOverflow)? / T::Decimals::get();
        let slash_debt = weight.checked_mul(&DelegationPools::<T, I>::get(committer_id).slash_per_token).ok_or(Error::<T, I>::CalculationOverflow)? / T::Decimals::get();

        ensure!(Delegations::<T, I>::get(&who, committer_id).is_none(), Error::<T, I>::AlreadyDelegating);

        Delegations::<T, I>::insert(
            &who,
            committer_id,
            Stake::new(amount, cooldown_period),
        );
        DelegationPoolMembers::<T, I>::insert(
            &who,
            committer_id,
            PoolMember::new(weight, reward_debt, slash_debt),
        );

        // UPDATE per pool and global TOTALS
        DelegationPools::<T, I>::try_mutate(committer_id, |pool| {pool.weight = pool.weight.checked_add(&weight).ok_or(Error::<T, I>::CalculationOverflow)?; Ok(())})?;
        // delegator_total += amount
        <TotalDelegated<T, I>>::try_mutate(who, |s| -> Result<(), Error<T, I>> {
            *s = s.checked_add(&amount).ok_or(Error::<T, I>::CalculationOverflow)?;
            Ok(())
        })?;

        Ok(())
    }

    /// Helper function that allows to both increase and decrease amount in a pool. It accrues outstanding rewards/slashes before re-snapshotting the debts and adapting weights.
    /// 
    /// - Increase can happen on new delegator joining or staking more.
    /// - Decrease can happen on delegator leaving.
    fn committer_change_stake(committer_id: T::CommitterId, change: StakeChange<T::Balance>) -> Result<(), Error<T, I>> {
        let stake = Stakes::<T, I>::get(&committer_id).ok_or(Error::<T, I>::NotStaking)?;

        let amount = match change {
            StakeChange::Add(amount) => {
                <TotalCommittedStake<T, I>>::try_mutate(committer_id, |s| -> Result<T::Balance, Error<T, I>> {
                    *s = s.checked_add(&amount).ok_or(Error::<T, I>::CalculationOverflow)?;
                    Ok(*s)
                })
            },
            StakeChange::Sub(amount) => {
                <TotalCommittedStake<T, I>>::try_mutate(committer_id, |s| -> Result<T::Balance, Error<T, I>> {
                    *s = s.checked_sub(&amount).ok_or(Error::<T, I>::CalculationOverflow)?;
                    Ok(*s)
                })
            },
        }?;
        
        for (pool_id, metric) in ComputeCommitments::<T, I>::iter_prefix(committer_id) {
            Self::staker_accrue(pool_id, committer_id)?;
            let prev_weight = <StakingPoolMembers<T, I>>::get(committer_id, pool_id).ok_or(Error::<T, I>::NotStaking)?.weight;
            
            let weight: T::Balance = (((metric.into_inner() / (1_000_000_000_000_000_000u128 / T::Decimals::get().saturated_into::<u128>())).checked_mul(amount.saturated_into::<u128>()).ok_or(Error::<T, I>::CalculationOverflow)? / T::Decimals::get().saturated_into::<u128>()).checked_mul(stake.cooldown_period.saturated_into::<u128>().into()).ok_or(Error::<T, I>::CalculationOverflow)?
                /T::MaxCooldownPeriod::get().saturated_into::<u128>()).into();
            
            let reward_debt = weight.checked_mul(&StakingPools::<T, I>::get(pool_id).reward_per_token).ok_or(Error::<T, I>::CalculationOverflow)? /T::Decimals::get();
            let slash_debt = weight.checked_mul(&StakingPools::<T, I>::get(pool_id).slash_per_token).ok_or(Error::<T, I>::CalculationOverflow)? /T::Decimals::get();

            match change {
                StakeChange::Add(_) => {
                    let weight_diff=                  &weight.checked_sub(&prev_weight).ok_or(Error::<T, I>::CalculationOverflow)?;
                    StakingPools::<T, I>::try_mutate(pool_id, |pool| {pool.weight = pool.weight.checked_add(weight_diff).ok_or(Error::<T, I>::CalculationOverflow)?; Ok(())})?;
                },
                StakeChange::Sub(_) => {
                    let weight_diff=              &prev_weight.checked_sub(&weight).ok_or(Error::<T, I>::CalculationOverflow)?;
                    StakingPools::<T, I>::try_mutate(pool_id, |pool| {pool.weight = pool.weight.checked_sub(weight_diff).ok_or(Error::<T, I>::CalculationOverflow)?; Ok(())})?;
                },
            }
            
            <StakingPoolMembers<T, I>>::insert(committer_id, pool_id,  PoolMember::new(weight, reward_debt, slash_debt));
        }

        Ok(())
    }
    
    pub fn staker_accrue(pool_id: PoolId, committer_id: T::CommitterId) -> Result<(), Error<T, I>> {
        let pool = StakingPools::<T, I>::get(pool_id);

        StakingPoolMembers::<T, I>::try_mutate(&committer_id, pool_id, |state_| {
            let state = state_.as_mut().ok_or(Error::<T, I>::NotStaking)?;
            let reward = state.weight
                .checked_mul(&pool.reward_per_token).ok_or(Error::<T, I>::CalculationOverflow)?
                / T::Decimals::get()
                .checked_sub(&state.reward_debt).ok_or(Error::<T, I>::CalculationOverflow)?;
            let slash = state.weight
                .checked_mul(&pool.slash_per_token).ok_or(Error::<T, I>::CalculationOverflow)?
                / T::Decimals::get()
                .checked_sub(&state.slash_debt).ok_or(Error::<T, I>::CalculationOverflow)?;
    
            Stakes::<T, I>::try_mutate(&committer_id, |staker_| {
                let stake = staker_.as_mut().ok_or(Error::<T, I>::NotStaking)?;
                stake.accrued_reward = stake.accrued_reward.checked_add(&reward).ok_or(Error::<T, I>::CalculationOverflow)?;
                stake.accrued_slash = stake.accrued_slash.checked_add(&slash).ok_or(Error::<T, I>::CalculationOverflow)?;
                state.reward_debt = state.weight
                    .checked_mul(&pool.reward_per_token).ok_or(Error::<T, I>::CalculationOverflow)?
                    / T::Decimals::get();
                state.slash_debt = state.weight
                    .checked_mul(&pool.slash_per_token).ok_or(Error::<T, I>::CalculationOverflow)?
                    /T::Decimals::get();
                Ok(())
            })
        })
    }

    /// It is guaranteed to withdraw reward/slash only if the result is Ok. 
    pub fn withdraw_staker_accrued(pool_id: PoolId, committer_id: T::CommitterId) -> Result<(T::Balance, T::Balance), Error<T, I>> {
        Self::staker_accrue(pool_id, committer_id)?;

        Stakes::<T, I>::try_mutate(&committer_id, |staker_| {
            let stake = staker_.as_mut().ok_or(Error::<T, I>::NotStaking)?;
            let r = stake.accrued_reward;
            let s = stake.accrued_slash;
            stake.accrued_reward = Zero::zero();
            stake.accrued_slash = Zero::zero();
            Ok((r, s))
        })
    }

    /// It is guaranteed to withdraw reward/slash only if the result is Ok. 
    pub fn withdraw_delegator_accrued(delegator: T::AccountId, committer_id: T::CommitterId) -> Result<(T::Balance, T::Balance), Error<T, I>> {
        Self::distribute_down(committer_id)?;

        Delegations::<T, I>::try_mutate(delegator, &committer_id, |state_| {
            let state = state_.as_mut().ok_or(Error::<T, I>::NotStaking)?;
            let r = state.accrued_reward;
            let s = state.accrued_slash;
            state.accrued_reward = Zero::zero();
            state.accrued_slash = Zero::zero();
            Ok((r, s))
        })
    }

	pub fn cooldown_stake_for(committer_id: T::CommitterId) -> Result<(), Error<T, I>> {
		let current_block = T::BlockNumber::from(<frame_system::Pallet<T>>::block_number());

		<Stakes<T, I>>::try_mutate(committer_id, |s| -> Result<(), Error<T, I>> {
			let s = s.as_mut().ok_or(Error::<T, I>::NotStaking)?;
			ensure!(s.cooldown_started.is_none(), Error::<T, I>::CooldownAlreadyStarted);
			s.cooldown_started = Some(current_block);
			Ok(())
		})?;

		Ok(())
	}

	// TODO move to it's own module for delegation
	pub fn cooldown_delegation_for(
		who: &T::AccountId,
		committer_id: T::CommitterId,
	) -> Result<T::BlockNumber, Error<T, I>> {
		// Special case: the staker delegated to has started cooldown himself. In this case the start of the delegator's cooldown is pretended to be the staker's start of cooldown.
		let stake = <Stakes<T, I>>::get(committer_id).ok_or(Error::<T, I>::InternalErrorNotStaking)?;
		let cooldown_start = if let Some(c) = stake.cooldown_started {
			c
		} else {
			T::BlockNumber::from(<frame_system::Pallet<T>>::block_number())
		};

		<Delegations<T, I>>::try_mutate(who, committer_id, |d| -> Result<(), Error<T, I>> {
			let s = d.as_mut().ok_or(Error::<T, I>::CalculationOverflow)?;
			ensure!(s.cooldown_started.is_none(), Error::<T, I>::CooldownAlreadyStarted);
			s.cooldown_started = Some(cooldown_start);
			Ok(())
		})?;

		Ok(cooldown_start)
	}

	pub fn end_delegation_for(
		who: &T::AccountId,
		committer_id: T::CommitterId,
	) -> Result<T::Balance, Error<T, I>> {
		let current_block = T::BlockNumber::from(<frame_system::Pallet<T>>::block_number());

        Self::distribute_down(committer_id)?;
        
		let stake = <Delegations<T, I>>::try_mutate(
			who,
			committer_id,
			|s_| -> Result<StakeFor<T, I>, Error<T, I>> {
				let s = s_.as_mut().ok_or(Error::<T, I>::NotDelegating)?;
				let cooldown_start = s.cooldown_started.ok_or(Error::<T, I>::CooldownNotStarted)?;
				ensure!(
					cooldown_start.saturating_add(s.cooldown_period) >= current_block,
					Error::<T, I>::CooldownNotEnded
				);

				Ok(s_.take().unwrap())
			},
		)?;

        let state = <DelegationPoolMembers<T, I>>::try_mutate(
			who,
			committer_id,
			|s_| -> Result<PoolMemberFor<T, I>, Error<T, I>> {
				s_.as_mut().ok_or(Error::<T, I>::NotDelegating)?;
				Ok(s_.take().unwrap())
			},
		)?;

        Self::committer_change_stake(committer_id, StakeChange::Sub(stake.amount))?;

        // UPDATE per pool and global TOTALS
        DelegationPools::<T, I>::try_mutate(committer_id, |pool| {pool.weight = pool.weight.checked_sub(&state.weight).ok_or(Error::<T, I>::CalculationOverflow)?; Ok(())})?;
        // delegator_total -= amount
		<TotalDelegated<T, I>>::try_mutate(who, |s| -> Result<(), Error<T, I>> {
			*s = s.checked_sub(&stake.amount).ok_or(Error::<T, I>::CalculationOverflow)?;
			Ok(())
		})?;

		Self::unlock_funds(&who, stake.amount);

		Ok(stake.amount)
	}

	pub fn unstake_for(who: T::AccountId, committer_id: T::CommitterId) -> Result<T::Balance, Error<T, I>> {
		let current_block = T::BlockNumber::from(<frame_system::Pallet<T>>::block_number());

		let stake = <Stakes<T, I>>::try_mutate(committer_id, |s_| -> Result<StakeFor<T, I>, Error<T, I>> {
			let s = s_.as_mut().ok_or(Error::<T, I>::NotStaking)?;
			let cooldown_start = s.cooldown_started.ok_or(Error::<T, I>::CooldownNotStarted)?;
			ensure!(
				cooldown_start.saturating_add(s.cooldown_period) >= current_block,
				Error::<T, I>::CooldownNotEnded
			);

			Ok(s_.take().unwrap())
		})?;

		<TotalStake<T, I>>::try_mutate(|s| -> Result<(), Error<T, I>> {
			*s = s.checked_sub(&stake.amount).ok_or(Error::<T, I>::CalculationOverflow)?;
			Ok(())
		})?;

		Self::unlock_funds(&who, stake.amount);

		Ok(stake.amount)
	}

    fn delegation_ratio(
        committer_id: T::CommitterId,
    ) -> Perquintill {
        let denominator: u128 = <TotalCommittedStake<T, I>>::get(committer_id).into();
        let nominator: u128 = <Stakes<T, I>>::get(committer_id)
            .map(|s| s.amount)
            .unwrap_or(Zero::zero())
            .into();
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
	/// This method ensures the new total is locked, respecting potential previous delegation locks for same committer.
	pub fn lock_funds(
		who: &T::AccountId,
		amount: T::Balance,
		reason: LockReason<T::CommitterId>,
	) -> Result<(), Error<T, I>> {
		let new_lock_total = match reason {
			LockReason::Staking => {
				let delegator_total = <TotalDelegated<T, I>>::get(who);
				delegator_total.saturating_add(amount)
			},
			LockReason::Delegation(committer_id) => {
                let staked = if let Ok(delegator_committer_id) = T::CommitterIdProvider::committer_id_for(&who) {
                    <Stakes<T, I>>::get(delegator_committer_id).map(|s| s.amount)
                } else {
                    None
                }.unwrap_or(Zero::zero());
				let delegated = <Delegations<T, I>>::get(who, committer_id)
					.map(|d| d.amount)
					.unwrap_or(Zero::zero());
				let delegator_total = <TotalDelegated<T, I>>::get(who);
				delegator_total
					.saturating_sub(delegated)
					.saturating_add(staked)
					.saturating_add(amount)
			},
		};

		// also reserved balance can be locked, therefore compare to total_balance
		if <T::Currency as Currency<T::AccountId>>::total_balance(who) < new_lock_total.into() {
			Err(Error::<T, I>::InsufficientBalance)?;
		}
		<T::Currency as LockableCurrency<T::AccountId>>::set_lock(
			T::LockIdentifier::get(),
			&who,
			new_lock_total.into(),
			WithdrawReasons::all(),
		);
		Ok(())
	}

	/// Returns the staked amount to an account's usable balance (for the part that is not also
	/// reserved) by unlocking the amount.
	///
	/// Note that the free balance does not change.
	pub fn unlock_funds(who: &T::AccountId, amount: T::Balance) {
		let new_total_stake =
			<T::Currency as InspectLockableCurrency<T::AccountId>>::balance_locked(
				T::LockIdentifier::get(),
				who,
			)
			.saturating_sub(amount.into());

		if new_total_stake.is_zero() {
			<T::Currency as LockableCurrency<T::AccountId>>::remove_lock(
				T::LockIdentifier::get(),
				who,
			);
		} else {
			<T::Currency as LockableCurrency<T::AccountId>>::set_lock(
				T::LockIdentifier::get(),
				&who,
				new_total_stake.into(),
				WithdrawReasons::all(),
			);
		}
	}
}
