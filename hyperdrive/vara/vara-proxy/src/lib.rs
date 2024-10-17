#![no_std]

#[cfg(feature = "wasm-binary")]
#[cfg(not(target_arch = "wasm32"))]
pub use code::WASM_BINARY_OPT as WASM_BINARY;

mod service;
mod storage;
mod types;
mod utils;

use sails_rs::prelude::*;
use service::VaraProxyService;

pub struct VaraProxyProgram;

#[sails_rs::program]
impl VaraProxyProgram {
	// Program's constructor
	pub fn new(owner: Option<ActorId>, ibc: Option<ActorId>) -> Self {
		VaraProxyService::init(owner, ibc);
		Self
	}

	// Exposed service
	pub fn vara_proxy(&self) -> service::VaraProxyService {
		service::VaraProxyService::new()
	}
}

#[cfg(feature = "wasm-binary")]
#[cfg(not(target_arch = "wasm32"))]
mod code {
	include!(concat!(env!("OUT_DIR"), "/wasm_binary.rs"));
}
