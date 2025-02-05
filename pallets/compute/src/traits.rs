use frame_support::{
	pallet_prelude::{DispatchResult, Weight},
	sp_runtime::DispatchError,
};
use sp_runtime::{traits::Zero, Perquintill};

use crate::{Config, EpochOf};

pub trait ComputeRewardDistributor<T: Config<I>, I: 'static> {
	/// Calculates the reward without transferring.
	///
	/// # Arguments
	///
	/// * `ratio` - The ratio of the total reward to be distributed for `epoch`. The actual amount to distribute by epoch can come from a (dynamic) source like inflation per epoch.
	/// * `epoch` - The global epoch for which this reward is distributed.
	fn calculate_reward(
		ratio: Perquintill,
		epoch: EpochOf<T, I>,
	) -> Result<T::Balance, DispatchError>;

	/// Calculates the reward without transferring.
	///
	/// # Arguments
	///
	/// * `processor` - The processor generating this reward, it will be paid out to manager.
	/// * `amount` - The amount to distribute (transfer).
	fn distribute_reward(processor: &T::AccountId, amount: T::Balance) -> DispatchResult;

	fn is_elegible_for_reward(processor: &T::AccountId) -> bool;
}

impl<T: Config<I>, I: 'static> ComputeRewardDistributor<T, I> for () {
	fn calculate_reward(
		_ratio: Perquintill,
		_epoch: EpochOf<T, I>,
	) -> Result<T::Balance, DispatchError> {
		Ok(Zero::zero())
	}

	fn distribute_reward(_processor: &T::AccountId, _amount: T::Balance) -> DispatchResult {
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
}
