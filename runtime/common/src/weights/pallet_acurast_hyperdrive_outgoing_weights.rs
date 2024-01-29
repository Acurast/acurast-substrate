use frame_support::weights::{constants::RocksDbWeight as DbWeight, Weight};

use pallet_acurast_hyperdrive_outgoing::WeightInfo;

pub struct TezosHyperdriveOutgoingWeight;

impl WeightInfo for TezosHyperdriveOutgoingWeight {
	fn send_message() -> Weight {
		// TODO benchmark this
		DbWeight::get().reads_writes(3, 3)
	}
}
