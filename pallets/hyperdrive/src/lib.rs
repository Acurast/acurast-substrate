#![cfg_attr(not(feature = "std"), no_std)]

extern crate core;

pub use pallet::*;
pub use traits::*;
pub use types::*;

#[cfg(test)]
mod mock;
#[cfg(any(test, feature = "runtime-benchmarks"))]
mod stub;
#[cfg(test)]
mod substrate_tests;
#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
mod traits;

pub mod chain;

mod types;
pub mod weights;

#[frame_support::pallet]
pub mod pallet {
	use chain::substrate::{
		SubstrateMessageDecoder, SubstrateMessageDecoderError, SubstrateMessageEncoder,
		SubstrateMessageEncoderError,
	};
	use frame_support::{pallet_prelude::*, sp_runtime::traits::AtLeast32BitUnsigned, traits::Get};
	use frame_system::pallet_prelude::*;
	use pallet_acurast_hyperdrive_ibc::{
		ContractCall, Layer, MessageBody, OutgoingMessageWithMetaFor, Subject, SubjectFor,
	};
	use pallet_acurast_marketplace::RegistrationExtra;
	use sp_runtime::traits::Hash;
	use sp_std::{prelude::*, vec};

	use super::*;

	/// A instantiable pallet for receiving secure state synchronizations into Acurast.
	#[pallet::pallet]
	pub struct Pallet<T, I = ()>(PhantomData<(T, I)>);

	/// Configures the pallet instance for a specific target chain from which we synchronize state into Acurast.
	#[pallet::config]
	pub trait Config<I: 'static = ()>:
		frame_system::Config + pallet_acurast::Config + pallet_acurast_hyperdrive_ibc::Config<I>
	{
		type RuntimeEvent: From<Event<Self, I>>
			+ IsType<<Self as frame_system::Config>::RuntimeEvent>;

		type ActionExecutor: ActionExecutor<Self>;
		type Sender: Get<Self::AccountId>;
		type ParsableAccountId: Into<Self::AccountId> + TryFrom<Vec<u8>>;
		type AlephZeroContract: Get<Self::AccountId>;
		type AlephZeroContractSelector: Get<[u8; 4]>;
		type Balance: Member
			+ Parameter
			+ AtLeast32BitUnsigned
			// required to translate Tezos Ints of unknown precision (Alternative: use Tezos SDK types in clients of this pallet)
			+ From<u128>
			+ Default
			+ Copy
			+ MaybeSerializeDeserialize
			+ MaxEncodedLen
			+ TypeInfo;

		type WeightInfo: WeightInfo;
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config<I>, I: 'static = ()> {
		AlephZeroContractUpdated { contract: ContractCall<T::AccountId> },
		SentToProxy(OutgoingMessageWithMetaFor<T, I>),
		ReceivedFromProxy(ProcessMessageResult),
	}

	#[pallet::error]
	pub enum Error<T, I = ()> {
		/// Nested HyperdriveIBC error.
		PalletHyperdriveIBC(pallet_acurast_hyperdrive_ibc::Error<T, I>),
		SubstrateMessageDecoderError(u8),
		SubstrateMessageEncoderError(u8),
		InvalidSender,
	}

