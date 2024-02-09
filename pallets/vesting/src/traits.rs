use frame_support::{sp_runtime::DispatchError, weights::Weight};
use sp_arithmetic::Perbill;

/// Trait used to manage vesting stakes and accrued rewards.
pub trait VestingBalance<AccountId, Balance> {
    /// Tracks a stake being locked on an account.
    ///
    /// Can be implemented by setting a lock/freeze on `pallet_balances`.
    fn lock_stake(target: &AccountId, stake: Balance) -> Result<(), DispatchError>;
    /// Adjusts the stake lock to the new total.
    fn adjust_lock(acc: &AccountId, stake: Balance);
    /// Pays out the accrued amount to an individual account.
    /// It depends on the implementation if `accrued` was already minted or still has to be.
    ///
    /// Can be implemented by transfering the amount from a pallet account to `target` on `pallet_balances`.
    fn pay_accrued(target: &AccountId, accrued: Balance) -> Result<(), DispatchError>;
    /// Pays out the accrued amount to an individual account of the kicker.
    /// It depends on the implementation if `accrued` was already minted or still has to be.
    ///
    /// Can be implemented by transfering the amount from a pallet account to `target` on `pallet_balances`.
    fn pay_kicker(target: &AccountId, accrued: Balance) -> Result<(), DispatchError>;
    /// Pays out the staked amount to an individual account.
    /// It depends on the implementation if `stake` was burned when staked and has to be minted again.
    ///
    /// Can be implemented by transfering the amount from a pallet account to `target` on `pallet_balances`.
    fn unlock_stake(target: &AccountId, stake: Balance) -> Result<(), DispatchError>;
    fn power_decreased(target: &AccountId, perbill: Perbill) -> Result<(), DispatchError>;
    fn power_increased(
        target: &AccountId,
        reciprocal_perbill: Perbill,
    ) -> Result<(), DispatchError>;
}

pub trait WeightInfo {
    fn vest() -> Weight;
    fn revest() -> Weight;
    fn divest() -> Weight;
    fn cooldown() -> Weight;
    fn kick_out() -> Weight;
    fn distribute_reward() -> Weight;
}

impl WeightInfo for () {
    fn vest() -> Weight {
        Weight::from_parts(10_000, 0)
    }

    fn revest() -> Weight {
        Weight::from_parts(10_000, 0)
    }

    fn divest() -> Weight {
        Weight::from_parts(10_000, 0)
    }

    fn cooldown() -> Weight {
        Weight::from_parts(10_000, 0)
    }

    fn kick_out() -> Weight {
        Weight::from_parts(10_000, 0)
    }

    fn distribute_reward() -> Weight {
        Weight::from_parts(10_000, 0)
    }
}
