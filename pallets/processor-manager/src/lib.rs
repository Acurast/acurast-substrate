#![cfg_attr(not(feature = "std"), no_std)]

mod functions;
mod migration;
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
pub use pallet::*;
pub use traits::*;
pub use types::*;

pub(crate) use pallet::STORAGE_VERSION;

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
	use core::ops::Div;

	use acurast_common::{ComputeHooks, ListUpdateOperation, ManagerIdProvider, Version};
	use frame_support::{
		dispatch::DispatchResultWithPostInfo,
		pallet_prelude::*,
		sp_runtime::{
			traits::{CheckedAdd, IdentifyAccount, StaticLookup, Verify},
			Saturating,
		},
		traits::{tokens::Balance, Get, UnixTime},
		Blake2_128, Blake2_128Concat, Parameter,
	};
	use frame_system::{
		ensure_root, ensure_signed,
		pallet_prelude::{BlockNumberFor, OriginFor},
	};
	use parity_scale_codec::MaxEncodedLen;
	use sp_std::prelude::*;

	#[cfg(feature = "runtime-benchmarks")]
	use crate::benchmarking::BenchmarkHelper;
	use crate::{
		traits::*, BinaryHash, Metrics, ProcessorList, ProcessorPairingFor, ProcessorUpdatesFor,
		RewardDistributionSettings, RewardDistributionWindow, UpdateInfo,
	};

	/// Configure the pallet by specifying the parameters and types on which it depends.
	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// Because this pallet emits events, it depends on the runtime's definition of an event.
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
		type Proof: Parameter + Member + Verify + MaxEncodedLen;
		type ManagerId: Parameter + Member + MaxEncodedLen + Copy + CheckedAdd + From<u128>;
		type ManagerIdProvider: ManagerIdProvider<Self::AccountId, Self::ManagerId>;
		type ComputeHooks: ComputeHooks<Self::AccountId, Self::Balance>;
		type ProcessorAssetRecovery: ProcessorAssetRecovery<Self>;
		type MaxPairingUpdates: Get<u32>;
		type MaxProcessorsInSetUpdateInfo: Get<u32>;
		type Counter: Parameter + Member + MaxEncodedLen + Copy + CheckedAdd + Ord + From<u8>;
		type PairingProofExpirationTime: Get<u128>;
		type Advertisement: Parameter + Member;
		type AdvertisementHandler: AdvertisementHandler<Self>;
		type UnixTime: UnixTime;
		type Balance: Parameter + IsType<u128> + Div + Balance + MaybeSerializeDeserialize;
		type ProcessorRewardDistributor: ProcessorRewardDistributor<Self>;
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
			Self { managers: Default::default() }
		}
	}

	#[pallet::genesis_build]
	impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
		fn build(&self) {
			for (manager, processors) in &self.managers {
				let manager_id =
					T::ManagerIdProvider::manager_id_for(manager).unwrap_or_else(|_| {
						// Get the latest manager identifier in the sequence.
						let id = <LastManagerId<T>>::get().unwrap_or(0.into()) + 1.into();

						// Using .expect here should be fine it is only applied at the genesis block.
						T::ManagerIdProvider::create_manager_id(id, manager)
							.expect("Could not create manager id.");

						// Update sequential manager identifier
						<LastManagerId<T>>::set(Some(id));

						id
					});

				processors.iter().for_each(|processor| {
					// Set manager/processor indexes
					<ManagedProcessors<T>>::insert(manager_id, processor, ());
					<ProcessorToManagerIdIndex<T>>::insert(processor, manager_id);

					// Update the processor counter for the manager
					let counter =
						<ManagerCounter<T>>::get(manager).unwrap_or(0u8.into()) + 1.into();
					<ManagerCounter<T>>::insert(manager, counter);
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

	/// Current api version to be used.
	///
	/// This is a single version number allowing to switch quickly between supported parachain API versions, within one processor build (without a forced OTA update):
	/// - The `api_version` should be read out regularly by processors to select the implementation compatible with the current runtime API (and storage structure).
	///   Thus, the processor must receive a OTA update adding support for future `api_version`(s) yet to be deployed by an Acurast Parachain runtime upgrade.
	/// - The version number must be increased on backwards incompatible changes on a runtime upgrade, **by means of a migration** to make it synchronous with the runtime upgrade.
	///   **All processors that have not installed a build to support this version will break.**
	/// - There is a permissioned extrinsic to reduce the `api_version` to react to processors breaking upon a runtime upgrade.
	///   This is only a valid rollback strategy if the storage format did not change backwards incompatibly.
	#[pallet::storage]
	#[pallet::getter(fn api_version)]
	pub(super) type ApiVersion<T: Config> = StorageValue<_, u32, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn known_binary_hash)]
	pub(super) type KnownBinaryHash<T: Config> =
		StorageMap<_, Blake2_128Concat, Version, BinaryHash>;

	#[pallet::storage]
	#[pallet::getter(fn processor_update_info)]
	pub(super) type ProcessorUpdateInfo<T: Config> =
		StorageMap<_, Blake2_128Concat, T::AccountId, UpdateInfo>;

	#[pallet::storage]
	#[pallet::getter(fn processor_reward_distribution_settings)]
	pub(super) type ProcessorRewardDistributionSettings<T: Config> =
		StorageValue<_, RewardDistributionSettings<T::Balance, T::AccountId>, OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn processor_reward_distribution_window)]
	pub(super) type ProcessorRewardDistributionWindow<T: Config> =
		StorageMap<_, Blake2_128Concat, T::AccountId, RewardDistributionWindow>;

	#[pallet::storage]
	#[pallet::getter(fn processor_min_version_for_reward)]
	pub(super) type ProcessorMinVersionForReward<T: Config> =
		StorageMap<_, Blake2_128Concat, u32, u32>;

	pub(crate) const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

	#[pallet::pallet]
	#[pallet::storage_version(STORAGE_VERSION)]
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
		/// Set update info for processors. [manager_account_id, update_info]
		ProcessorUpdateInfoSet(T::AccountId, UpdateInfo),
		/// Set api version used by processors. [api_version]
		ApiVersionUpdated(u32),
		/// Reward has been sent to processor. [processor_account_id, amount]
		ProcessorRewardSent(T::AccountId, T::Balance),
		/// Updated the minimum required processor version to receive rewards.
		MinProcessorVersionForRewardUpdated(Version),
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

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn on_runtime_upgrade() -> Weight {
			crate::migration::migrate::<T>()
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
					},
					ListUpdateOperation::Remove => {
						Self::do_remove_processor_manager_pairing(&update.item.account, &who)?
					},
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

			let now = T::UnixTime::now().as_millis();

			<ProcessorHeartbeat<T>>::insert(&who, now);
			<ProcessorVersion<T>>::insert(&who, version);

			Self::deposit_event(Event::<T>::ProcessorHeartbeatWithVersion(who.clone(), version));

			if Self::is_elegible_for_reward(&who, &version) {
				if let Some(amount) = Self::do_reward_distribution(&who) {
					Self::deposit_event(Event::<T>::ProcessorRewardSent(who, amount));
				}
			}

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
				<KnownBinaryHash<T>>::insert(version, *hash);
			} else {
				<KnownBinaryHash<T>>::remove(version)
			}

			Self::deposit_event(Event::<T>::BinaryHashUpdated(version, hash));

			Ok(().into())
		}

		#[pallet::call_index(7)]
		#[pallet::weight(T::WeightInfo::update_api_version())]
		pub fn update_api_version(
			origin: OriginFor<T>,
			api_version: u32,
		) -> DispatchResultWithPostInfo {
			ensure_root(origin)?;

			<ApiVersion<T>>::put(api_version);

			Self::deposit_event(Event::<T>::ApiVersionUpdated(api_version));

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

			_ = Self::known_binary_hash(update_info.version)
				.ok_or(Error::<T>::UnknownProcessorVersion)?;

			for processor in processors {
				_ = Self::ensure_managed(&who, &processor)?;
				<ProcessorUpdateInfo<T>>::insert(&processor, update_info.clone());
			}

			Self::deposit_event(Event::<T>::ProcessorUpdateInfoSet(who, update_info));

			Ok(().into())
		}

		#[pallet::call_index(9)]
		#[pallet::weight(T::WeightInfo::update_reward_distribution_settings())]
		pub fn update_reward_distribution_settings(
			origin: OriginFor<T>,
			new_settings: Option<RewardDistributionSettings<T::Balance, T::AccountId>>,
		) -> DispatchResultWithPostInfo {
			ensure_root(origin)?;
			<ProcessorRewardDistributionSettings<T>>::set(new_settings);

			Ok(().into())
		}

		#[pallet::call_index(10)]
		#[pallet::weight(T::WeightInfo::update_min_processor_version_for_reward())]
		pub fn update_min_processor_version_for_reward(
			origin: OriginFor<T>,
			version: Version,
		) -> DispatchResultWithPostInfo {
			ensure_root(origin)?;
			<ProcessorMinVersionForReward<T>>::insert(version.platform, version.build_number);
			Self::deposit_event(Event::<T>::MinProcessorVersionForRewardUpdated(version));

			Ok(().into())
		}

		/// Heartbeats with version and metrics.
		///
		/// # Errros
		///
		/// This extrinsic **skips errors** arising due to governance misconfiguration or processor version mismatch which e.g. could result in metrics provided for an inexistent pool.
		///
		/// # Events
		///
		/// We do not emit separate `ProcessorHeartbeatWithMetrics` for backwards compatibility of clients.
		/// The version field allows to know if this event (and the potential subsequent ProcessorRewardSent) is emitted from [`Self::heartbeat_with_version`] or [`Self::heartbeat_with_metrics`] .
		#[pallet::call_index(11)]
		#[pallet::weight(T::WeightInfo::heartbeat_with_metrics(metrics.len() as u32))]
		pub fn heartbeat_with_metrics(
			origin: OriginFor<T>,
			version: Version,
			metrics: Metrics,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			_ = Self::manager_id_for_processor(&who).ok_or(Error::<T>::ProcessorHasNoManager)?;

			let now = T::UnixTime::now().as_millis();

			<ProcessorHeartbeat<T>>::insert(&who, now);
			<ProcessorVersion<T>>::insert(&who, version);

			Self::deposit_event(Event::<T>::ProcessorHeartbeatWithVersion(who.clone(), version));

			if Self::is_elegible_for_reward(&who, &version) {
				let total_amount = match (
					Self::do_reward_distribution(&who),
					T::ComputeHooks::commit(&who, metrics.into_iter()),
				) {
					(Some(h), Some(c)) => Some(h.saturating_add(c)),
					(Some(h), None) => Some(h),
					(None, Some(c)) => Some(c),
					(None, None) => None,
				};

				if let Some(total_amount) = total_amount {
					Self::deposit_event(Event::<T>::ProcessorRewardSent(who, total_amount));
				}
			}

			Ok(().into())
		}
	}
}

sp_api::decl_runtime_apis! {
	/// API to interact with Acurast marketplace pallet.
	pub trait ProcessorManagerRuntimeApi<AccountId: parity_scale_codec::Codec, ManagerId: parity_scale_codec::Codec> {
		 fn processor_update_infos(
			source: AccountId,
		) -> Result<UpdateInfos, RuntimeApiError>;

		fn manager_id_for_processor(
			source: AccountId,
		) -> Result<ManagerId, RuntimeApiError>;
	}
}
