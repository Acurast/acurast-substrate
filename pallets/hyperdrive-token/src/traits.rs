use frame_support::weights::Weight;

/// Weight functions needed for pallet_acurast_hyperdrive.
pub trait WeightInfo {
	fn transfer_native() -> Weight;
	fn retry_transfer_native() -> Weight;
	fn update_ethereum_contract() -> Weight;
	fn update_solana_contract() -> Weight;
}
