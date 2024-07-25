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
        prelude::vec::Vec,
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
			Self {
                fulfillments: Default::default(),
                fulfillments_index: Default::default(),
            }
		}

		#[ink(message)]
		pub fn fulfill(&mut self, job_id: u64, payload: Vec<u8>) -> crate::FulfillReturn {
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
