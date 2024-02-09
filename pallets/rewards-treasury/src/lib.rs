#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[cfg(test)]
pub mod mock;
#[cfg(test)]
mod stub;
#[cfg(test)]
mod tests;

#[frame_support::pallet]
pub mod pallet {
    use frame_support::traits::fungible::Inspect;
    use frame_support::traits::tokens::{Fortitude, Precision, Preservation};
    use frame_support::{
        pallet_prelude::*,
        traits::{tokens::fungible::Mutate, Get},
    };
    use frame_system::pallet_prelude::BlockNumberFor;
    use pallet_balances;
    use sp_std::prelude::*;

    use crate::*;

    /// Configure the pallet by specifying the parameters and types on which it depends.
    #[pallet::config]
    pub trait Config<I: 'static = ()>: frame_system::Config + pallet_balances::Config<I> {
        /// Because this pallet emits events, it depends on the runtime's definition of an event.
        type RuntimeEvent: From<Event<Self, I>>
            + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        /// The epoch length in blocks. At each epoch's end the penultimate (last but one) balance is burnt.
        #[pallet::constant]
        type Epoch: Get<BlockNumberFor<Self>>;
        /// The ID for this pallet
        #[pallet::constant]
        type Treasury: Get<<Self as frame_system::Config>::AccountId>;
    }

    #[pallet::storage]
    #[pallet::getter(fn penultimate_balance)]
    pub(super) type PenultimateBalance<T: Config<I>, I: 'static = ()> =
        StorageValue<_, T::Balance, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn latest_burn)]
    pub(super) type LatestBurn<T: Config<I>, I: 'static = ()> =
        StorageValue<_, BlockNumberFor<T>, ValueQuery>;

    #[pallet::pallet]
    pub struct Pallet<T, I = ()>(PhantomData<(T, I)>);

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config<I>, I: 'static = ()> {
        /// Burnt penultimate epoch's accumulated balance from treasury. [amount_burnt]
        BurntFromTreasuryAtEndOfEpoch(T::Balance),
    }

    // Errors inform users that something went wrong.
    #[pallet::error]
    pub enum Error<T, I = ()> {}

    #[pallet::hooks]
    impl<T: Config<I>, I: 'static> Hooks<BlockNumberFor<T>> for Pallet<T, I> {
        fn on_finalize(_: BlockNumberFor<T>) {}

        fn on_initialize(current_block: BlockNumberFor<T>) -> Weight {
            // check for <= (not ==) to ensure burns still happen when T::Epoch gets increased or decreased
            let latest_burn_at = Self::latest_burn();
            let epoch = T::Epoch::get();
            if latest_burn_at + epoch <= current_block {
                (match <PenultimateBalance<T, I>>::try_mutate(
                    |penultimate_balance| -> Result<T::Balance, DispatchError> {
                        let actual_burnt = <pallet_balances::Pallet<T, I> as Mutate<_>>::burn_from(
                            &T::Treasury::get(),
                            penultimate_balance.to_owned(),
                            Precision::BestEffort,
                            Fortitude::Polite,
                        )?;
                        <LatestBurn<T, I>>::put(current_block);

                        *penultimate_balance =
                            <pallet_balances::Pallet<T, I> as Inspect<_>>::reducible_balance(
                                &T::Treasury::get(),
                                Preservation::Preserve,
                                Fortitude::Polite,
                            );

                        Ok(actual_burnt)
                    },
                ) {
                    Ok(actual_burnt) => {
                        Self::deposit_event(Event::BurntFromTreasuryAtEndOfEpoch(actual_burnt));
                    }
                    Err(e) => {
                        log::error!(
                            target: "runtime::pallet_acurast_rewards_treasury",
                            "Error reducing treasury balance: {:?}",
                            e,
                        );
                    }
                });
                // burn_from (2 reads, 2 writes) + self (2 reads, 2 writes)
                T::DbWeight::get().reads_writes(4, 4)
            } else {
                T::DbWeight::get().reads(1)
            }
        }
    }

    #[pallet::call]
    impl<T: Config<I>, I: 'static> Pallet<T, I> {}
}
