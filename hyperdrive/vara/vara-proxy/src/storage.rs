use collections::HashMap;
use sails_rs::prelude::*;

use crate::types::*;
use crate::utils::ProxyError;

pub static mut STORAGE: Option<Storage> = None;

#[derive(Debug, Default)]
pub struct Storage {
	config: Config,
	next_outgoing_action_id: u64,
	next_job_id: u128,
	job_info: HashMap<u128, (u16, Vec<u8>)>,
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

	pub fn next_outgoing_action_id() -> u64 {
		Self::get().next_outgoing_action_id
	}

	pub fn get_and_increase_next_outgoing_action_id() -> u64 {
		let result = Self::next_outgoing_action_id();
		Self::get_mut().next_outgoing_action_id += 1;
		result
	}

	pub fn next_job_id() -> u128 {
		Self::get().next_job_id
	}

	pub fn get_and_increase_next_job_id() -> u128 {
		let result = Self::next_job_id();
		Self::get_mut().next_job_id += 1;
		result
	}

	pub fn job_info() -> &'static mut HashMap<u128, (u16, Vec<u8>)> {
		let storage = Self::get_mut();
		&mut storage.job_info
	}

	pub fn get_job(job_id: u128) -> Result<(Version, Vec<u8>), ProxyError> {
		if let Some((version, job_bytes)) = Self::job_info().get(&job_id) {
			match version {
				o if *o == Version::V1 as u16 => Ok((Version::V1, job_bytes.clone())),
				v => Err(ProxyError::UnknownJobVersion(*v)),
			}
		} else {
			Err(ProxyError::UnknownJob)
		}
	}
}
