#![cfg_attr(not(feature = "std"), no_std)]

extern crate core;

pub use pallet::*;
pub use traits::*;
pub use types::*;

#[cfg(test)]
mod tests;

// #[cfg(feature = "runtime-benchmarks")]
// mod benchmarking;
mod traits;

mod types;
pub mod weights;

#[frame_support::pallet]
pub mod pallet {
	use frame_support::{
		dispatch::PostDispatchInfo,
		pallet_prelude::*,
		traits::{
			fungible::{
				hold::Mutate as HoldMutateFungible, Inspect as InspectFungible,
				Mutate as MutateFungible,
			},
			tokens::{Fortitude, Precision, Restriction},
			EnsureOrigin, Get,
		},
		transactional,
	};
	use frame_system::pallet_prelude::*;
	use pallet_acurast::MultiOrigin;
	use sp_arithmetic::traits::{Saturating, Zero};
	use sp_runtime::traits::{Hash, Verify};
	use sp_std::{prelude::*, vec};

	use super::*;

	/// A instantiable pallet for receiving secure state synchronizations into Acurast.
	#[pallet::pallet]
	pub struct Pallet<T, I = ()>(PhantomData<(T, I)>);

	/// Configures the pallet.
	#[pallet::config]
	pub trait Config<I: 'static = ()>: frame_system::Config {
		type RuntimeEvent: From<Event<Self, I>>
			+ IsType<<Self as frame_system::Config>::RuntimeEvent>;

		#[pallet::constant]
		type MinTTL: Get<BlockNumberFor<Self>>;
		#[pallet::constant]
		type IncomingTTL: Get<BlockNumberFor<Self>>;
		#[pallet::constant]
		type MinDeliveryConfirmationSignatures: Get<u32>;
		#[pallet::constant]
		type MinReceiptConfirmationSignatures: Get<u32>;

		/// The currency mechanism, used for paying for fees sending messages.
		type Currency: InspectFungible<Self::AccountId>
			+ MutateFungible<Self::AccountId>
			+ HoldMutateFungible<Self::AccountId, Reason = Self::RuntimeHoldReason>;

		/// Overarching hold reason.
		type RuntimeHoldReason: From<HoldReason<I>>;

		/// The hashing system (algorithm) being used in the runtime (e.g. Blake2).
		type MessageIdHashing: Hash<Output = MessageId> + TypeInfo;

		type MessageProcessor: MessageProcessor<Self::AccountId, Self::AccountId>;

		type UpdateOrigin: EnsureOrigin<Self::RuntimeOrigin>;

