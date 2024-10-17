use collections::BTreeSet;
use gstd::{exec, msg, BlockNumber};
use sails_rs::prelude::*;

use crate::storage::*;
use crate::types::*;
use crate::utils::*;

#[derive(Default)]
pub struct VaraIbcService();

impl VaraIbcService {
	pub fn init(owner: Option<ActorId>) -> Self {
		unsafe {
			STORAGE = Some(Storage::default());
		}
		Storage::config().owner = owner.unwrap_or(msg::source());
		Storage::config().min_delivery_signatures = 1;
		Storage::config().min_receipt_signatures = 1;
		Storage::config().min_ttl = 20;
		Storage::config().incoming_ttl = 30;
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
		blake2_256(encoded.as_slice())
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
				blake2_256((message, r).encode().as_slice())
			} else {
				blake2_256(message.encode().as_slice())
			};

			let public: Public =
				secp256k1_ecdsa_recover_compressed(&signature, &message_hash)?.to_vec();

			Storage::oracle_public_keys().get(&public).ok_or(IbcError::PublicKeyUnknown)?;

			if seen.contains(&public) {
				Err(IbcError::DuplicateSignature)?
			}
			seen.insert(public);

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

#[sails_rs::service(events = Event)]
impl VaraIbcService {
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
		Storage::incoming_index()
	}

	pub fn oracles(&self, public: Public) -> bool {
		Storage::oracle_public_keys().get(&public).is_some()
	}

	fn do_receive_message(
		&mut self,
		sender: Subject,
		nonce: MessageNonce,
		recipient: Subject,
		payload: Payload,
		signatures: Signatures,
	) -> Result<(), IbcError> {
		let contract = if let Subject::Vara(Layer::Contract(contract_call)) = recipient.clone() {
			Ok(contract_call.contract)
		} else {
			Err(IbcError::IncorrectRecipient)
		}?;

		let id = Self::msg_id(&sender, nonce);

		if Storage::incoming().get(&id).is_some() {
			Err(IbcError::MessageAlreadyReceived)?
		}

		let current_block = exec::block_height();
		let relayer = msg::source();

		let message = Message { id, sender, nonce, recipient, payload };

		Self::check_signatures(
			&message,
			None,
			signatures,
			Storage::config().min_receipt_signatures,
		)?;

		let message_with_meta = IncomingMessageWithMeta { message, current_block, relayer };

		Storage::incoming().insert(id, message_with_meta.clone());

		Storage::incoming_index().push(id);

		match msg::send(contract, message_with_meta.message.payload, 0) {
			Ok(_) => {
				let _ = self.notify_on(Event::MessageProcessed { id });
			},
			Err(_error) => {
				// swallow error to make storing message persistent
				let _ = self.notify_on(Event::MessageProcessedWithErrors { id });
			},
		}

		Ok(())
	}

	pub fn receive_message(
		&mut self,
		sender: Subject,
		nonce: MessageNonce,
		recipient: Subject,
		payload: Payload,
		signatures: Signatures,
	) {
		panicking(Self::ensure_unpaused);

		let _message = panicking(move || {
			self.do_receive_message(sender, nonce, recipient, payload, signatures)
		});
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
				Subject::Vara(Layer::Contract(ContractCall {
					contract: exec::program_id(),
					selector: None,
				})),
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
	
    pub fn send_test_message(
		&mut self,
		recipient: Subject,
		ttl: BlockNumber,
	) {
		panicking(Self::ensure_unpaused);

		let fee = msg::value();

		let message = panicking(|| {
			Self::do_send_message(
				Subject::Vara(Layer::Contract(ContractCall {
					contract: exec::program_id(),
					selector: None,
				})),
				msg::source(),
				blake2_256(self.message_count().encode().as_slice()),
				recipient,
                // the test message payload is just the sender of the message
				msg::source().encode(),
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
