use acurast_common::{ComputeHooks, MetricInput};

use crate::*;

impl<T: Config<I>, I: 'static> ComputeHooks<T::AccountId, T::Balance> for Pallet<T, I> {
	fn commit(
		processor: &T::AccountId,
		metrics: impl IntoIterator<Item = MetricInput>,
	) -> Option<T::Balance> {
		let reward = match Self::do_claim(processor, (1..=Self::last_metric_pool_id()).collect()) {
			Ok(r) => r,
			Err(_) => None,
		};

		Self::do_commit(processor, metrics);

		reward
	}
}
