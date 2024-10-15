use collections::HashMap;
use sails_rs::prelude::*;

use crate::types::*;

pub static mut STORAGE: Option<Storage> = None;

#[derive(Debug, Default)]
pub struct Storage {
	config: Config,
	fulfillments: HashMap<u64, Vec<u8>>,
	fulfillments_index: Vec<u64>,
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

	pub fn fulfillments() -> &'static mut HashMap<u64, Vec<u8>> {
		let storage = Self::get_mut();
		&mut storage.fulfillments
	}

	pub fn fulfillments_index() -> &'static mut Vec<u64> {
		let storage = Self::get_mut();
		&mut storage.fulfillments_index
	}
}
