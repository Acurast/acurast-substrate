use collections::HashMap;
use sails_rs::prelude::*;

use crate::types::*;

pub static mut STORAGE: Option<Storage> = None;

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