		type WeightInfo: WeightInfo;
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub (super) fn deposit_event)]
	pub enum Event<T: Config<I>, I: 'static = ()> {
		OracleUpdated {
			added: Vec<(Public, ActivityWindow<BlockNumberFor<T>>)>,
			updated: Vec<(Public, ActivityWindow<BlockNumberFor<T>>)>,
			removed: Vec<Public>,
		},
		MessageReadyToSend {
			message: OutgoingMessageWithMetaFor<T, I>,
		},
		MessageDelivered {
			id: MessageId,
		},
		MessageRemoved {
			id: MessageId,
		},
		MessageProcessed {
			message: IncomingMessageWithMetaFor<T>,
		},
		MessageProcessedWithErrors {
			message: IncomingMessageWithMetaFor<T>,
		},
	}

	/// This storage field maps the oracles' public keys to their respective activity window.
	#[pallet::storage]
	#[pallet::getter(fn relayer_oracle_public_keys)]
	pub type OraclePublicKeys<T: Config<I>, I: 'static = ()> =
		StorageMap<_, Blake2_128Concat, Public, ActivityWindow<BlockNumberFor<T>>>;

	/// This storage field contains the latest number of total messages sent.
	/// Useful to generate a unique nonce for a message as `hash(message_counter() + 1)`.
	#[pallet::storage]
	#[pallet::getter(fn message_counter)]
	pub type MessageCounter<T: Config<I>, I: 'static = ()> =
		StorageValue<_, MessageIndex, ValueQuery>;

	/// Message lookup to map [`MessageId`] -> ([`SubjectFor<T, I>`], [`MessageNonce`]]).
	#[pallet::storage]
	#[pallet::getter(fn outgoing_messages)]
	pub type OutgoingMessages<T: Config<I>, I: 'static = ()> =
		StorageMap<_, Identity, MessageId, OutgoingMessageWithMetaFor<T, I>>;

	/// Messages stored as a map [`SubjectFor<T, I>`] -> [`MessageNonce`] -> [`MessageId`].
	#[pallet::storage]
	#[pallet::getter(fn outgoing_messages_lookup)]
	pub type OutgoingMessagesLookup<T: Config<I>, I: 'static = ()> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		SubjectFor<T>,
		Blake2_128Concat,
		MessageNonce,
		MessageId,
	>;

	/// Received messages stored as a map [`MessageId`] -> [`MessageFor<T, I>`].
	#[pallet::storage]
	#[pallet::getter(fn incoming_messages)]
	pub type IncomingMessages<T: Config<I>, I: 'static = ()> =
		StorageMap<_, Identity, MessageId, IncomingMessageWithMetaFor<T>>;

	/// Messages stored as a map [`SubjectFor<T, I>`] -> [`MessageNonce`] -> [`MessageId`].
	#[pallet::storage]
	#[pallet::getter(fn incoming_messages_lookup)]
	pub type IncomingMessagesLookup<T: Config<I>, I: 'static = ()> =
		StorageDoubleMap<_, Blake2_128Concat, SubjectFor<T>, Blake2_128Concat, MessageId, ()>;

	#[pallet::error]
	pub enum Error<T, I = ()> {
		TTLSmallerThanMinimum,
		MessageWithSameNoncePending,
		CouldNotHoldFee,
		CouldNotReleaseHoldFee,
		MessageNotFound,
		CannotRemovePendingMessage,
		DeliveryConfirmationOverdue,
		NotEnoughSignaturesProvided,
		NotEnoughSignaturesValid,
		SignatureInvalid,
		MessageAlreadyReceived,
		IncorrectRecipient,
		PayloadLengthExceeded,
	}

	/// A reason for the pallet placing a hold on funds.
	#[pallet::composite_enum]
	pub enum HoldReason<I: 'static = ()> {
		#[codec(index = 0)]
		OutgoingMessageFee,
	}

	#[pallet::call]
	impl<T: Config<I>, I: 'static> Pallet<T, I> {
		/// Used to add, update or remove oracles.
		#[pallet::call_index(0)]
		#[pallet::weight(< T as Config < I >>::WeightInfo::update_oracles(updates.len() as u32))]
		pub fn update_oracles(
			origin: OriginFor<T>,
			updates: OracleUpdates<T>,
		) -> DispatchResultWithPostInfo {
			T::UpdateOrigin::ensure_origin(origin)?;

			// Process actions
			let (added, updated, removed) =
				updates.iter().fold((vec![], vec![], vec![]), |acc, action| {
					let (mut added, mut updated, mut removed) = acc;
					match action {
						OracleUpdate::Add(public, activity_window) => {
							<OraclePublicKeys<T, I>>::set(*public, Some(activity_window.clone()));
							added.push((*public, activity_window.clone()))
						},
						OracleUpdate::Update(account, activity_window) => {
							<OraclePublicKeys<T, I>>::set(*account, Some(activity_window.clone()));
							updated.push((*account, activity_window.clone()))
						},
						OracleUpdate::Remove(account) => {
							<OraclePublicKeys<T, I>>::remove(account);
							removed.push(*account)
						},
					}
					(added, updated, removed)
				});

			// Emit event to inform that the state transmitters were updated
			Self::deposit_event(Event::OracleUpdated { added, updated, removed });

			Ok(PostDispatchInfo { actual_weight: None, pays_fee: Pays::No })
		}

		/// Sends a (test) message with the origin of the extrinsic call as the payload.
		#[pallet::call_index(1)]
		#[pallet::weight(< T as Config < I >>::WeightInfo::send_test_message())]
		pub fn send_test_message(
			origin: OriginFor<T>,
			// message params
			recipient: SubjectFor<T>,
			ttl: BlockNumberFor<T>,
			fee: BalanceOf<T, I>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let count = Self::message_counter();
			<MessageCounter<T, I>>::set(count + 1);

			let _message = Self::do_send_message(
				Subject::Acurast(Layer::Extrinsic(who.clone())),
				&who,
				T::MessageIdHashing::hash_of(&count),
				recipient,
				// the test message payload is just the sender of the message
				who.encode(),
				ttl,
				fee,
			)?;

			Ok(())
		}

		/// Used by a relayer to confirm that a message has been delivered, claiming the message fee.
		#[pallet::call_index(2)]
		#[pallet::weight(< T as Config < I >>::WeightInfo::confirm_message_delivery())]
		pub fn confirm_message_delivery(
			origin: OriginFor<T>,
			// We only pass the id and retrieve message from runtime storage to ensure the signatures are over the message originally sent (+ the relayer's address).
			id: MessageId,
			// Signatures confirming the message delivery within `ttl_block`.
			//
			// Can be left empty if the message was not delivered in time. In this case the fee is paid back to sender (not necessarily the origin of this call).
			signatures: Signatures,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let message = Self::outgoing_messages(id).ok_or(Error::<T, I>::MessageNotFound)?;

			let current_block = <frame_system::Pallet<T>>::block_number();

			ensure!(message.ttl_block >= current_block, Error::<T, I>::DeliveryConfirmationOverdue);

			Self::check_signatures(
				&message.message,
				// include the relayer's address in the payload validated for signature to avoid front-running on actual relayer that paid for tx on destination chain
				Some(who.clone()),
				signatures,
				T::MinDeliveryConfirmationSignatures::get(),
			)?;

			T::Currency::transfer_on_hold(
				&HoldReason::OutgoingMessageFee.into(),
				&message.payer,
				&who,
				message.fee,
				Precision::BestEffort,
				Restriction::Free,
				Fortitude::Polite,
			)
			.map_err(|_| Error::<T, I>::CouldNotHoldFee)?;

			Self::deposit_event(Event::MessageDelivered { id });

			// clear
			<OutgoingMessages<T, I>>::remove(id);
			<OutgoingMessagesLookup<T, I>>::remove(&message.message.sender, message.message.nonce);

			Ok(())
		}

		#[pallet::call_index(3)]
		#[pallet::weight(< T as Config < I >>::WeightInfo::remove_message())]
		pub fn remove_message(origin: OriginFor<T>, id: MessageId) -> DispatchResult {
			let _who = ensure_signed(origin)?;

			let message = Self::outgoing_messages(id).ok_or(Error::<T, I>::MessageNotFound)?;

			let current_block = <frame_system::Pallet<T>>::block_number();

			ensure!(message.ttl_block < current_block, Error::<T, I>::CannotRemovePendingMessage);

			T::Currency::release(
				&HoldReason::OutgoingMessageFee.into(),
				&message.payer,
				message.fee,
				Precision::BestEffort,
			)
			.map_err(|_| Error::<T, I>::CouldNotHoldFee)?;

			Self::deposit_event(Event::MessageRemoved { id });

			// clear
			<OutgoingMessages<T, I>>::remove(id);
			<OutgoingMessagesLookup<T, I>>::remove(&message.message.sender, message.message.nonce);

			Ok(())
		}

		/// Receives messages signed by the oracles.
		#[pallet::call_index(4)]
		#[pallet::weight(< T as Config < I >>::WeightInfo::receive_message())]
		pub fn receive_message(
			origin: OriginFor<T>,
			sender: SubjectFor<T>,
			nonce: MessageNonce,
			recipient: SubjectFor<T>,
			payload: Payload,
			relayer: MultiOrigin<T::AccountId>,
			signatures: Signatures,
		) -> DispatchResult {
			let _who = ensure_signed(origin)?;

			ensure!(
				matches!(recipient, SubjectFor::<T>::Acurast(_)),
				Error::<T, I>::IncorrectRecipient
			);

			let id = Self::message_id(&sender, nonce);
			ensure!(
				<IncomingMessages<T, I>>::get(id).is_none(),
				Error::<T, I>::MessageAlreadyReceived
			);

			let current_block = <frame_system::Pallet<T>>::block_number();

			let message =
				MessageFor::<T> { id, sender, nonce, recipient: recipient.clone(), payload };

			Self::check_signatures(
				&message.clone(),
				None,
				signatures,
				T::MinReceiptConfirmationSignatures::get(),
			)?;

			let message_with_meta = IncomingMessageWithMetaFor::<T> {
				message: message.clone(),
				current_block,
				relayer,
			};
			<IncomingMessages<T, I>>::insert(id, message_with_meta.clone());
			<IncomingMessagesLookup<T, I>>::insert(&recipient, id, ());

			// don't fail extrinsic from here onwards
			if let Err(e) = Self::process_message(message.clone()) {
				log::warn!("Received message {:?} processed with errors: {:?}", message, e.error);
				Self::deposit_event(Event::MessageProcessedWithErrors {
					message: message_with_meta,
				});
			} else {
				Self::deposit_event(Event::MessageProcessed { message: message_with_meta });
			}

			Ok(())
		}

		/// Cleans up incoming messages for which [`<T as Config<I>>::IncomingTTL`] passed.
		#[pallet::call_index(5)]
		#[pallet::weight(< T as Config < I >>::WeightInfo::clean_incoming())]
		pub fn clean_incoming(
			origin: OriginFor<T>,
			ids: MessagesCleanup,
		) -> DispatchResultWithPostInfo {
			let _who = ensure_signed(origin)?;

			let current_block = <frame_system::Pallet<T>>::block_number();

			let l = ids.len();
			let mut i = 0usize;
			for id in ids.iter() {
				let Some(message) = <IncomingMessages<T, I>>::get(id) else {
					continue;
				};
				if message.current_block.saturating_add(T::IncomingTTL::get()) < current_block {
					<IncomingMessages<T, I>>::remove(id);
					<IncomingMessagesLookup<T, I>>::remove(
						&message.message.recipient,
						message.message.nonce,
					);
					i += 1;
				}
			}

			if i == l {
				Ok(Pays::No.into())
			} else {
				Ok(().into())
			}
		}
	}

	impl<T: Config<I>, I: 'static> Pallet<T, I> {
		pub fn check_signatures(
			message: &MessageFor<T>,
			relayer: Option<T::AccountId>,
			signatures: Signatures,
			min_signatures: u32,
		) -> Result<(), Error<T, I>> {
			ensure!(
				signatures.len() >= min_signatures as usize,
				Error::<T, I>::NotEnoughSignaturesProvided
			);

			let current_block = <frame_system::Pallet<T>>::block_number();

			// we always deny invalid signatures but skip signatures for unknown public keys or known public keys outside activity window
			// this allows for some flexibility while deprecating relayer oracles
			let mut not_found: Vec<(
				[u8; SIGNATURE_SERIALIZED_SIZE],
				[u8; PUBLIC_KEY_SERIALIZED_SIZE],
			)> = Default::default();
			let mut outside_activity_window: Vec<(
				[u8; SIGNATURE_SERIALIZED_SIZE],
				[u8; PUBLIC_KEY_SERIALIZED_SIZE],
			)> = Default::default();
			let mut valid = 0;
			let mut checked: Vec<Public> = vec![];
			signatures.into_iter().try_for_each(
				|(signature, public)| -> Result<(), Error<T, I>> {
					match <OraclePublicKeys<T, I>>::get(public) {
						None => {
							not_found.push((signature.0, public.0));
						},
						Some(activity_window) if !checked.contains(&public) => {
							// valid window is defined inclusive start_block, exclusive end_block

							if activity_window.start_block <= current_block
								&& activity_window
									.end_block
									.map_or(true, |end_block| current_block < end_block)
							{
								if let Some(r) = &relayer {
									ensure!(
										signature.verify(&(message, r).encode()[..], &public),
										Error::<T, I>::SignatureInvalid
									);
								} else {
									ensure!(
										signature.verify(&message.encode()[..], &public),
										Error::<T, I>::SignatureInvalid
									);
								};
								valid += 1;
								checked.push(public);
							} else {
								outside_activity_window.push((signature.0, public.0));
							}
						},
						_ => {},
					}

					Ok(())
				},
			)?;

			if !not_found.len().is_zero() {
				log::warn!(
					"Some provided signatures from unknown public keys provided: {:?}",
					not_found
				);
			}
			if !outside_activity_window.len().is_zero() {
				log::warn!("Some provided signatures were invalid since the oracles' public key are outside their activity window: {:?}", outside_activity_window);
			}

			if valid < min_signatures {
				log::info!("check_signatures failed since not enough valid signatures remain. Unknown oracles: {:?}, inactive oracles: {:?}.", outside_activity_window, outside_activity_window);
				Err(Error::<T, I>::NotEnoughSignaturesValid)?
			}

			Ok(())
		}

		fn message_id(sender: &SubjectFor<T>, nonce: MessageNonce) -> MessageId {
			T::MessageIdHashing::hash_of(&(sender, nonce))
		}

		// fn outgoing_message_by_sender(
		// 	sender: &SenderFor<T>,
		// 	nonce: &MessageNonce,
		// ) -> Option<OutgoingMessageWithMetaFor<T, I>> {
		// 	let id = <OutgoingMessagesLookup<T, I>>::get(sender, nonce)?;
		// 	<OutgoingMessages<T, I>>::get(&id)
		// }

		// fn incoming_message_by_recipient(
		// 	recipient: &SubjectFor<T>,
		// 	nonce: &MessageNonce,
		// ) -> Option<IncomingMessageWithMetaFor<T>> {
		// 	let id = <IncomingMessagesLookup<T, I>>::get(recipient, nonce)?;
		// 	<IncomingMessages<T, I>>::get(&id)
		// }

		/// Sends a message by the given `sender` paid by a potentially different `payer`.
		///
		/// **NOTE**:
		///
		/// * This is an internal function but could be made available
		/// to other pallets if the authorization for the passed `sender` has been ensured at the caller.
		/// Be careful to not allow for unintended impersonation.
		/// * _Exactly-once delivery_ is **not** guaranteed even the `nonce` serves as deduplication during ttl; While, after ttl passed and message fee cannot be claimed by relayer, a _different_ message with same nonce can be sent off, it cannot be guaranteed a relayer received the oracle signatures before and still submits first message to proxy.
		pub fn do_send_message(
			sender: SubjectFor<T>,
			payer: &T::AccountId,
			nonce: MessageNonce,
			recipient: SubjectFor<T>,
			payload: Vec<u8>,
			ttl: BlockNumberFor<T>,
			fee: BalanceOf<T, I>,
		) -> Result<OutgoingMessageWithMetaFor<T, I>, Error<T, I>> {
			let current_block = <frame_system::Pallet<T>>::block_number();

			// look for duplicates
			let id = Self::message_id(&sender, nonce);
			if let Some(message) = Self::outgoing_messages(id) {
				// potential duplicate found: check for ttl
				ensure!(
					message.ttl_block < current_block,
					Error::<T, I>::MessageWithSameNoncePending
				)

				// continue below and overwrite message
			}

			// validate params
			ensure!(ttl >= T::MinTTL::get(), Error::<T, I>::TTLSmallerThanMinimum);

			let message = MessageFor::<T> {
				id,
				sender: sender.clone(),
				nonce,
				recipient,
				payload: Payload::try_from(payload)
					.map_err(|_| Error::<T, I>::PayloadLengthExceeded)?,
			};
			let message_with_meta = OutgoingMessageWithMetaFor::<T, I> {
				message,
				current_block,
				ttl_block: current_block.saturating_add(ttl),
				fee,
				payer: payer.clone(),
			};
			<OutgoingMessages<T, I>>::insert(id, &message_with_meta);
			<OutgoingMessagesLookup<T, I>>::insert(&sender, nonce, id);

			T::Currency::hold(&HoldReason::OutgoingMessageFee.into(), payer, fee)
				.map_err(|_| Error::<T, I>::CouldNotHoldFee)?;

			log::info!("Hyperdrive-IBC message is ready to send: {:?}", &message_with_meta);
			Self::deposit_event(Event::MessageReadyToSend { message: message_with_meta.clone() });

			Ok(message_with_meta)
		}

		#[transactional]
		fn process_message(message: MessageFor<T>) -> DispatchResultWithPostInfo {
			T::MessageProcessor::process(message.into())
		}
	}

	impl<T: Config<I>, I: 'static> MessageSender<T, BalanceOf<T, I>> for Pallet<T, I> {
		fn send_message(
			sender: SubjectFor<T>,
			payer: &T::AccountId,
			nonce: MessageNonce,
			recipient: SubjectFor<T>,
			payload: Vec<u8>,
			ttl: BlockNumberFor<T>,
			fee: BalanceOf<T, I>,
		) -> DispatchResult {
			let _ = Self::do_send_message(sender, payer, nonce, recipient, payload, ttl, fee)?;
			Ok(())
		}
	}
}
