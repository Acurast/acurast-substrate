use core::marker::PhantomData;

use frame_support::sp_runtime::SaturatedConversion;
use frame_support::traits::tokens::Preservation;
use frame_support::{
    pallet_prelude::Member,
    sp_runtime::{
        traits::{AccountIdConversion, Get},
        DispatchError, Percent,
    },
    traits::tokens::fungible,
    PalletId,
};
use sp_std::prelude::*;
use xcm::prelude::AssetId;

use pallet_acurast::{JobId, MultiOrigin};

use crate::Config;

/// Trait used to manage lock up and payments of rewards.
pub trait RewardManager<T: frame_system::Config + Config> {
    fn lock_reward(
        job_id: &JobId<T::AccountId>,
        reward: <T as Config>::Balance,
    ) -> Result<(), DispatchError>;
    fn pay_reward(
        job_id: &JobId<T::AccountId>,
        reward: <T as Config>::Balance,
        target: &T::AccountId,
    ) -> Result<(), DispatchError>;
    fn pay_matcher_reward(
        remaining_rewards: Vec<(JobId<T::AccountId>, <T as Config>::Balance)>,
        matcher: &T::AccountId,
    ) -> Result<(), DispatchError>;
    fn refund(job_id: &JobId<T::AccountId>) -> Result<T::Balance, DispatchError>;
}

impl<T: frame_system::Config + Config> RewardManager<T> for () {
    fn lock_reward(
        _job_id: &JobId<T::AccountId>,
        _reward: <T as Config>::Balance,
    ) -> Result<(), DispatchError> {
        Ok(())
    }

    fn pay_reward(
        _job_id: &JobId<T::AccountId>,
        _reward: <T as Config>::Balance,
        _target: &T::AccountId,
    ) -> Result<(), DispatchError> {
        Ok(())
    }

    fn pay_matcher_reward(
        _remaining_rewards: Vec<(JobId<T::AccountId>, <T as Config>::Balance)>,
        _matcher: &T::AccountId,
    ) -> Result<(), DispatchError> {
        Ok(())
    }

    fn refund(_job_id: &JobId<T::AccountId>) -> Result<T::Balance, DispatchError> {
        Ok(0u8.into())
    }
}

// This trait provives methods for managing the fees.
pub trait FeeManager {
    fn get_fee_percentage() -> Percent;
    fn get_matcher_percentage() -> Percent;
    fn pallet_id() -> PalletId;
}

trait IsNativeAsset {
    fn is_native_asset(&self) -> bool;
}

impl IsNativeAsset for AssetId {
    fn is_native_asset(&self) -> bool {
        match self {
            AssetId::Concrete(multi_location) => multi_location.is_here(),
            _ => false,
        }
    }
}

pub struct AssetRewardManager<AssetSplit, Currency, JobBudget>(
    PhantomData<(AssetSplit, Currency, JobBudget)>,
);

impl<T, AssetSplit, Currency, Budget> RewardManager<T>
    for AssetRewardManager<AssetSplit, Currency, Budget>
