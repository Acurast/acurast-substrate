use frame_support::weights::Weight;

/// Weight functions needed for pallet_acurast_hyperdrive_ibc.
pub trait WeightInfo {
	fn update_oracles(l: u32) -> Weight;
	fn send_message() -> Weight;
	fn confirm_message_delivery() -> Weight;
	fn remove_message() -> Weight;
	fn receive_message() -> Weight;
}
