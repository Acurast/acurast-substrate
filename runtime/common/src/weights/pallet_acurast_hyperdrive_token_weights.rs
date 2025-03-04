use frame_support::weights::{constants::RocksDbWeight as DbWeight, Weight};

use pallet_acurast_hyperdrive_token::WeightInfo;

pub struct HyperdriveTokenWeight;

impl WeightInfo for HyperdriveTokenWeight {
	fn transfer_native() -> Weight {
		// TODO benchmark this
		DbWeight::get().reads_writes(3, 3)
	}
	fn retry_transfer_native() -> Weight {
		// TODO benchmark this
		DbWeight::get().reads_writes(3, 3)
	}
	fn update_ethereum_contract() -> Weight {
		// TODO benchmark this
		DbWeight::get().reads_writes(3, 3)
	}
	fn update_solana_contract() -> Weight {
		// TODO benchmark this
		DbWeight::get().reads_writes(3, 3)
	}
}
