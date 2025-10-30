use core::marker::PhantomData;

use frame_support::{
	sp_runtime::{
		traits::{AccountIdConversion, Get},
		DispatchError, Percent, SaturatedConversion,
	},
	traits::{
		fungible::{Balanced, Credit},
		tokens::{fungible, Fortitude, Precision, Preservation},
		OnUnbalanced,
	},
	PalletId,
};
use sp_std::prelude::*;

use pallet_acurast::{JobId, MultiOrigin};

use crate::Config;

/// Trait used to manage lock up and payments of rewards.
pub trait RewardManager<T: frame_system::Config + Config> {
	fn lock_reward(
		job_id: &JobId<T::AccountId>,
		reward: <T as Config>::Balance,
	) -> Result<(), DispatchError>;
	fn handle_reward(
		job_id: &JobId<T::AccountId>,
		reward: <T as Config>::Balance,
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

	fn handle_reward(
		_job_id: &JobId<T::AccountId>,
		_reward: <T as Config>::Balance,
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

// This trait provides methods for managing the fees.
pub trait FeeManager {
	fn get_fee_percentage() -> Percent;
	fn get_matcher_percentage() -> Percent;
	fn pallet_id() -> PalletId;
}

pub struct AssetRewardManager<AssetSplit, Currency, JobBudget, OU>(
	PhantomData<(AssetSplit, Currency, JobBudget, OU)>,
);

impl<T, AssetSplit, Currency, Budget, OU> RewardManager<T>
	for AssetRewardManager<AssetSplit, Currency, Budget, OU>
where
	T: Config + frame_system::Config,
	AssetSplit: FeeManager,
	Currency: fungible::Mutate<T::AccountId, Balance = T::Balance>
		+ Balanced<T::AccountId, Balance = T::Balance>,
	Budget: JobBudget<T>,
	OU: OnUnbalanced<Credit<T::AccountId, Currency>>,
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
			},
			MultiOrigin::Tezos(_)
			| MultiOrigin::Ethereum(_)
			| MultiOrigin::AlephZero(_)
			| MultiOrigin::Vara(_)
			| MultiOrigin::Ethereum20(_)
			| MultiOrigin::Solana(_) => {
				// The availability of these funds was ensured on the target chain side
				Currency::transfer(
					&hyperdrive_pallet_account,
					&pallet_account,
					reward.saturated_into(),
					Preservation::Preserve,
				)?;
			},
			MultiOrigin::AcurastCanary(_) => {
				return Err(DispatchError::Other("Unexpected MultiOrigin"));
			},
		};

		Budget::reserve(job_id, reward)
			.map_err(|_| DispatchError::Other("Severe Error: JobBudget::reserve failed"))?;

		Ok(())
	}

	fn handle_reward(
		job_id: &JobId<T::AccountId>,
		reward: T::Balance,
	) -> Result<(), DispatchError> {
		Budget::unreserve(job_id, reward)
			.map_err(|_| DispatchError::Other("Severe Error: JobBudget::unreserve failed"))?;

		let pallet_account: T::AccountId = <T as Config>::PalletId::get().into_account_truncating();

		let imbalance = Currency::withdraw(
			&pallet_account,
			reward,
			Precision::Exact,
			Preservation::Preserve,
			Fortitude::Polite,
		)?;

		OU::on_unbalanced(imbalance);

		Ok(())
	}

	fn pay_matcher_reward(
		remaining_rewards: Vec<(JobId<T::AccountId>, T::Balance)>,
		matcher: &T::AccountId,
	) -> Result<(), DispatchError> {
		let matcher_fee_percentage = AssetSplit::get_matcher_percentage();

		let mut matcher_reward: T::Balance = 0u8.into();
		for (job_id, remaining_reward) in remaining_rewards.into_iter() {
			let matcher_fee = matcher_fee_percentage.mul_floor(remaining_reward);
			Budget::unreserve(&job_id, matcher_fee)
				.map_err(|_| DispatchError::Other("Severe Error: JobBudget::unreserve failed"))?;
			matcher_reward += matcher_fee;
		}

		let pallet_account: T::AccountId = <T as Config>::PalletId::get().into_account_truncating();

		// Extract fee from the matcher reward
		let fee_percentage = AssetSplit::get_fee_percentage();
		let fee = fee_percentage.mul_floor(matcher_reward);

		// Subtract the fee from the reward
		let reward_after_fee = matcher_reward - fee;

		// Transfer fees to Acurast fees manager account
		let fee_pallet_account: T::AccountId = AssetSplit::pallet_id().into_account_truncating();

		if fee.gt(&(0u128.into())) {
			Currency::transfer(
				&pallet_account,
				&fee_pallet_account,
				fee.saturated_into::<<Currency as fungible::Inspect<T::AccountId>>::Balance>(),
				Preservation::Preserve,
			)?;
		}

		if reward_after_fee.gt(&(0u128).into()) {
			Currency::transfer(
				&pallet_account,
				matcher,
				reward_after_fee
					.saturated_into::<<Currency as fungible::Inspect<T::AccountId>>::Balance>(),
				Preservation::Preserve,
			)?;
		}

		Ok(())
	}

	fn refund(job_id: &JobId<T::AccountId>) -> Result<T::Balance, DispatchError> {
		let remaining = Budget::unreserve_remaining(job_id);
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
			},
			MultiOrigin::Tezos(_)
			| MultiOrigin::Ethereum(_)
			| MultiOrigin::AlephZero(_)
			| MultiOrigin::Vara(_)
			| MultiOrigin::Ethereum20(_)
			| MultiOrigin::Solana(_) => {
				Currency::transfer(
					&pallet_account,
					// TODO refunded amount is collected on hyperdrive_pallet_account but not yet refunded to proxy chain
					&hyperdrive_pallet_account,
					remaining.saturated_into(),
					Preservation::Preserve,
				)?;
			},
			MultiOrigin::AcurastCanary(_) => {
				return Err(DispatchError::Other("Unexpected MultiOrigin"));
			},
		};

		Ok(remaining)
	}
}

/// Manages each job's budget by reserving/unreserving rewards that are externally strored, e.g. on a pallet account in `pallet_balances`.
pub trait JobBudget<T: frame_system::Config + Config> {
	#[allow(clippy::result_unit_err)]
	fn reserve(job_id: &JobId<T::AccountId>, reward: T::Balance) -> Result<(), ()>;

	/// Unreserve exactly `reward` from reserved balance and fails if this exceeds the reserved amount.
	#[allow(clippy::result_unit_err)]
	fn unreserve(job_id: &JobId<T::AccountId>, reward: T::Balance) -> Result<(), ()>;

	/// Unreserves the remaining balance.
	fn unreserve_remaining(job_id: &JobId<T::AccountId>) -> T::Balance;

	/// The reserved amount.
	fn reserved(job_id: &JobId<T::AccountId>) -> T::Balance;
}
