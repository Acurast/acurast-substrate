#![no_std]

mod types;
mod utils;

use collections::{BTreeSet, HashMap};
use gstd::{exec, msg, BlockNumber};
use sails_rs::prelude::*;
use types::*;
use utils::*;

#[derive(Default)]
pub struct Hyperdrive;

#[program]
impl Hyperdrive {
	pub fn new() -> Self {
		Self
	}

	pub fn ibc(&self) -> Ibc {
		Ibc::default()
	}
}

static mut STORAGE: Option<Storage> = None;

#[derive(Debug, Default)]
pub struct Storage {
	config: Config,
	outgoing: HashMap<MsgId, OutgoingMessageWithMeta>,
	outgoing_index: Vec<MsgId>,
	incoming: HashMap<MsgId, IncomingMessageWithMeta>,
	incoming_index: Vec<MsgId>,
	message_counter: u128,
	oracle_public_keys: HashMap<Public, ()>,
}

impl Storage {
	pub fn get_mut() -> &'static mut Self {
		unsafe { STORAGE.as_mut().expect("Storage is not initialized") }
	}

	pub fn get() -> &'static Self {
		unsafe { STORAGE.as_ref().expect("Storage is not initialized") }
	}

	pub fn config() -> &'static mut Config {
		let storage = Self::get_mut();
		&mut storage.config
	}

	pub fn outgoing() -> &'static mut HashMap<MsgId, OutgoingMessageWithMeta> {
		let storage = Self::get_mut();
		&mut storage.outgoing
	}

	pub fn outgoing_index() -> &'static mut Vec<MsgId> {
		let storage = Self::get_mut();
		&mut storage.outgoing_index
	}

	pub fn incoming() -> &'static mut HashMap<MsgId, IncomingMessageWithMeta> {
		let storage = Self::get_mut();
		&mut storage.incoming
	}

	pub fn incoming_index() -> &'static mut Vec<MsgId> {
		let storage = Self::get_mut();
		&mut storage.incoming_index
	}

	pub fn message_counter() -> u128 {
		let storage = Self::get();
		storage.message_counter
	}

	pub fn increase_message_counter() -> u128 {
		let storage = Self::get_mut();
		let counter = storage.message_counter;
		storage.message_counter = counter.saturating_add(1);
		storage.message_counter
	}

	pub fn oracle_public_keys() -> &'static mut HashMap<Public, ()> {
		let storage = Self::get_mut();
		&mut storage.oracle_public_keys
	}
}

#[derive(Default)]
pub struct Ibc();

impl Ibc {
	pub fn init() -> Self {
		unsafe {
			STORAGE = Some(Storage::default());
		}
		Self()
	}

	fn ensure_owner() -> Result<(), IbcError> {
		let config = Storage::config();
		if config.owner.eq(&msg::source()) {
			Ok(())
		} else {
			Err(IbcError::NotOwner)
		}
	}

	fn ensure_unpaused() -> Result<(), IbcError> {
		let config = Storage::config();
		if config.paused {
			Err(IbcError::ContractPaused)
		} else {
			Ok(())
		}
	}

	fn update_oracles(updates: Vec<OracleUpdate>) {
		let oracle_public_keys = Storage::oracle_public_keys();
		for update in updates {
			match update {
				OracleUpdate::Add(public) => {
					oracle_public_keys.insert(public, ());
				},
				OracleUpdate::Remove(public) => {
					oracle_public_keys.remove(&public);
				},
			}
		}
	}

	fn msg_id(sender: &Subject, nonce: MessageNonce) -> MsgId {
		let encoded = (sender, nonce).encode();

		todo!()
	}

	fn check_signatures(
		message: &Message,
		relayer: Option<AccountId>,
		signatures: Signatures,
		min_signatures: u8,
	) -> Result<(), IbcError> {
		if signatures.len() < min_signatures.into() {
			Err(IbcError::NotEnoughSignaturesValid)?
		}

		let mut seen: BTreeSet<Vec<u8>> = Default::default();
		signatures.into_iter().try_for_each(|signature| -> Result<(), IbcError> {
			let message_hash: [u8; 32] = if let Some(r) = &relayer {
				// self.env().hash_bytes::<Blake2x256>(&(message, r).encode())
				// TODO: find a way to hash
				[0; 32]
			} else {
				// self.env().hash_bytes::<Blake2x256>(&message.encode())
				[0; 32]
			};

			// TODO: validate signature
			//let public: Public = self
			//	.env()
			//	.ecdsa_recover(&signature, &message_hash)
			//	.map_err(|_| Error::SignatureInvalid)?
			//	.to_vec();

			// self.oracle_public_keys.get(&public).ok_or(Error::PublicKeyUnknown)?;

			// if seen.contains(&public) {
			//	Err(Error::DuplicateSignature)?
			// }
			// seen.insert(public);

			Ok(())
		})?;

		Ok(())
	}

