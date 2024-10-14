#![no_std]

mod service;
mod storage;
mod types;
mod utils;

#[cfg(feature = "wasm-binary")]
#[cfg(not(target_arch = "wasm32"))]
pub use code::WASM_BINARY_OPT as WASM_BINARY;

use sails_rs::prelude::*;
use service::VaraIbcService;

pub struct VaraIbcProgram;

#[sails_rs::program]
impl VaraIbcProgram {
	// Program's constructor
	pub fn new(owner: Option<ActorId>) -> Self {
		VaraIbcService::init(owner);
		Self
	}

	// Exposed service
	pub fn vara_ibc(&self) -> service::VaraIbcService {
		service::VaraIbcService::new()
	}
}

#[cfg(feature = "wasm-binary")]
#[cfg(not(target_arch = "wasm32"))]
mod code {
	include!(concat!(env!("OUT_DIR"), "/wasm_binary.rs"));
}
