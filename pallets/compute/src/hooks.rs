use frame_support::traits::{Currency, ExistenceRequirement, IsType};
use sp_runtime::traits::{One, Saturating, Zero};
use sp_std::prelude::*;

use acurast_common::{ComputeHooks, MetricInput};

use crate::*;

impl<T: Config<I>, I: 'static> ComputeHooks<T::AccountId, T::ManagerId, BalanceFor<T, I>>
	for Pallet<T, I>
{
	fn commit(
		processor: &T::AccountId,
		manager: &(T::AccountId, T::ManagerId),
		metrics: &[MetricInput],
	) -> BalanceFor<T, I>
	where
		BalanceFor<T, I>: IsType<u128>,
	{
		let pool_ids = (1..=Self::last_metric_pool_id()).collect::<Vec<_>>();
		let cycle = Self::current_cycle();
		let reward = Self::do_commit(processor, manager, metrics, pool_ids.as_slice(), cycle);

		if !reward.is_zero() {
			<ManagerMetricRewards<T, I>>::mutate(manager.1, |reward_state| {
				let state = reward_state.get_or_insert(MetricsRewardStateFor::<T, I> {
					paid: Zero::zero(),
					claimed: Zero::zero(),
				});
				#[allow(clippy::bind_instead_of_map)]
				let _ = T::Currency::transfer(
					&Self::account_id(),
					&manager.0,
					reward,
					ExistenceRequirement::KeepAlive,
				)
				.and_then(|_| {
					state.paid = state.paid.saturating_add(reward);
					Ok(())
				});
				state.claimed = cycle.epoch.saturating_sub(One::one());
			});
		}

		reward
	}
}
