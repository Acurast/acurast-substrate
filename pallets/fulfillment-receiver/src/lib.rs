#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

#[cfg(any(test, feature = "runtime-benchmarks"))]
mod stub;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

pub mod traits;
pub mod types;

pub use acurast_common::is_valid_script;
pub use pallet::*;
pub use types::*;

#[frame_support::pallet]
pub mod pallet {
    use crate::Fulfillment;
    use frame_support::{dispatch::DispatchResultWithPostInfo, pallet_prelude::*};
    use frame_system::{ensure_signed, pallet_prelude::OriginFor};
    use sp_std::prelude::*;

    use crate::traits::*;

    /// Configure the pallet by specifying the parameters and types on which it depends.
    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// Because this pallet emits events, it depends on the runtime's definition of an event.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        /// Handler to notify the runtime when a new fulfillment is received.
        type OnFulfillment: OnFulfillment<Self>;
        /// Weight Info for extrinsics.
        type WeightInfo: WeightInfo;
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        FulfillReceived(T::AccountId, Fulfillment),
    }

    // Errors inform users that something went wrong.
    #[pallet::error]
    pub enum Error<T> {
        FulfillmentRejected,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Submit a fulfillment for an acurast job.
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::fulfill())]
        pub fn fulfill(
            origin: OriginFor<T>,
            fulfillment: Fulfillment,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;

            // Notify the runtime about the fulfillment.
            let info = T::OnFulfillment::on_fulfillment(who.clone(), fulfillment.clone())?;
            Self::deposit_event(Event::FulfillReceived(who, fulfillment));
            Ok(info)
        }
    }
}
