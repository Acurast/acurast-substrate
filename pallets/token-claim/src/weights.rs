use frame_support::{traits::Get, weights::Weight};
use sp_std::marker::PhantomData;

pub struct WeightInfo<T>(PhantomData<T>);
impl<T: frame_system::Config> crate::WeightInfo for WeightInfo<T> {
	fn claim() -> Weight {
		Weight::from_parts(13_000_000, 0)
			.saturating_add(Weight::from_parts(0, 4668))
			.saturating_add(T::DbWeight::get().reads(1))
			.saturating_add(T::DbWeight::get().writes(3))
	}

	fn vest() -> Weight {
		Weight::from_parts(40_000_000, 0)
			.saturating_add(Weight::from_parts(0, 4668))
			.saturating_add(T::DbWeight::get().reads(2))
			.saturating_add(T::DbWeight::get().writes(2))
	}
}
