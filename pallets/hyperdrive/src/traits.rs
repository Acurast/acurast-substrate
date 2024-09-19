use frame_support::weights::Weight;

/// Weight functions needed for pallet_acurast_hyperdrive.
pub trait WeightInfo {
	fn update_aleph_zero_contract() -> Weight;
	fn update_vara_contract() -> Weight;
}
