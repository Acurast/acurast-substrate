#![cfg_attr(not(feature = "std"), no_std, no_main)]
#![allow(clippy::arithmetic_side_effects)]

use ink::env::call::Selector;

// Method selectors

pub const FULFILL_SELECTOR: Selector = Selector::new(ink::selector_bytes!("fulfill"));

// Method types

pub type FulfillReturn = Result<(), ink::prelude::string::String>;

#[ink::contract]
mod client {
	use ink::prelude::vec::Vec;

	#[ink(storage)]
	pub struct Client {
		// Template
	}

	impl Client {
		#[ink(constructor)]
        #[allow(clippy::should_implement_trait)]
		pub fn default() -> Self {
			Self {}
		}

		#[ink(message)]
		pub fn fulfill(&mut self, _job_id: u64, _payload: Vec<u8>) -> crate::FulfillReturn {
			Ok(())
		}
	}
}
