#![cfg_attr(not(feature = "std"), no_std, no_main)]
#![allow(clippy::arithmetic_side_effects)]

use ink::env::call::Selector;

// Method selectors

pub const FULFILL_SELECTOR: Selector = Selector::new(ink::selector_bytes!("fulfill"));

// Method types

pub type FulfillReturn = Result<(), ink::prelude::string::String>;

#[ink::contract]
mod client {
	use ink::{
		prelude::{string::ToString, vec::Vec},
		storage::{Lazy, Mapping},
	};

	#[ink(storage)]
	pub struct Client {
		fulfillments: Mapping<u64, Vec<u8>>,
		fulfillments_index: Lazy<Vec<u64>>,
	}

	impl Client {
		#[ink(constructor)]
		#[allow(clippy::should_implement_trait)]
		pub fn default() -> Self {
			Self { fulfillments: Default::default(), fulfillments_index: Default::default() }
		}

		#[ink(message)]
		pub fn fulfill(&mut self, job_id: u64, payload: Vec<u8>) -> crate::FulfillReturn {
			let calling_contract = self.env().caller();

			// Verify if sender is assigned to the job
			// 5Df6i9Ccy9R3bgBDvoWhYp8bTonxEWRqQMYqHisFpEtFUkpo
			let proxy_contract = AccountId::from([
				70, 119, 129, 217, 127, 53, 112, 17, 126, 182, 26, 12, 209, 195, 170, 46, 213, 215,
				252, 71, 129, 94, 196, 10, 240, 83, 155, 13, 137, 153, 7, 71,
			]);
			if calling_contract != proxy_contract {
				return Err("UnauthorizedCaller".to_string())
			}

			self.fulfillments.insert(job_id, &payload);
			let mut index = self.fulfillments_index.get_or_default();
			index.push(job_id);
			self.fulfillments_index.set(&index);
			Ok(())
		}

		#[ink(message)]
		pub fn fulfillment(&self, job_id: u64) -> Option<Vec<u8>> {
			self.fulfillments.get(job_id)
		}

		#[ink(message)]
		pub fn fulfillments_index(&self) -> Vec<u64> {
			self.fulfillments_index.get_or_default()
		}
	}
}
