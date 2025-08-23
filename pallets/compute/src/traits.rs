use frame_support::pallet_prelude::{DispatchResult, Weight};

use crate::{BalanceFor, Config};

pub trait ComputeRewardDistributor<T: Config<I>, I: 'static> {
	/// Calculates the reward without transferring.
	///
	/// # Arguments
	///
	/// * `processor` - The processor generating this reward, it will be paid out to manager.
	/// * `amount` - The amount to distribute (transfer).
	fn distribute_reward(processor: &T::AccountId, amount: BalanceFor<T, I>) -> DispatchResult;

	fn is_elegible_for_reward(processor: &T::AccountId) -> bool;
}

impl<T: Config<I>, I: 'static> ComputeRewardDistributor<T, I> for () {
	fn distribute_reward(_processor: &T::AccountId, _amount: BalanceFor<T, I>) -> DispatchResult {
		Ok(())
	}

	fn is_elegible_for_reward(_processor: &<T>::AccountId) -> bool {
		true
	}
}

pub trait WeightInfo {
	fn create_pool(x: u32) -> Weight;
	fn modify_pool_same_config() -> Weight;
	fn modify_pool_replace_config(x: u32) -> Weight;
	fn modify_pool_update_config(x: u32) -> Weight;
	fn update_reward_distribution_settings() -> Weight;
}

impl WeightInfo for () {
	fn create_pool(_x: u32) -> Weight {
		Weight::from_parts(10_000, 0)
	}

	fn modify_pool_same_config() -> Weight {
		Weight::from_parts(10_000, 0)
	}

	fn modify_pool_replace_config(_x: u32) -> Weight {
		Weight::from_parts(10_000, 0)
	}

	fn modify_pool_update_config(_x: u32) -> Weight {
		Weight::from_parts(10_000, 0)
	}

	fn update_reward_distribution_settings() -> Weight {
		Weight::from_parts(10_000, 0)
	}
}
