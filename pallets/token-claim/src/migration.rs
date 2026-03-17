use frame_support::{
	pallet_prelude::*,
	traits::{GetStorageVersion, StorageVersion},
	weights::Weight,
};

use super::*;

pub fn migrate<T: Config>() -> Weight {
	let migrations: [(u16, &dyn Fn() -> Weight); 1] = [(1, &migrate_to_v1::<T>)];

	let mut onchain_version = Pallet::<T>::on_chain_storage_version();
	let mut weight: Weight = Default::default();
	for (i, f) in migrations.into_iter() {
		let migrating_version = StorageVersion::new(i);
		if onchain_version < migrating_version {
			weight = weight.saturating_add(f());
			migrating_version.put::<Pallet<T>>();
			onchain_version = migrating_version;
			weight = weight.saturating_add(T::DbWeight::get().writes(1));
		}
	}

	weight
}

mod v0 {
	use frame_support::pallet_prelude::*;
	use parity_scale_codec::{Decode, Encode};

	#[derive(RuntimeDebug, Encode, Decode, MaxEncodedLen, TypeInfo, Clone, PartialEq, Eq)]
	pub struct ClaimTypeConfig<AccountId, BlockNumber> {
		pub signer: AccountId,
		pub funder: AccountId,
		pub vesting_duration: BlockNumber,
	}
}

/// Removes the `signer` field from [`ClaimTypeConfig`], using `funder` for both roles.
pub fn migrate_to_v1<T: Config>() -> Weight {
	use frame_system::pallet_prelude::BlockNumberFor;

	let mut weight = Weight::zero();

	ClaimTypeConfigs::<T>::translate::<v0::ClaimTypeConfig<T::AccountId, BlockNumberFor<T>>, _>(
		|_id, old| {
			weight = weight.saturating_add(T::DbWeight::get().reads_writes(1, 1));
			Some(ClaimTypeConfigFor::<T> {
				funder: old.funder,
				vesting_duration: old.vesting_duration,
			})
		},
	);

	weight
}
