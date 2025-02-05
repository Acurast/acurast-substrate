use frame_support::{
	traits::{
		// Currency,
		Get,
		GetStorageVersion,
		StorageVersion,
		// WithdrawReasons,
	},
	weights::Weight,
};

use super::*;

pub fn migrate<T: Config<I>, I: 'static>() -> Weight {
	let migrations: [(u16, &dyn Fn() -> Weight); 1] = [(1, &migrate_to_v1::<T, I>)];

	let onchain_version = Pallet::<T, I>::on_chain_storage_version();
	let mut weight: Weight = Default::default();
	for (i, f) in migrations.into_iter() {
		if onchain_version < StorageVersion::new(i) {
			weight += f();
		}
	}

	STORAGE_VERSION.put::<Pallet<T, I>>();
	weight + T::DbWeight::get().writes(1)
}

/// Adds `config` to [`MetricPool`];
pub fn migrate_to_v1<T: Config<I>, I: 'static>() -> Weight {
	let mut weight = Weight::zero();
	weight = weight.saturating_add(
		T::DbWeight::get().reads(<MetricPools<T, I>>::iter_values().count() as u64),
	);
	<MetricPools<T, I>>::translate_values::<v0::MetricPoolFor<T, I>, _>(|old| {
		Some(MetricPoolFor::<T, I> {
			config: Default::default(),
			name: old.name,
			reward: old.reward,
			total: old.total,
		})
	});
	weight
}

pub mod v0 {
	use core::ops::Add;

	use super::*;
	use frame_support::pallet_prelude::*;
	use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
	use sp_runtime::{
		traits::{Debug, One},
		FixedU128, Perquintill,
	};

	#[derive(
		RuntimeDebugNoBound, Encode, Decode, MaxEncodedLen, TypeInfo, Clone, PartialEq, Eq,
	)]
	pub struct MetricPool<
		Epoch: Copy + Ord + One + Add<Output = Epoch> + Debug,
		Value: Copy + Default + Debug,
	> {
		pub name: MetricPoolName,
		pub reward: ProvisionalBuffer<Epoch, Value>,
		pub total: SlidingBuffer<Epoch, FixedU128>,
	}

	pub type MetricPoolFor<T, I> = MetricPool<EpochOf<T, I>, Perquintill>;
}
