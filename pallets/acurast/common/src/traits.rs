use frame_support::{
	sp_runtime::{DispatchError, DispatchResult},
	traits::Get,
};
use sp_std::{fmt, prelude::*};

use crate::MetricInput;

/// A bound that can be used to restrict length sequence types such as [`frame_support::BoundedVec`] appearing in types used in dispatchable functions.
///
/// Similar to [`frame_support::Parameter`] without encoding traits, since bounds are never encoded.
pub trait ParameterBound: Get<u32> + Clone + Eq + fmt::Debug + scale_info::TypeInfo {}
impl<T> ParameterBound for T where T: Get<u32> + Clone + Eq + fmt::Debug + scale_info::TypeInfo {}

pub trait ManagerIdProvider<AccountId, ManagerId> {
	fn create_manager_id(id: ManagerId, owner: &AccountId) -> DispatchResult;
	fn manager_id_for(owner: &AccountId) -> Result<ManagerId, DispatchError>;
	fn owner_for(manager_id: ManagerId) -> Result<AccountId, DispatchError>;
}

/// A trait to describe hooks the `pallet_acruast_compute` provides.
pub trait ComputeHooks<AccountId, Balance> {
	/// Commits compute for current processor epoch by providing benchmarked results for a (sub)set of metrics.
	///
	/// **The caller has to ensure the passed processor is allowed to commit**.
	///
	/// Metrics are specified with the `pool_name` and an lookup will map the names to their corresponding `pool_id`.
	///
	/// # Errors
	///
	/// **Unknown pools are silently skipped.**
	fn commit(
		processor: &AccountId,
		metrics: impl IntoIterator<Item = MetricInput>,
	) -> Option<Balance>;
}

impl<AccountId, Balance> ComputeHooks<AccountId, Balance> for () {
	fn commit(
		_processor: &AccountId,
		_metrics: impl IntoIterator<Item = MetricInput>,
	) -> Option<Balance> {
		None
	}
}
