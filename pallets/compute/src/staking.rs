use acurast_common::ManagerIdProvider;
use frame_support::{
	pallet_prelude::*,
	traits::{Currency, Get, InspectLockableCurrency, LockableCurrency, WithdrawReasons},
};
use sp_runtime::traits::{CheckedAdd, CheckedSub, Saturating, Zero};

use crate::*;

impl<T: Config<I>, I: 'static> Pallet<T, I> {
	pub fn stake_for(who: &T::AccountId, amount: T::Balance) -> Result<(), DispatchError> {
		ensure!(
			!amount.is_zero() && amount >= <T as Config<I>>::MinStake::get(),
			Error::<T, I>::MinStakeSubceeded
		);

		Self::lock_funds(&who, amount, LockReason::Staking)?;

		<Stakes<T, I>>::try_mutate(who, |state| -> Result<(), Error<T, I>> {
			let prev_amount = if let Some(current_state) = state {
				// only increasing stake is allowed
				if amount <= current_state.amount {
					Err(Error::<T, I>::CannotStakeLessOrEqual)?;
				}

				let prev_amount = current_state.amount;

				current_state.amount = amount;

				prev_amount
			} else {
				*state = Some(Stake {
					amount,
					accrued: T::Balance::zero(),
					cooldown_period: T::BlockNumber::zero(),
					cooldown_started: None,
				});

				T::Balance::zero()
			};

			// total_stake += stake [- prev_stake]
			let diff =
				&amount.checked_sub(&prev_amount).ok_or(Error::<T, I>::CalculationOverflow)?;
			<TotalStake<T, I>>::try_mutate(|s| -> Result<(), Error<T, I>> {
				*s = s.checked_add(diff).ok_or(Error::<T, I>::CalculationOverflow)?;
				Ok(())
			})?;

			Ok(())
		})?;

		Ok(())
	}

	pub fn cooldown_stake_for(who: &T::AccountId) -> Result<(), DispatchError> {
		let current_block = T::BlockNumber::from(<frame_system::Pallet<T>>::block_number());

		<Stakes<T, I>>::try_mutate(who, |s| -> Result<(), Error<T, I>> {
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
		manager_id: T::ManagerId,
	) -> Result<T::BlockNumber, DispatchError> {
		let manager = T::ManagerIdProvider::owner_for(manager_id)?;

		// Special case: the staker delegated to has started cooldown himself. In this case the start of the delegator's cooldown is pretended to be the staker's start of cooldown.
		let stake = <Stakes<T, I>>::get(manager).ok_or(Error::<T, I>::InternalErrorNotStaking)?;
		let cooldown_start = if let Some(c) = stake.cooldown_started {
			c
		} else {
			T::BlockNumber::from(<frame_system::Pallet<T>>::block_number())
		};

		<Delegations<T, I>>::try_mutate(who, manager_id, |d| -> Result<(), Error<T, I>> {
			let s = d.as_mut().ok_or(Error::<T, I>::CalculationOverflow)?;
			ensure!(s.cooldown_started.is_none(), Error::<T, I>::CooldownAlreadyStarted);
			s.cooldown_started = Some(cooldown_start);
			Ok(())
		})?;

		Ok(cooldown_start)
	}

	pub fn end_delegation_for(
		who: &T::AccountId,
		manager_id: T::ManagerId,
	) -> Result<T::Balance, DispatchError> {
		let current_block = T::BlockNumber::from(<frame_system::Pallet<T>>::block_number());

		let state = <Delegations<T, I>>::try_mutate(
			who,
			manager_id,
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

		<DelegatorTotals<T, I>>::try_mutate(who, |s| -> Result<(), Error<T, I>> {
			*s = s.checked_sub(&state.amount).ok_or(Error::<T, I>::CalculationOverflow)?;
			Ok(())
		})?;

		let w = Self::delegator_weight(&state);
		<DelegateeTotals<T, I>>::try_mutate(manager_id, |t| -> Result<(), Error<T, I>> {
			*t = DelegateeTotal {
				amount: t.amount.saturating_sub(state.amount),
				weight: t.weight.saturating_sub(w),
				count: t.count.saturating_sub(1u8),
			};
			Ok(())
		})?;

		Self::unlock_funds(&who, state.amount);

		Ok(state.amount)
	}

	pub fn unstake_for(who: &T::AccountId) -> Result<T::Balance, DispatchError> {
		let current_block = T::BlockNumber::from(<frame_system::Pallet<T>>::block_number());

		let amount = <Stakes<T, I>>::try_mutate(who, |s_| -> Result<T::Balance, Error<T, I>> {
			let s = s_.as_mut().ok_or(Error::<T, I>::NotStaking)?;
			let cooldown_start = s.cooldown_started.ok_or(Error::<T, I>::CooldownNotStarted)?;
			ensure!(
				cooldown_start.saturating_add(s.cooldown_period) >= current_block,
				Error::<T, I>::CooldownNotEnded
			);

			Ok(s_.take().unwrap().amount)
		})?;

		<TotalStake<T, I>>::try_mutate(|s| -> Result<(), Error<T, I>> {
			*s = s.checked_sub(&amount).ok_or(Error::<T, I>::CalculationOverflow)?;
			Ok(())
		})?;

		Self::unlock_funds(&who, amount);

		Ok(amount)
	}

	/// Locks the new stake on the account. The account can have existing stake or delegations locked.
	///
	/// NOTE: we have to lock total stake not difference, so this helper function must be aware of all existing reasons for locking from the compute pallet, under [`T::LockIdentifier`].
	///
	/// This method ensures the new total is locked, respecting potential previous delegation locks for same manager.
	pub fn lock_funds(
		who: &T::AccountId,
		amount: T::Balance,
		reason: LockReason<T::ManagerId>,
	) -> Result<(), DispatchError> {
		let new_lock_total = match reason {
			LockReason::Staking => {
				let delegator_total = <DelegatorTotals<T, I>>::get(who);
				delegator_total.saturating_add(amount)
			},
			LockReason::Delegation(manager_id) => {
				let staked =
					<Stakes<T, I>>::get(who).map(|s| s.amount).unwrap_or(T::Balance::zero());
				let delegated = <Delegations<T, I>>::get(who, manager_id)
					.map(|d| d.amount)
					.unwrap_or(T::Balance::zero());
				let delegator_total = <DelegatorTotals<T, I>>::get(who);
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
