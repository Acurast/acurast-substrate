#![cfg_attr(not(feature = "std"), no_std)]

mod functions;
mod traits;
mod types;

#[cfg(test)]
pub mod mock;
#[cfg(feature = "std")]
pub mod rpc;
#[cfg(any(test, feature = "runtime-benchmarks"))]
mod stub;
#[cfg(test)]
mod tests;

pub mod weights;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

#[cfg(feature = "runtime-benchmarks")]
pub use benchmarking::BenchmarkHelper;
use frame_support::BoundedVec;
pub use functions::*;
pub use pallet::*;
pub use traits::*;
pub use types::*;

pub type ProcessorPairingFor<T> =
    ProcessorPairing<<T as frame_system::Config>::AccountId, <T as Config>::Proof>;
pub type ProcessorPairingUpdateFor<T> =
    ProcessorPairingUpdate<<T as frame_system::Config>::AccountId, <T as Config>::Proof>;

pub type ProcessorUpdatesFor<T> =
    BoundedVec<ProcessorPairingUpdateFor<T>, <T as Config>::MaxPairingUpdates>;
pub type ProcessorList<T> =
    BoundedVec<<T as frame_system::Config>::AccountId, <T as Config>::MaxProcessorsInSetUpdateInfo>;

#[frame_support::pallet]
pub mod pallet {
    #[cfg(feature = "runtime-benchmarks")]
    use crate::benchmarking::BenchmarkHelper;
    use acurast_common::ListUpdateOperation;
    use codec::MaxEncodedLen;
    use frame_support::sp_runtime;
    use frame_support::{
        dispatch::DispatchResultWithPostInfo,
        pallet_prelude::{Member, *},
        sp_runtime::traits::{CheckedAdd, IdentifyAccount, StaticLookup, Verify},
        traits::{Get, UnixTime},
        Blake2_128, Parameter,
    };
    use frame_system::{ensure_root, ensure_signed, pallet_prelude::OriginFor};
    use sp_std::prelude::*;

    use crate::{
        traits::*, BinaryHash, ProcessorList, ProcessorPairingFor, ProcessorUpdatesFor, UpdateInfo,
        Version,
    };