where
    T: Config + frame_system::Config,
    AssetSplit: FeeManager,
    Currency: fungible::Mutate<T::AccountId>,
    <Currency as fungible::Inspect<T::AccountId>>::Balance: Member + From<T::Balance>,
    Budget: JobBudget<T>,
{
    fn lock_reward(job_id: &JobId<T::AccountId>, reward: T::Balance) -> Result<(), DispatchError> {
        let pallet_account: T::AccountId = <T as Config>::PalletId::get().into_account_truncating();
        let hyperdrive_pallet_account: T::AccountId =
            <T as Config>::HyperdrivePalletId::get().into_account_truncating();
        match &job_id.0 {
            MultiOrigin::Acurast(who) => {
                Currency::transfer(
                    who,
                    &pallet_account,
                    reward.saturated_into(),
                    Preservation::Preserve,
                )?;
            }
            MultiOrigin::Tezos(_) | MultiOrigin::Ethereum(_) | MultiOrigin::AlephZero(_) => {
                // The availability of these funds was ensured on the target chain side
                Currency::transfer(
                    &hyperdrive_pallet_account,
                    &pallet_account,
                    reward.saturated_into(),
                    Preservation::Preserve,
                )?;
            }
        };

        Budget::reserve(&job_id, reward)
            .map_err(|_| DispatchError::Other("Severe Error: JobBudget::reserve failed"))?;

        Ok(())
    }

    fn pay_reward(
        job_id: &JobId<T::AccountId>,
        reward: T::Balance,
        target: &T::AccountId,
    ) -> Result<(), DispatchError> {
        Budget::unreserve(&job_id, reward)
            .map_err(|_| DispatchError::Other("Severe Error: JobBudget::unreserve failed"))?;

        let pallet_account: T::AccountId = <T as Config>::PalletId::get().into_account_truncating();

        // Extract fee from the processor reward
        let fee_percentage = AssetSplit::get_fee_percentage(); // TODO: fee will be indexed by version in the future
        let fee = fee_percentage.mul_floor(reward);

        // Subtract the fee from the reward
        let reward_after_fee = reward - fee;

        // Transfer fees to Acurast fees manager account
        let fee_pallet_account: T::AccountId = AssetSplit::pallet_id().into_account_truncating();

        Currency::transfer(
            &pallet_account,
            &fee_pallet_account,
            fee.saturated_into::<<Currency as fungible::Inspect<T::AccountId>>::Balance>(),
            Preservation::Preserve,
        )?;
        Currency::transfer(
            &pallet_account,
            target,
            reward_after_fee
                .saturated_into::<<Currency as fungible::Inspect<T::AccountId>>::Balance>(),
            Preservation::Preserve,
        )?;

        Ok(())
    }

    fn pay_matcher_reward(
        remaining_rewards: Vec<(JobId<T::AccountId>, T::Balance)>,
        matcher: &T::AccountId,
    ) -> Result<(), DispatchError> {
        let matcher_fee_percentage = AssetSplit::get_matcher_percentage(); // TODO: fee will be indexed by version in the future

        let mut matcher_reward: T::Balance = 0u8.into();
        for (job_id, remaining_reward) in remaining_rewards.into_iter() {
            let matcher_fee = matcher_fee_percentage.mul_floor(remaining_reward);
            Budget::unreserve(&job_id, matcher_fee)
                .map_err(|_| DispatchError::Other("Severe Error: JobBudget::unreserve failed"))?;
            matcher_reward += matcher_fee;
        }

        let pallet_account: T::AccountId = <T as Config>::PalletId::get().into_account_truncating();

        // Extract fee from the matcher reward
        let fee_percentage = AssetSplit::get_fee_percentage(); // TODO: fee will be indexed by version in the future
        let fee = fee_percentage.mul_floor(matcher_reward);

        // Subtract the fee from the reward
        let reward_after_fee = matcher_reward - fee;

        // Transfer fees to Acurast fees manager account
        let fee_pallet_account: T::AccountId = AssetSplit::pallet_id().into_account_truncating();

        Currency::transfer(
            &pallet_account,
            &fee_pallet_account,
            fee.saturated_into::<<Currency as fungible::Inspect<T::AccountId>>::Balance>(),
            Preservation::Preserve,
        )?;
        Currency::transfer(
            &pallet_account,
            matcher,
            reward_after_fee
                .saturated_into::<<Currency as fungible::Inspect<T::AccountId>>::Balance>(),
            Preservation::Preserve,
        )?;

        Ok(())
    }

    fn refund(job_id: &JobId<T::AccountId>) -> Result<T::Balance, DispatchError> {
        let remaining = Budget::unreserve_remaining(&job_id);
        // Send remaining funds to the job creator
        let pallet_account: T::AccountId = <T as Config>::PalletId::get().into_account_truncating();
        let hyperdrive_pallet_account: T::AccountId =
            <T as Config>::HyperdrivePalletId::get().into_account_truncating();
        match &job_id.0 {
            MultiOrigin::Acurast(who) => {
                Currency::transfer(
                    &pallet_account,
                    who,
                    remaining.saturated_into(),
                    Preservation::Preserve,
                )?;
            }
            MultiOrigin::Tezos(_) | MultiOrigin::Ethereum(_) | MultiOrigin::AlephZero(_) => {
                Currency::transfer(
                    &pallet_account,
                    &hyperdrive_pallet_account,
                    remaining.saturated_into(),
                    Preservation::Preserve,
                )?;
            }
        };

        Ok(remaining)
    }
}

/// Manages each job's budget by reserving/unreserving rewards that are externally strored, e.g. on a pallet account in `pallet_balances`.
pub trait JobBudget<T: frame_system::Config + Config> {
    fn reserve(job_id: &JobId<T::AccountId>, reward: T::Balance) -> Result<(), ()>;

    /// Unreserve exactly `reward` from reserved balance and fails if this exceeds the reserved amount.
    fn unreserve(job_id: &JobId<T::AccountId>, reward: T::Balance) -> Result<(), ()>;

    /// Unreserves the remaining balance.
    fn unreserve_remaining(job_id: &JobId<T::AccountId>) -> T::Balance;

    /// The reserved amount.
    fn reserved(job_id: &JobId<T::AccountId>) -> T::Balance;
}
