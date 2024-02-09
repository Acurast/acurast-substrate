use frame_support::traits::Get;
use sp_std::fmt;
use sp_std::prelude::*;

/// A bound that can be used to restrict length sequence types such as [`frame_support::BoundedVec`] appearing in types used in dispatchable functions.
///
/// Similar to [`frame_support::Parameter`] without encoding traits, since bounds are never encoded.
pub trait ParameterBound: Get<u32> + Clone + Eq + fmt::Debug + scale_info::TypeInfo {}
impl<T> ParameterBound for T where T: Get<u32> + Clone + Eq + fmt::Debug + scale_info::TypeInfo {}
