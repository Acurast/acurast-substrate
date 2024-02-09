use crate::Config;
use frame_support::BoundedVec;
use pallet_acurast::{AllowedSources, MultiOrigin};

/// Checks if a consumer is whitelisted/
pub(crate) fn is_consumer_whitelisted<T: Config>(
    consumer: &MultiOrigin<T::AccountId>,
    allowed_consumers: &Option<BoundedVec<MultiOrigin<T::AccountId>, T::MaxAllowedConsumers>>,
) -> bool {
    allowed_consumers
        .as_ref()
        .map(|allowed_consumers| {
            allowed_consumers
                .iter()
                .any(|allowed_consumer| allowed_consumer == consumer)
        })
        .unwrap_or(true)
}

/// Checks if a source/processor is whitelisted
pub fn is_source_whitelisted<T: Config>(
    source: &T::AccountId,
    allowed_sources: &Option<AllowedSources<T::AccountId, T::MaxAllowedSources>>,
) -> bool {
    allowed_sources
        .as_ref()
        .map(|allowed_sources| {
            allowed_sources
                .iter()
                .any(|allowed_source| allowed_source == source)
        })
        .unwrap_or(true)
}