	impl<T: Config<I>, I: 'static> From<pallet_acurast_hyperdrive_ibc::Error<T, I>> for Error<T, I> {
		fn from(e: pallet_acurast_hyperdrive_ibc::Error<T, I>) -> Self {
			Error::<T, I>::PalletHyperdriveIBC(e)
		}
	}

	impl<T: Config<I>, I: 'static> From<SubstrateMessageDecoderError> for Error<T, I> {
		fn from(e: SubstrateMessageDecoderError) -> Self {
			Error::<T, I>::SubstrateMessageDecoderError(e as u8)
		}
	}

	impl<T: Config<I>, I: 'static> From<SubstrateMessageEncoderError> for Error<T, I> {
		fn from(e: SubstrateMessageEncoderError) -> Self {
			Error::<T, I>::SubstrateMessageEncoderError(e as u8)
		}
	}

	#[pallet::type_value]
	pub fn InitialAlephZeroContract<T: Config<I>, I: 'static>() -> ContractCall<T::AccountId> {
		ContractCall {
			contract: T::AlephZeroContract::get(),
			selector: Some(T::AlephZeroContractSelector::get()),
		}
	}

	#[pallet::storage]
	#[pallet::getter(fn aleph_zero_contract)]
	pub type AlephZeroContract<T: Config<I>, I: 'static = ()> =
		StorageValue<_, ContractCall<T::AccountId>, ValueQuery, InitialAlephZeroContract<T, I>>;

	/// Next outgoing message number. The latest used number is the stored value - 1.
	#[pallet::storage]
	#[pallet::getter(fn next_message_number)]
	pub type NextMessageNumber<T: Config<I>, I: 'static = ()> = StorageValue<_, u64, ValueQuery>;

	#[pallet::call]
	impl<T: Config<I>, I: 'static> Pallet<T, I> {
		/// Updates the AlephZero (target chain) contract address in storage. Can only be called by a privileged/root account.
		#[pallet::call_index(0)]
		#[pallet::weight(< T as Config<I>>::WeightInfo::update_aleph_zero_contract())]
		pub fn update_aleph_zero_contract(
			origin: OriginFor<T>,
			contract: ContractCall<T::AccountId>,
		) -> DispatchResult {
			ensure_root(origin)?;
			<AlephZeroContract<T, I>>::set(contract.clone());
			Self::deposit_event(Event::AlephZeroContractUpdated { contract });
			Ok(())
		}
	}

	impl<T: Config<I>, I: 'static> Pallet<T, I>
	where
		T: pallet_acurast_hyperdrive_ibc::Config<I>,
	{
		/// Sends a message with the given [`Action`] over Hyperdrive.
		///
		/// NOTE: the account triggering this message is the payer but not the sender (sender is a constant pallet account [`T::Sender`]).
		pub fn send_to_proxy(
			chain: ProxyChain,
			action: IncomingAction,
			payer: &T::AccountId,
		) -> Result<(), Error<T, I>> {
			let next_message_number = Self::next_message_number();
			NextMessageNumber::<T, I>::put(next_message_number + 1);

			let message = Message { id: next_message_number, action };
			let encoded = <SubstrateMessageEncoder as MessageEncoder>::encode(&message)?;

			let recipient = match chain {
				ProxyChain::AlephZero =>
					Subject::AlephZero(Layer::Contract(Self::aleph_zero_contract())),
			};
			let message = pallet_acurast_hyperdrive_ibc::Pallet::<T, I>::do_send_message(
				Subject::Acurast(Layer::Extrinsic(T::Sender::get())),
				// payer
				payer,
				T::MessageIdHashing::hash_of(&next_message_number),
				recipient,
				encoded,
				30u8.into(),
				1000u32.into(),
			)?;

			Self::deposit_event(Event::SentToProxy(message));

			Ok(().into())
		}
	}

	impl<T: Config<I>, I: 'static>
		pallet_acurast_hyperdrive_ibc::MessageProcessor<T::AccountId, T::AccountId> for Pallet<T, I>
	where
		T::RegistrationExtra: From<RegistrationExtra<T::Balance, T::AccountId, T::MaxSlots>>,
	{
		fn process(message: MessageBody<T::AccountId, T::AccountId>) -> DispatchResultWithPostInfo {
			match message.sender {
				SubjectFor::<T>::AlephZero(_) => {
					let action =
						<SubstrateMessageDecoder::<I, T::ParsableAccountId, T::AccountId> as types::MessageDecoder<T>>::decode(
							&message.payload,
						)
						.map_err(|e| Error::<T, I>::SubstrateMessageDecoderError(e as u8))?;
					T::ActionExecutor::execute(action)?;

					Ok(().into())
				},
				_ => Err(Error::<T, I>::InvalidSender),
			}?;

			Ok(().into())
		}
	}
}
