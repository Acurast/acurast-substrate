use frame_support::pallet_prelude::*;
use frame_support::traits::Currency;

use crate::{BalanceOf, Config, Stake, StakeOf};

/// Allows to hook custom logic for staking related state transitions.
pub trait StakingHooks<T: Config> {
    /// Returns the stakable balance. The stakable balance is a part of the total account balance that might be required
    /// to fullfill additional preconditions such that this amount was vested.
    ///
    /// Note that the returned balance includes both free amounts and amounts locked for staking or vesting. To calculate
    /// the **remaining stakable balance** you have to consider the current bond.
    fn get_collator_stakable_balance(acc: &T::AccountId) -> Result<BalanceOf<T>, DispatchError>;

    /// Defines the behavior to payout the collator's reward.
    fn payout_collator_reward(
        round_index: crate::RoundIndex,
        collator: &T::AccountId,
        reward: BalanceOf<T>,
    ) -> Weight;

    fn power(acc: &T::AccountId, amount: BalanceOf<T>) -> Result<StakeOf<T>, DispatchError>;

    fn compound(target: &T::AccountId, more: BalanceOf<T>) -> Result<(), DispatchError>;
}

impl<T: Config> StakingHooks<T> for () {
    fn get_collator_stakable_balance(acc: &T::AccountId) -> Result<BalanceOf<T>, DispatchError> {
        Ok(T::Currency::free_balance(acc))
    }

    /// The default behavior for paying out the collator's reward. The amount is directly
    /// deposited into the collator's account.
    fn payout_collator_reward(
        for_round: crate::RoundIndex,
        collator: &T::AccountId,
        reward: crate::BalanceOf<T>,
    ) -> Weight {
        crate::Pallet::<T>::mint_collator_reward(for_round, collator, reward)
    }

    fn power(_acc: &T::AccountId, amount: BalanceOf<T>) -> Result<StakeOf<T>, DispatchError> {
        // default to 1:1 for amount:power
        Ok(Stake::new(amount, amount))
    }

    fn compound(_target: &T::AccountId, _more: BalanceOf<T>) -> Result<(), DispatchError> {
        Ok(())
    }
}
