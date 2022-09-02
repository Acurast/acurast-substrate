#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use codec::{Decode, Encode};
	use frame_support::{
		dispatch::DispatchResultWithPostInfo,
		ensure,
		pallet_prelude::*,
		sp_runtime::traits::{MaybeDisplay, StaticLookup},
		storage::bounded_vec::BoundedVec,
		Blake2_128Concat,
	};
	use frame_system::pallet_prelude::*;
	use scale_info::TypeInfo;
	use sp_std::prelude::*;

	/// This trait provides the interface for a fulfillment router.
	pub trait FulfillmentRouter<T: Config> {
		fn received_fulfillment(
			origin: OriginFor<T>,
			from: T::AccountId,
			fulfillment: Fulfillment,
			registration: Registration<T::AccountId, T::RegistrationExtra>,
			requester: <T::Lookup as StaticLookup>::Target,
		) -> DispatchResultWithPostInfo;
	}

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
		/// Extra structure to include in the registration of a job.
		type RegistrationExtra: Parameter + Member + MaxEncodedLen;
		/// The fulfillment router to route a job fulfillment to its final destination.
		type FulfillmentRouter: FulfillmentRouter<Self>;
		/// The max length of the allowed sources list for a registration.
		#[pallet::constant]
		type MaxAllowedSources: Get<u16>;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	#[pallet::without_storage_info]
	pub struct Pallet<T>(_);

	const SCRIPT_PREFIX: &'static [u8] = b"ipfs://";
	const SCRIPT_LENGTH: u32 = 53;

	/// Type representing the utf8 bytes of a string containing the value of an ipfs url.
	/// The ipfs url is expected to point to a script.
	pub type Script = BoundedVec<u8, ConstU32<SCRIPT_LENGTH>>;

	/// Structure representing a job fulfillment. It contains the script that generated the payload and the actual payload.
	#[derive(RuntimeDebug, Encode, Decode, TypeInfo, Clone, PartialEq)]
	pub struct Fulfillment {
		/// The script that generated the payload.
		pub script: Script,
		/// The output of a script.
		pub payload: Vec<u8>,
	}

	/// Structure representing a job registration.
	#[derive(RuntimeDebug, Encode, Decode, MaxEncodedLen, TypeInfo, Clone, PartialEq)]
	pub struct Registration<A, T>
	where
		A: Parameter + Member + MaybeSerializeDeserialize + MaybeDisplay + Ord + MaxEncodedLen,
		T: Parameter + Member + MaxEncodedLen,
	{
		/// The script to execute. It is a vector of bytes representing a utf8 string. The string needs to be a ipfs url that points to the script.
		pub script: Script,
		/// An optional array of the [AccountId]s allowed to fulfill the job. If the array is [None], then all sources are allowed.
		pub allowed_sources: Option<Vec<A>>,
		/// A boolean indicating if only verified sources can fulfill the job. A verified source is one that has provided a valid key attestation.
		pub allow_only_verified_sources: bool,
		/// Extra parameters. This type can be configured through [Config::RegistrationExtra].
		pub extra: T,
	}

	/// Structure used to updated the allowed sources list of a [Registration].
	#[derive(RuntimeDebug, Encode, Decode, MaxEncodedLen, TypeInfo, Clone, PartialEq)]
	pub struct AllowedSourcesUpdate<A>
	where
		A: Parameter + Member + MaybeSerializeDeserialize + MaybeDisplay + Ord + MaxEncodedLen,
	{
		/// The update operation
		pub operation: AllowedSourcesUpdateOperation,
		/// The [AccountId] to add or remove.
		pub account_id: A,
	}

	/// The allowed sources update operation.
	#[derive(RuntimeDebug, Encode, Decode, MaxEncodedLen, TypeInfo, Clone, PartialEq, Copy)]
	pub enum AllowedSourcesUpdateOperation {
		Add,
		Remove,
	}

	/// The storage for [Registration]s. They are stored by [AccountId] and [Script].
	#[pallet::storage]
	#[pallet::getter(fn stored_registration)]
	pub type StoredRegistration<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::AccountId,
		Blake2_128Concat,
		Script,
		Registration<T::AccountId, T::RegistrationExtra>,
	>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A registration was successfully stored. [registration, who]
		RegistrationStored(Registration<T::AccountId, T::RegistrationExtra>, T::AccountId),
		/// A registration was successfully removed. [registration, who]
		RegistrationRemoved(Script, T::AccountId),
		/// A fulfillment has been posted. [who, fulfillment, registration, receiver]
		ReceivedFulfillment(
			T::AccountId,
			Fulfillment,
			Registration<T::AccountId, T::RegistrationExtra>,
			T::AccountId,
		),
		/// The allowed sources have been updated. [who, old_registration, allowed_sources, operation]
		AllowedSourcesUpdated(
			T::AccountId,
			Registration<T::AccountId, T::RegistrationExtra>,
			Vec<AllowedSourcesUpdate<T::AccountId>>,
		),
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Fulfill was executed for a not registered job.
		RegistrationNotFound,
		/// The source of the fulfill is not allowed for the job.
		FulfillSourceNotAllowed,
		/// The allowed soruces list for a registration exeeded the max length.
		TooManyAllowedSources,
		/// The allowed soruces list for a registration cannot be empty if provided.
		TooFewAllowedSources,
		/// The provided script value is not valid. The value needs to be and ipfs:// url.
		InvalidScriptValue,
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Registers a job by providing a [Registration]. If a job for the same script was previously registered, it will be overwritten.
		#[pallet::weight(10_000 + T::DbWeight::get().writes(1))]
		pub fn register(
			origin: OriginFor<T>,
			registration: Registration<T::AccountId, T::RegistrationExtra>,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			let script_len = (&registration).script.len() as u32;
			ensure!(
				script_len == SCRIPT_LENGTH && (&registration).script.starts_with(SCRIPT_PREFIX),
				Error::<T>::InvalidScriptValue
			);
			let allowed_sources_len = (&registration)
				.allowed_sources
				.as_ref()
				.map(|sources| sources.len())
				.unwrap_or(0);
			let max_allowed_sources_len = T::MaxAllowedSources::get() as usize;
			ensure!(allowed_sources_len > 0, Error::<T>::TooFewAllowedSources);
			ensure!(
				allowed_sources_len <= max_allowed_sources_len,
				Error::<T>::TooManyAllowedSources
			);
			<StoredRegistration<T>>::insert(
				who.clone(),
				(&registration).script.clone(),
				registration.clone(),
			);
			Self::deposit_event(Event::RegistrationStored(registration, who));
			Ok(().into())
		}

		/// Deregisters a job for the given script.
		#[pallet::weight(10_000 + T::DbWeight::get().writes(1))]
		pub fn deregister(origin: OriginFor<T>, script: Script) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			<StoredRegistration<T>>::remove(who.clone(), script.clone());
			Self::deposit_event(Event::RegistrationRemoved(script, who));
			Ok(().into())
		}

		/// Updates the allowed sources list of a [Registration].
		#[pallet::weight(10_000 + T::DbWeight::get().reads_writes(1, 1))]
		pub fn update_allowed_sources(
			origin: OriginFor<T>,
			script: Script,
			updates: Vec<AllowedSourcesUpdate<T::AccountId>>,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			let registration = <StoredRegistration<T>>::get(who.clone(), script.clone())
				.ok_or(Error::<T>::RegistrationNotFound)?;

			let mut current_allowed_sources =
				(&registration).allowed_sources.clone().unwrap_or(vec![]);
			for update in &updates {
				let position =
					current_allowed_sources.iter().position(|value| value == &update.account_id);
				match (position, update.operation) {
					(None, AllowedSourcesUpdateOperation::Add) => {
						current_allowed_sources.push(update.account_id.clone())
					},
					(Some(pos), AllowedSourcesUpdateOperation::Remove) => {
						current_allowed_sources.remove(pos);
					},
					_ => {},
				}
			}
			let allowed_sources = if current_allowed_sources.is_empty() {
				None
			} else {
				Some(current_allowed_sources)
			};
			<StoredRegistration<T>>::insert(
				who.clone(),
				script.clone(),
				Registration {
					script,
					allowed_sources,
					extra: (&registration).extra.clone(),
					allow_only_verified_sources: (&registration).allow_only_verified_sources,
				},
			);

			Self::deposit_event(Event::AllowedSourcesUpdated(who, registration, updates));

			Ok(().into())
		}

		/// Fulfills a previously registered job.
		#[pallet::weight(10_000 + T::DbWeight::get().reads(1))]
		pub fn fulfill(
			origin: OriginFor<T>,
			fulfillment: Fulfillment,
			requester: <T::Lookup as StaticLookup>::Source,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin.clone())?;
			let requester = T::Lookup::lookup(requester)?;

			let registration =
				<StoredRegistration<T>>::get(requester.clone(), (&fulfillment).script.clone())
					.ok_or(Error::<T>::RegistrationNotFound)?;

			ensure_source_allowed::<T>(&who, &registration)?;

			let info = T::FulfillmentRouter::received_fulfillment(
				origin,
				who.clone(),
				fulfillment.clone(),
				registration.clone(),
				requester.clone(),
			)?;
			Self::deposit_event(Event::ReceivedFulfillment(
				who,
				fulfillment,
				registration,
				requester,
			));
			Ok(info)
		}
	}

	fn ensure_source_allowed<T: Config>(
		source: &T::AccountId,
		registration: &Registration<T::AccountId, T::RegistrationExtra>,
	) -> Result<(), Error<T>> {
		registration
			.allowed_sources
			.as_ref()
			.map(|allowed_sources| {
				allowed_sources
					.iter()
					.position(|allowed_source| allowed_source == source)
					.map(|_| ())
					.ok_or(Error::<T>::FulfillSourceNotAllowed)
			})
			.unwrap_or(Ok(()))
	}
}
