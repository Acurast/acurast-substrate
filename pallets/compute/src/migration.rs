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
use sp_runtime::traits::Zero;

use super::*;

pub fn migrate<T: Config<I>, I: 'static>() -> Weight {
	let migrations: [(u16, &dyn Fn() -> Weight); 2] =
		[(2, &migrate_to_v2::<T, I>), (3, &migrate_to_v3::<T, I>)];

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

/// Adds `max_stake_metric_ratio` to [`MetricPool`];
pub fn migrate_to_v2<T: Config<I>, I: 'static>() -> Weight {
	let mut weight = Weight::zero();
	weight = weight.saturating_add(
		T::DbWeight::get().reads(<MetricPools<T, I>>::iter_values().count() as u64),
	);
	<MetricPools<T, I>>::translate_values::<v1::MetricPoolFor<T>, _>(|old| {
		Some(MetricPoolFor::<T> {
			config: old.config,
			name: old.name,
			reward: old.reward,
			total: old.total,
			max_stake_metric_ratio: Zero::zero(),
		})
	});
	weight
}

/// Adds `total_inflation_per_distribution` and `stake_backed_ratio` to [`RewardSettings`];
pub fn migrate_to_v3<T: Config<I>, I: 'static>() -> Weight {
	let mut weight = Weight::zero();

	// Migrate RewardDistributionSettings to new format
	let reads = if <RewardDistributionSettings<T, I>>::exists() { 1 } else { 0 };
	weight = weight.saturating_add(T::DbWeight::get().reads(reads));

	let _ = <RewardDistributionSettings<T, I>>::translate::<v2::RewardSettingsFor<T, I>, _>(
		|old_settings_opt| {
			old_settings_opt.map(|old_settings| {
				// Migrate to new structure with default values for new fields
				RewardDistributionSettingsFor::<T, I> {
					total_reward_per_distribution: old_settings.total_reward_per_distribution,
					total_inflation_per_distribution: sp_runtime::Perquintill::zero(), // Default to no inflation
					stake_backed_ratio: sp_runtime::Perquintill::from_percent(70), // Default to 70% for stake-backed
					distribution_account: old_settings.distribution_account,
				}
			})
		},
	);

	weight = weight.saturating_add(T::DbWeight::get().writes(if reads > 0 { 1 } else { 0 }));

	weight
}

pub mod v2 {
	use super::*;
	use frame_support::pallet_prelude::*;
	use parity_scale_codec::{Decode, Encode, MaxEncodedLen};

	#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, MaxEncodedLen, TypeInfo)]
	pub struct RewardSettings<Balance, AccountId> {
		pub total_reward_per_distribution: Balance,
		pub distribution_account: AccountId,
	}

	pub type RewardSettingsFor<T, I> =
		RewardSettings<BalanceFor<T, I>, <T as frame_system::Config>::AccountId>;
}

pub mod v1 {
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
		pub config: MetricPoolConfigValues,
		pub name: MetricPoolName,
		pub reward: ProvisionalBuffer<Epoch, Value>,
		pub total: SlidingBuffer<Epoch, FixedU128>,
	}

	pub type MetricPoolFor<T> = MetricPool<EpochOf<T>, Perquintill>;
}
