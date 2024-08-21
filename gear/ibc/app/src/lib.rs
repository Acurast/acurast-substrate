#![no_std]

mod service;
mod storage;
mod types;
mod utils;

use sails_rs::prelude::*;
use service::*;

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
