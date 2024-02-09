use frame_support::{ensure, pallet_prelude::DispatchResult};
use reputation::BetaParameters;
use sp_core::Get;

use crate::{
    AdvertisementFor, AdvertisementRestriction, Config, Error, Pallet, StoredAdvertisementPricing,
    StoredAdvertisementRestriction, StoredReputation, StoredStorageCapacity,
};

impl<T: Config> Pallet<T> {
    pub fn do_advertise(
        processor: &T::AccountId,
        advertisement: &AdvertisementFor<T>,
    ) -> DispatchResult {
        if let Some(allowed_consumers) = &advertisement.allowed_consumers {
            let max_allowed_consumers_len = T::MaxAllowedSources::get() as usize;
            ensure!(
                allowed_consumers.len() > 0,
                Error::<T>::TooFewAllowedConsumers
            );
            ensure!(
                allowed_consumers.len() <= max_allowed_consumers_len,
                Error::<T>::TooManyAllowedConsumers
            );
        }

        // update capacity to save on operations when checking available capacity
        if let Some(old) = <StoredAdvertisementRestriction<T>>::get(processor) {
            // allow capacity to become negative (in which case source remains assigned but does not receive new jobs assigned)
            <StoredStorageCapacity<T>>::mutate(processor, |c| {
                // new remaining capacity = new total capacity - (old total capacity - old remaining capacity) = old remaining capacity + new total capacity - old total capacity
                *c = Some(
                    c.unwrap_or(0)
                        .checked_add(advertisement.storage_capacity as i64)
                        .unwrap_or(i64::MAX)
                        .checked_sub(old.storage_capacity as i64)
                        .unwrap_or(0),
                )
            });
        } else {
            <StoredStorageCapacity<T>>::insert(processor, advertisement.storage_capacity as i64);
        }

        <StoredAdvertisementRestriction<T>>::insert(
            &processor,
            AdvertisementRestriction {
                max_memory: advertisement.max_memory,
                network_request_quota: advertisement.network_request_quota,
                storage_capacity: advertisement.storage_capacity,
                allowed_consumers: advertisement.allowed_consumers.clone(),
                available_modules: advertisement.available_modules.clone(),
            },
        );
        // update separate pricing index
        <StoredAdvertisementPricing<T>>::insert(processor, advertisement.pricing.clone());
        <StoredReputation<T>>::mutate(processor, |r| {
            if r.is_none() {
                *r = Some(BetaParameters::default());
            }
        });

        Ok(().into())
    }
}
