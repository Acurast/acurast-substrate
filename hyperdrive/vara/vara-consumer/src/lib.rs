#![no_std]

mod service;
mod storage;
mod types;
mod utils;

#[cfg(feature = "wasm-binary")]
#[cfg(not(target_arch = "wasm32"))]
pub use code::WASM_BINARY_OPT as WASM_BINARY;

use sails_rs::prelude::*;
use service::VaraConsumerService;

pub struct VaraConsumerProgram;

#[sails_rs::program]
impl VaraConsumerProgram {
	// Program's constructor
	pub fn new(owner: Option<ActorId>, proxy: ActorId) -> Self {
		VaraConsumerService::init(owner, proxy);
		Self
	}

	// Exposed service
	pub fn vara_consumer(&self) -> service::VaraConsumerService {
		service::VaraConsumerService::new()
	}
}

#[cfg(feature = "wasm-binary")]
#[cfg(not(target_arch = "wasm32"))]
mod code {
	include!(concat!(env!("OUT_DIR"), "/wasm_binary.rs"));
}