	fn do_send_message(
		sender: Subject,
		payer: AccountId,
		nonce: MessageNonce,
		recipient: Subject,
		payload: Payload,
		ttl: BlockNumber,
		fee: Balance,
	) -> Result<OutgoingMessageWithMeta, IbcError> {
		Self::ensure_unpaused()?;

		let current_block = exec::block_height();

		let id = Self::msg_id(&sender, nonce);

		// look for duplicates
		if let Some(message) = Storage::outgoing().get(&id) {
			// potential duplicate found: check for ttl
			if message.ttl_block >= current_block {
				Err(IbcError::MessageWithSameNoncePending)?;
			}

			// continue below and overwrite message
		}

		// validate params
		if ttl < Storage::config().min_ttl {
			Err(IbcError::TTLSmallerThanMinimum)?
		}

		let message = Message { id, sender, nonce, recipient, payload };
		let message_with_meta = OutgoingMessageWithMeta {
			message,
			current_block,
			ttl_block: current_block.saturating_add(ttl),
			fee,
			payer,
		};

		Storage::outgoing().insert(id, message_with_meta.clone());

		Storage::outgoing_index().push(id);

		let _ = Storage::increase_message_counter();

		Ok(message_with_meta)
	}

	pub fn do_confirm_message_delivery(
		// We only pass the id and retrieve message from runtime storage to ensure the signatures are over the message originally sent (+ the relayer's address).
		id: MsgId,
		// Signatures confirming the message delivery within `ttl_block`.
		//
		// Can be left empty if the message was not delivered in time. In this case the fee is paid back to sender (not necessarily the origin of this call).
		signatures: Signatures,
	) -> Result<(), IbcError> {
		let message = Storage::outgoing().get(&id).ok_or(IbcError::MessageNotFound)?;

		let current_block = exec::block_height();

		if message.ttl_block < current_block {
			Err(IbcError::DeliveryConfirmationOverdue)?
		};

		let relayer = msg::source();
		Self::check_signatures(
			&message.message,
			Some(relayer),
			signatures,
			Storage::config().min_delivery_signatures,
		)?;

		panicking(|| msg::send(message.payer, (), message.fee));

		Storage::outgoing().remove(&id);

		let index = Storage::outgoing_index();
		index.retain(|&i| i != id);

		Ok(())
	}
}

#[service(events = Event)]
impl Ibc {
	pub fn new() -> Self {
		Self()
	}

	pub fn configure(&mut self, actions: Vec<ConfigureArgument>) {
		panicking(Self::ensure_owner);

		let config = Storage::config();

		for action in actions {
			match action {
				ConfigureArgument::Owner(address) => config.owner = address,
				ConfigureArgument::Paused(paused) => config.paused = paused,
				ConfigureArgument::OraclePublicKeys(oracle_updates) => {
					Self::update_oracles(oracle_updates);
					let _ = self.notify_on(Event::OraclesUpdated);
				},
				ConfigureArgument::MinDeliverySignatures(min_delivery_signatures) => {
					config.min_delivery_signatures = min_delivery_signatures
				},
				ConfigureArgument::MinReceiptSignatures(min_receipt_signatures) => {
					config.min_receipt_signatures = min_receipt_signatures
				},
				ConfigureArgument::MinTTL(min_ttl) => config.min_ttl = min_ttl,
				ConfigureArgument::IncomingTTL(incoming_ttl) => config.incoming_ttl = incoming_ttl,
			}
		}
	}

	pub fn config(&self) -> &'static Config {
		Storage::config()
	}

	pub fn message_count(&self) -> u128 {
		Storage::message_counter()
	}

	pub fn outgoing_message(&self, message_id: MsgId) -> Option<&'static OutgoingMessageWithMeta> {
		Storage::outgoing().get(&message_id)
	}

	pub fn outgoing_index(&self) -> &'static Vec<MsgId> {
		Storage::outgoing_index()
	}

	pub fn incoming_message(&self, message_id: MsgId) -> Option<&'static IncomingMessageWithMeta> {
		Storage::incoming().get(&message_id)
	}

	pub fn incoming_index(&self) -> &'static Vec<MsgId> {
		Storage::outgoing_index()
	}

	pub fn oracles(&self, public: Public) -> bool {
		Storage::oracle_public_keys().get(&public).is_some()
	}

	pub fn send_message(
		&mut self,
		nonce: MessageNonce,
		recipient: Subject,
		payload: Payload,
		ttl: BlockNumber,
	) {
		panicking(Self::ensure_unpaused);

		let fee = msg::value();

		let message = panicking(move || {
			Self::do_send_message(
				Subject::Vara(exec::program_id()),
				msg::source(),
				nonce,
				recipient,
				payload,
				ttl,
				fee,
			)
		});

		let _ = self.notify_on(Event::MessageReadyToSend { message });
	}

	pub fn confirm_message_delivery(
		&mut self,
		// We only pass the id and retrieve message from runtime storage to ensure the signatures are over the message originally sent (+ the relayer's address).
		id: MsgId,
		// Signatures confirming the message delivery within `ttl_block`.
		//
		// Can be left empty if the message was not delivered in time. In this case the fee is paid back to sender (not necessarily the origin of this call).
		signatures: Signatures,
	) {
		panicking(Self::ensure_unpaused);

		panicking(move || Self::do_confirm_message_delivery(id, signatures));

		let _ = self.notify_on(Event::MessageDelivered { id });
	}
}