    /// Configure the pallet by specifying the parameters and types on which it depends.
    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// Because this pallet emits events, it depends on the runtime's definition of an event.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        type Proof: Parameter + Member + Verify + MaxEncodedLen;
        type ManagerId: Parameter + Member + MaxEncodedLen + Copy + CheckedAdd + From<u128>;
        type ManagerIdProvider: ManagerIdProvider<Self>;
        type ProcessorAssetRecovery: ProcessorAssetRecovery<Self>;
        type MaxPairingUpdates: Get<u32>;
        type MaxProcessorsInSetUpdateInfo: Get<u32>;
        type Counter: Parameter + Member + MaxEncodedLen + Copy + CheckedAdd + Ord + From<u8>;
        type PairingProofExpirationTime: Get<u128>;
        type Advertisement: Parameter + Member;
        type AdvertisementHandler: AdvertisementHandler<Self>;
        /// Timestamp
        type UnixTime: UnixTime;
        /// Weight Info for extrinsics.
        type WeightInfo: WeightInfo;
        #[cfg(feature = "runtime-benchmarks")]
        type BenchmarkHelper: BenchmarkHelper<Self>;
    }

    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        pub managers: Vec<(T::AccountId, Vec<T::AccountId>)>,
    }

    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            Self {
                managers: Default::default(),
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
        fn build(&self) {
            for (manager, processors) in &self.managers {
                let manager_id =
                    T::ManagerIdProvider::manager_id_for(&manager).unwrap_or_else(|_| {
                        // Get the latest manager identifier in the sequence.
                        let id = <LastManagerId<T>>::get().unwrap_or(0.into()) + 1.into();

                        // Using .expect here should be fine it is only applied at the genesis block.
                        T::ManagerIdProvider::create_manager_id(id, &manager)
                            .expect("Could not create manager id.");

                        // Update sequential manager identifier
                        <LastManagerId<T>>::set(Some(id));

                        id
                    });

                processors.iter().for_each(|processor| {
                    // Set manager/processor indexes
                    <ManagedProcessors<T>>::insert(manager_id, &processor, ());
                    <ProcessorToManagerIdIndex<T>>::insert(&processor, manager_id);

                    // Update the processor counter for the manager
                    let counter =
                        <ManagerCounter<T>>::get(&manager).unwrap_or(0u8.into()) + 1.into();
                    <ManagerCounter<T>>::insert(&manager, counter);
                });
            }
        }
    }

    #[pallet::storage]
    #[pallet::getter(fn last_manager_id)]
    pub(super) type LastManagerId<T: Config> = StorageValue<_, T::ManagerId>;

    #[pallet::storage]
    #[pallet::getter(fn managed_processors)]
    pub(super) type ManagedProcessors<T: Config> =
        StorageDoubleMap<_, Blake2_128Concat, T::ManagerId, Blake2_128Concat, T::AccountId, ()>;

    #[pallet::storage]
    #[pallet::getter(fn manager_id_for_processor)]
    pub(super) type ProcessorToManagerIdIndex<T: Config> =
        StorageMap<_, Blake2_128, T::AccountId, T::ManagerId>;

    #[pallet::storage]
    #[pallet::getter(fn counter_for_manager)]
    pub(super) type ManagerCounter<T: Config> = StorageMap<_, Blake2_128, T::AccountId, T::Counter>;

    #[pallet::storage]
    #[pallet::getter(fn processor_last_seen)]
    pub(super) type ProcessorHeartbeat<T: Config> = StorageMap<_, Blake2_128, T::AccountId, u128>;

    #[pallet::storage]
    #[pallet::getter(fn processor_version)]
    pub(super) type ProcessorVersion<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, Version>;

    #[pallet::storage]
    #[pallet::getter(fn known_binary_hash)]
    pub(super) type KnownBinaryHash<T: Config> =
        StorageMap<_, Blake2_128Concat, Version, BinaryHash>;

    #[pallet::storage]
    #[pallet::getter(fn processor_update_info)]
    pub(super) type ProcessorUpdateInfo<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, UpdateInfo>;

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Manager id created. [manager_account_id, manager_id]
        ManagerCreated(T::AccountId, T::ManagerId),
        /// Processor pairing updated. [manager_account_id, updates]
        ProcessorPairingsUpdated(T::AccountId, ProcessorUpdatesFor<T>),
        /// Processor pairing updated. [processor_account_id, destination]
        ProcessorFundsRecovered(T::AccountId, T::AccountId),
        /// Processor paired. [processor_account_id, pairing]
        ProcessorPaired(T::AccountId, ProcessorPairingFor<T>),
        /// Heartbeat. [processor_account_id]
        ProcessorHeartbeat(T::AccountId),
        /// Processor advertisement. [manager_account_id, processor_account_id, advertisement]
        ProcessorAdvertisement(T::AccountId, T::AccountId, T::Advertisement),
        /// Heartbeat with version information. [processor_account_id, version]
        ProcessorHeartbeatWithVersion(T::AccountId, Version),
        /// Binary hash updated. [version, binary_hash]
        BinaryHashUpdated(Version, Option<BinaryHash>),
        /// Set update info for processor. [manager_account_id, update_info]
        ProcessorUpdateInfoSet(T::AccountId, UpdateInfo),
    }

    // Errors inform users that something went wrong.
    #[pallet::error]
    pub enum Error<T> {
        FailedToCreateManagerId,
        ProcessorAlreadyPaired,
        ProcessorPairedWithAnotherManager,
        InvalidPairingProof,
        ProcessorHasNoManager,
        CounterOverflow,
        PairingProofExpired,
        UnknownProcessorVersion,
    }

    impl<T: Config> Pallet<T> {
        fn ensure_managed(
            manager: &T::AccountId,
            processor: &T::AccountId,
        ) -> Result<T::ManagerId, DispatchError> {
            let manager_id = T::ManagerIdProvider::manager_id_for(manager)?;
            let processor_manager_id = Self::manager_id_for_processor(processor)
                .ok_or(Error::<T>::ProcessorHasNoManager)?;

            if manager_id != processor_manager_id {
                return Err(Error::<T>::ProcessorPairedWithAnotherManager)?;
            }

            Ok(manager_id)
        }
    }

    #[pallet::call]
    impl<T: Config> Pallet<T>
    where
        T::AccountId: IsType<<<T::Proof as Verify>::Signer as IdentifyAccount>::AccountId>,
    {
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::update_processor_pairings(pairing_updates.len() as u32))]
        pub fn update_processor_pairings(
            origin: OriginFor<T>,
            pairing_updates: ProcessorUpdatesFor<T>,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;

            let (manager_id, created) = Self::do_get_or_create_manager_id(&who)?;
            if created {
                Self::deposit_event(Event::<T>::ManagerCreated(who.clone(), manager_id));
            }

            for update in &pairing_updates {
                match update.operation {
                    ListUpdateOperation::Add => {
                        if !update.item.validate_timestamp::<T>() {
                            #[cfg(not(feature = "runtime-benchmarks"))]
                            return Err(Error::<T>::PairingProofExpired)?;
                        }
                        let counter = Self::counter_for_manager(&who)
                            .unwrap_or(0u8.into())
                            .checked_add(&1u8.into())
                            .ok_or(Error::<T>::CounterOverflow)?;
                        if !update.item.validate_signature::<T>(&who, counter) {
                            #[cfg(not(feature = "runtime-benchmarks"))]
                            return Err(Error::<T>::InvalidPairingProof)?;
                        }
                        Self::do_add_processor_manager_pairing(&update.item.account, manager_id)?;
                        <ManagerCounter<T>>::insert(&who, counter);
                    }
                    ListUpdateOperation::Remove => {
                        Self::do_remove_processor_manager_pairing(&update.item.account, manager_id)?
                    }
                }
            }

            Self::deposit_event(Event::<T>::ProcessorPairingsUpdated(who, pairing_updates));

            Ok(().into())
        }

        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::pair_with_manager())]
        pub fn pair_with_manager(
            origin: OriginFor<T>,
            pairing: ProcessorPairingFor<T>,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;

            if !pairing.validate_timestamp::<T>() {
                #[cfg(not(feature = "runtime-benchmarks"))]
                return Err(Error::<T>::PairingProofExpired)?;
            }

            let (manager_id, created) = Self::do_get_or_create_manager_id(&pairing.account)?;
            if created {
                Self::deposit_event(Event::<T>::ManagerCreated(
                    pairing.account.clone(),
                    manager_id,
                ));
            }

            let counter = Self::counter_for_manager(&pairing.account)
                .unwrap_or(0u8.into())
                .checked_add(&1u8.into())
                .ok_or(Error::<T>::CounterOverflow)?;

            if !pairing.validate_signature::<T>(&pairing.account, counter) {
                #[cfg(not(feature = "runtime-benchmarks"))]
                return Err(Error::<T>::InvalidPairingProof)?;
            }
            Self::do_add_processor_manager_pairing(&who, manager_id)?;
            <ManagerCounter<T>>::insert(&pairing.account, counter);

            Self::deposit_event(Event::<T>::ProcessorPaired(who, pairing));

            Ok(().into())
        }

        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::recover_funds())]
        pub fn recover_funds(
            origin: OriginFor<T>,
            processor: <T::Lookup as StaticLookup>::Source,
            destination: <T::Lookup as StaticLookup>::Source,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            let processor_account_id = <T::Lookup as StaticLookup>::lookup(processor)?;
            _ = Self::ensure_managed(&who, &processor_account_id)?;
            let destination_account_id = <T::Lookup as StaticLookup>::lookup(destination)?;

            T::ProcessorAssetRecovery::recover_assets(
                &processor_account_id,
                &destination_account_id,
            )?;

            Self::deposit_event(Event::<T>::ProcessorFundsRecovered(
                processor_account_id,
                destination_account_id,
            ));

            Ok(().into())
        }

        #[pallet::call_index(3)]
        #[pallet::weight(T::WeightInfo::heartbeat())]
        pub fn heartbeat(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            _ = Self::manager_id_for_processor(&who).ok_or(Error::<T>::ProcessorHasNoManager)?;

            <ProcessorHeartbeat<T>>::insert(&who, T::UnixTime::now().as_millis());

            Self::deposit_event(Event::<T>::ProcessorHeartbeat(who));

            Ok(().into())
        }

        #[pallet::call_index(4)]
        #[pallet::weight(T::WeightInfo::advertise_for())]
        pub fn advertise_for(
            origin: OriginFor<T>,
            processor: <T::Lookup as StaticLookup>::Source,
            advertisement: T::Advertisement,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            let processor_account_id = <T::Lookup as StaticLookup>::lookup(processor)?;
            _ = Self::ensure_managed(&who, &processor_account_id)?;

            T::AdvertisementHandler::advertise_for(&processor_account_id, &advertisement)?;

            Self::deposit_event(Event::<T>::ProcessorAdvertisement(
                who,
                processor_account_id,
                advertisement,
            ));

            Ok(().into())
        }

        #[pallet::call_index(5)]
        #[pallet::weight(T::WeightInfo::heartbeat_with_version())]
        pub fn heartbeat_with_version(
            origin: OriginFor<T>,
            version: Version,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            _ = Self::manager_id_for_processor(&who).ok_or(Error::<T>::ProcessorHasNoManager)?;

            <ProcessorHeartbeat<T>>::insert(&who, T::UnixTime::now().as_millis());
            <ProcessorVersion<T>>::insert(&who, version.clone());

            Self::deposit_event(Event::<T>::ProcessorHeartbeatWithVersion(who, version));

            Ok(().into())
        }

        #[pallet::call_index(6)]
        #[pallet::weight(T::WeightInfo::update_binary_hash())]
        pub fn update_binary_hash(
            origin: OriginFor<T>,
            version: Version,
            hash: Option<BinaryHash>,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;

            if let Some(hash) = &hash {
                <KnownBinaryHash<T>>::insert(&version, hash.clone());
            } else {
                <KnownBinaryHash<T>>::remove(&version)
            }

            Self::deposit_event(Event::<T>::BinaryHashUpdated(version, hash));

            Ok(().into())
        }

        #[pallet::call_index(8)]
        #[pallet::weight(T::WeightInfo::set_processor_update_info(processors.len() as u32))]
        pub fn set_processor_update_info(
            origin: OriginFor<T>,
            update_info: UpdateInfo,
            processors: ProcessorList<T>,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;

            _ = Self::known_binary_hash(&update_info.version)
                .ok_or(Error::<T>::UnknownProcessorVersion)?;

            for processor in processors {
                _ = Self::ensure_managed(&who, &processor)?;
                <ProcessorUpdateInfo<T>>::insert(&processor, update_info.clone());
            }

            Self::deposit_event(Event::<T>::ProcessorUpdateInfoSet(who, update_info));

            Ok(().into())
        }
    }
}

sp_api::decl_runtime_apis! {
    /// API to interact with Acurast marketplace pallet.
    pub trait ProcessorManagerRuntimeApi<AccountId: codec::Codec, ManagerId: codec::Codec> {
         fn processor_update_infos(
            source: AccountId,
        ) -> Result<UpdateInfos, RuntimeApiError>;

        fn manager_id_for_processor(
            source: AccountId,
        ) -> Result<ManagerId, RuntimeApiError>;
    }
}
