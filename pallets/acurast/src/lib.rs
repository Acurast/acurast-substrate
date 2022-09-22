#![cfg_attr(not(feature = "std"), no_std)]

mod primitives;

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use codec::{Decode, Encode, FullCodec};
	use core::fmt::Debug;
	use frame_support::{
		dispatch::DispatchResultWithPostInfo, pallet_prelude::*, sp_runtime::traits::StaticLookup,
		storage::bounded_vec::BoundedVec, Blake2_128Concat,
	};
	use frame_system::pallet_prelude::*;
	use scale_info::TypeInfo;
	use sp_std::prelude::*;

	pub trait FulfillmentRouter<T: Config> {
		fn received_fulfillment(
			from: T::AccountId,
			fulfillment: Fulfillment,
			registration: Registration<T::RegistrationExtra>,
			requester: <T::Lookup as StaticLookup>::Target,
		);
	}

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
		type RegistrationExtra: FullCodec
			+ MaxEncodedLen
			+ TypeInfo
			+ Sized
			+ Clone
			+ PartialEq
			+ Debug;
		type FulfillmentRouter: FulfillmentRouter<Self>;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	#[derive(RuntimeDebug, Encode, Decode, TypeInfo, Clone, PartialEq)]
	pub struct Fulfillment {
		pub script: BoundedVec<u8, ConstU32<53>>,
		pub payload: Vec<u8>,
	}

	#[derive(RuntimeDebug, Encode, Decode, MaxEncodedLen, TypeInfo, Clone, PartialEq)]
	pub struct Registration<T>
	where
		T: FullCodec + MaxEncodedLen + TypeInfo + Sized + Clone + PartialEq + Debug,
	{
		pub script: BoundedVec<u8, ConstU32<53>>,
		pub extra: T,
	}

	#[pallet::storage]
	#[pallet::getter(fn stored_registration)]
	pub type StoredRegistration<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::AccountId,
		Blake2_128Concat,
		BoundedVec<u8, ConstU32<53>>,
		Registration<T::RegistrationExtra>,
	>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A registration was successfully stored. [registration, who]
		RegistrationStored(Registration<T::RegistrationExtra>, T::AccountId),
		/// A registration was successfully removed. [registration, who]
		RegistrationRemoved(BoundedVec<u8, ConstU32<53>>, T::AccountId),
		/// A fulfillment has been posted. [who, fulfillment, registration, receiver]
		ReceivedFulfillment(
			T::AccountId,
			Fulfillment,
			Registration<T::RegistrationExtra>,
			T::AccountId,
		),
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Fulfill was exececuted for a not registered job.
		RegistrationNotFound,
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(10_000 + T::DbWeight::get().writes(1))]
		pub fn register(
			origin: OriginFor<T>,
			registration: Registration<T::RegistrationExtra>,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			<StoredRegistration<T>>::insert(
				who.clone(),
				(&registration).script.clone(),
				registration.clone(),
			);
			Self::deposit_event(Event::RegistrationStored(registration, who));
			Ok(().into())
		}

		#[pallet::weight(10_000 + T::DbWeight::get().writes(1))]
		pub fn deregister(
			origin: OriginFor<T>,
			script: BoundedVec<u8, ConstU32<53>>,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			<StoredRegistration<T>>::remove(who.clone(), script.clone());
			Self::deposit_event(Event::RegistrationRemoved(script, who));
			Ok(().into())
		}

		#[pallet::weight(10_000 + T::DbWeight::get().reads(1))]
		pub fn fulfill(
			origin: OriginFor<T>,
			fulfillment: Fulfillment,
			requester: <T::Lookup as StaticLookup>::Source,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			let requester = T::Lookup::lookup(requester)?;
			let registration =
				<StoredRegistration<T>>::get(requester.clone(), (&fulfillment).script.clone())
					.ok_or(Error::<T>::RegistrationNotFound)?;
			T::FulfillmentRouter::received_fulfillment(
				who.clone(),
				fulfillment.clone(),
				registration.clone(),
				requester.clone(),
			);
			Self::deposit_event(Event::ReceivedFulfillment(
				who,
				fulfillment,
				registration,
				requester,
			));
			Ok(().into())
		}
	}
}
