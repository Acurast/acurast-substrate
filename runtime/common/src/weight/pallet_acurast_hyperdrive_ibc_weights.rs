use frame_support::weights::{constants::RocksDbWeight as DbWeight, Weight};

use pallet_acurast_hyperdrive_ibc::WeightInfo;

pub struct HyperdriveWeight;

impl WeightInfo for HyperdriveWeight {
	fn update_oracles(_l: u32) -> Weight {
		// TODO benchmark this
		DbWeight::get().reads_writes(3, 3)
	}
	fn send_test_message() -> Weight {
		// TODO benchmark this
		DbWeight::get().reads_writes(3, 3)
	}
	fn confirm_message_delivery() -> Weight {
		// TODO benchmark this
		DbWeight::get().reads_writes(3, 3)
	}
	fn remove_message() -> Weight {
		// TODO benchmark this
		DbWeight::get().reads_writes(3, 3)
	}
	fn receive_message() -> Weight {
		// TODO benchmark this
		DbWeight::get().reads_writes(3, 3)
	}
	fn clean_incoming() -> Weight {
		// TODO benchmark this
		DbWeight::get().reads_writes(3, 3)
	}
}
