use frame_support::{traits::Get, weights::Weight};
use sp_std::marker::PhantomData;

pub struct WeightInfo<T>(PhantomData<T>);
impl<T: frame_system::Config> crate::WeightInfo for WeightInfo<T> {
	fn convert() -> Weight {
		Weight::from_parts(13_000_000, 0)
			.saturating_add(Weight::from_parts(0, 4668))
			.saturating_add(T::DbWeight::get().reads(1))
			.saturating_add(T::DbWeight::get().writes(3))
	}

	fn unlock() -> Weight {
		Weight::from_parts(13_000_000, 0)
			.saturating_add(Weight::from_parts(0, 4668))
			.saturating_add(T::DbWeight::get().reads(1))
			.saturating_add(T::DbWeight::get().writes(3))
	}

	fn retry_convert() -> Weight {
		Weight::from_parts(13_000_000, 0)
			.saturating_add(Weight::from_parts(0, 4668))
			.saturating_add(T::DbWeight::get().reads(1))
			.saturating_add(T::DbWeight::get().writes(3))
	}

	fn retry_convert_for() -> Weight {
		Weight::from_parts(13_000_000, 0)
			.saturating_add(Weight::from_parts(0, 4668))
			.saturating_add(T::DbWeight::get().reads(1))
			.saturating_add(T::DbWeight::get().writes(3))
	}

	fn retry_process_conversion() -> Weight {
		Weight::from_parts(13_000_000, 0)
			.saturating_add(Weight::from_parts(0, 4668))
			.saturating_add(T::DbWeight::get().reads(1))
			.saturating_add(T::DbWeight::get().writes(3))
	}

	fn retry_process_conversion_for() -> Weight {
		Weight::from_parts(13_000_000, 0)
			.saturating_add(Weight::from_parts(0, 4668))
			.saturating_add(T::DbWeight::get().reads(1))
			.saturating_add(T::DbWeight::get().writes(3))
	}

	fn set_enabled() -> Weight {
		Weight::from_parts(4_371_000, 0)
			.saturating_add(Weight::from_parts(0, 0))
			.saturating_add(T::DbWeight::get().writes(1))
	}

	fn deny_source() -> Weight {
		Weight::from_parts(4_371_000, 0)
			.saturating_add(Weight::from_parts(0, 0))
			.saturating_add(T::DbWeight::get().writes(1))
	}
}
