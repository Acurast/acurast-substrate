use frame_support::weights::Weight;

/// Weight functions needed for pallet_acurast.
pub trait WeightInfo {
	fn add_candidate() -> Weight;
	fn remove_candidate() -> Weight;
}

impl WeightInfo for () {
	fn add_candidate() -> Weight {
		Weight::from_parts(10_000, 0)
	}

	fn remove_candidate() -> Weight {
		Weight::from_parts(10_000, 0)
	}
}
