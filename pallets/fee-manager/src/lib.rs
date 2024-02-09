#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
mod traits;
pub mod weights;

use core::ops::AddAssign;

use frame_support::traits::Get;
use sp_arithmetic::Percent;

pub use pallet::*;
pub use traits::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;

    #[pallet::pallet]
    pub struct Pallet<T, I = ()>(PhantomData<(T, I)>);

    #[pallet::config]
    pub trait Config<I: 'static = ()>: frame_system::Config {
        type RuntimeEvent: From<Event<Self, I>>
            + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        #[pallet::constant]
        type DefaultFeePercentage: Get<Percent>;
        type UpdateOrigin: EnsureOrigin<Self::RuntimeOrigin>;
        type WeightInfo: WeightInfo;
    }

    #[pallet::type_value]
    pub fn DefaultFeePercentage<T: Config<I>, I: 'static>() -> Percent {
        T::DefaultFeePercentage::get()
    }

    #[pallet::storage]
    #[pallet::getter(fn fee_percentage)]
    pub type FeePercentage<T: Config<I>, I: 'static = ()> =
        StorageMap<_, Blake2_128, u16, Percent, ValueQuery, DefaultFeePercentage<T, I>>;

    #[pallet::storage]
    #[pallet::getter(fn fee_version)]
    pub type Version<T: Config<I>, I: 'static = ()> = StorageValue<_, u16, ValueQuery>;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config<I>, I: 'static = ()> {
        FeeUpdated { version: u16, fee: Percent },
    }

    #[pallet::call]
    impl<T: Config<I>, I: 'static> Pallet<T, I> {
        /// Updates the fee percentage. Can only be called by a privileged/root account.
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::update_fee_percentage())]
        pub fn update_fee_percentage(origin: OriginFor<T>, fee: Percent) -> DispatchResult {
            T::UpdateOrigin::ensure_origin(origin)?;
            let (new_version, _) = Self::set_fee_percentage(fee);
            Self::deposit_event(Event::FeeUpdated {
                version: new_version,
                fee,
            });
            Ok(())
        }
    }
}

impl<T: Config<I>, I: 'static> Pallet<T, I> {
    /// Sets the fee percentage in storage.
    pub fn set_fee_percentage(fee: Percent) -> (u16, u64) {
        let new_version = <Version<T, I>>::mutate(|version| {
            version.add_assign(1);
            *version
        });
        <FeePercentage<T, I>>::set(new_version, fee);
        (new_version, T::DbWeight::get().write)
    }
}
