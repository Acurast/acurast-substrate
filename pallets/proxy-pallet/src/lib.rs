#![cfg_attr(not(feature = "std"), no_std)]

/// Edit this file to define custom logic or remove it if it is not needed.
/// Learn more about FRAME and the core library of Substrate FRAME pallets:
/// <https://docs.substrate.io/v3/runtime/frame>
pub use pallet::*;



#[frame_support::pallet]
pub mod pallet {
	use frame_support::{dispatch::DispatchResultWithPostInfo, pallet_prelude::*};
	use frame_system::pallet_prelude::*;
	use xcm::v2::Instruction::Transact;
	use xcm::v2::{OriginKind, SendError};
	use xcm::v2::{Junction::{Parachain}, SendXcm, Xcm, Junctions::{X1}};
	use frame_support::inherent::Vec;
	/// Configure the pallet by specifying the parameters and types on which it depends.
	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// Because this pallet emits events, it depends on the runtime's definition of an event.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
		type AcurastParachainId: Get<u32>;
		type AcurastPalletId: Get<u32>;
		type XcmSender: SendXcm;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	// The pallet's runtime storage items.
	// https://docs.substrate.io/v3/runtime/storage
	#[pallet::storage]
	#[pallet::getter(fn something)]
	// Learn more about declaring storage items:
	// https://docs.substrate.io/v3/runtime/storage#declaring-storage-items
	pub type TestStorage<T> = StorageValue<_, u32>;

	// Pallets use events to inform users when important changes are made.
	// https://docs.substrate.io/v3/runtime/events-and-errors
	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Event documentation should end with an array that provides descriptive names for event
		/// parameters. [something, who]
		TestStored(u32, T::AccountId),
	}

	// Errors inform users that something went wrong.
	#[pallet::error]
	pub enum Error<T> {
		/// Error names should be descriptive.
		NoneValue,
		/// Errors should have helpful documentation associated with them.
		StorageOverflow,
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	// Dispatchable functions allows users to interact with the pallet and invoke state changes.
	// These functions materialize as "extrinsics", which are often compared to transactions.
	// Dispatchable functions must be annotated with a weight and must return a DispatchResult.
	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// An example dispatchable that takes a singles value as a parameter, writes the value to
		/// storage and emits an event. This function must be dispatched by a signed extrinsic.
		#[pallet::weight(10_000 + T::DbWeight::get().writes(1))]
		pub fn test_store(origin: OriginFor<T>, something: u32) -> DispatchResultWithPostInfo {
			// Check that the extrinsic was signed and get the signer.
			// This function will return an error if the extrinsic is not signed.
			// https://docs.substrate.io/v3/runtime/origins
			let who = ensure_signed(origin)?;
			let acurast_pallet_id = <T as Config>::AcurastPalletId::get();
			let proxy_call = ProxyCall::test_store{something};

			// send instruction to acurast parachain
			let _call_result = match acurast_call::<T>(proxy_call, acurast_pallet_id) {
				Ok(_) => {
					log::info!("success on acurast_call");
					"success" // TODO: success case
				},
				Err(_) => {
					log::info!("fail on acurast_call");
					"fail" // TODO: error case
				}
			};

			// Emit an event.
			Self::deposit_event(Event::TestStored(something, who));
			// Return a successful DispatchResultWithPostInfo
			Ok(().into())
		}

		/// An example dispatchable that may throw a custom error.
		#[pallet::weight(10_000 + T::DbWeight::get().reads_writes(1,1))]
		pub fn test_cause_error(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
			let _who = ensure_signed(origin)?;

			// Read a value from storage.
			match <TestStorage<T>>::get() {
				// Return an error if the value has not been set.
				None => Err(Error::<T>::NoneValue)?,
				Some(old) => {
					// Increment the value read from storage; will error in the event of overflow.
					let new = old.checked_add(1).ok_or(Error::<T>::StorageOverflow)?;
					// Update the value in storage with the incremented result.
					<TestStorage<T>>::put(new);
					Ok(().into())
				},
			}
		}

		// #[pallet::weight(10_000 + T::DbWeight::get().writes(1))]
		// pub fn test_three(origin: OriginFor<T>, something: u32, x: Vec<u32>, y: bool, z: TestEnum) -> DispatchResultWithPostInfo {
		// 	// Check that the extrinsic was signed and get the signer.
		// 	// This function will return an error if the extrinsic is not signed.
		// 	// https://docs.substrate.io/v3/runtime/origins
		// 	let who = ensure_signed(origin)?;
		//
		// 	// Update storage.
		// 	<TestStorage<T>>::put(something);
		//
		// 	// Emit an event.
		// 	Self::deposit_event(Event::TestStored(something, who));
		// 	// Return a successful DispatchResultWithPostInfo
		// 	Ok(().into())
		// }
	}

	#[derive(Clone, Eq, PartialEq, Encode, Decode, RuntimeDebug, TypeInfo)]
	#[allow(non_camel_case_types)]
	pub enum ProxyCall {
			#[codec(index = 0u8)]
		test_store { something: u32 },
		#[codec(index = 1u8)]
		test_cause_error {},
		// #[codec(index = 2u8)]
		// test_three {
		// 	something: u32,
		// 	x: Vec<u32>,
		// 	y: bool,
		// 	z: TestEnum,
		// },
	}

	// Extracted common functionality of sending an xcm commanding the parachain to update its storage
	pub fn acurast_call<T: Config>(proxy: ProxyCall, pallet_id: u32) -> Result<(), SendError> {
		let mut xcm_message = Vec::new();

		// create an encoded version of the call composed of the first byte being the pallet id
		// on the destination chain, second byte the position of the calling function on the enum,
		// and then the arguments SCALE encoded in order
		let mut encoded_call = Vec::<u8>::new();
		encoded_call.push(pallet_id as u8);
		encoded_call.append(&mut proxy.encode());
		log::info!("encoded call is: {:?}", encoded_call);
		// put our transact message in the vector of instructions
		xcm_message.push(Transact {
			origin_type: OriginKind::Xcm,
			require_weight_at_most: 1_000_000_000 as u64,
			call: encoded_call.into(),
		});

		// get acurast parachain id from the config defined in the runtime file
		let acurast_id: u32 = <T as Config>::AcurastParachainId::get();

		// use router to send the xcm message
		return match T::XcmSender::send_xcm(
			(1, X1(Parachain(acurast_id))),
			Xcm(xcm_message),
		) {
			Ok(_) => {
				Ok(())
			},
			Err(e) => {
				Err(e)
			},
		};
	}

	#[derive(Clone, Eq, PartialEq, Encode, Decode, RuntimeDebug, TypeInfo)]
	pub enum TestEnum {
		A(u32),
		B
	}
}
