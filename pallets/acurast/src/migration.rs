use frame_support::{
	traits::{GetStorageVersion, StorageVersion},
	weights::Weight,
	IterableStorageMap,
};
use sp_core::Get;

use super::*;

mod v4 {
	use acurast_common::{AttestationValidity, BoundedKeyDescription, ValidatingCertIds};
	use frame_support::pallet_prelude::*;
	use sp_std::prelude::*;

	#[derive(RuntimeDebug, Encode, Decode, MaxEncodedLen, TypeInfo, Clone, PartialEq)]
	pub struct Attestation {
		pub cert_ids: ValidatingCertIds,
		pub key_description: BoundedKeyDescription,
		pub validity: AttestationValidity,
	}
}

pub fn migrate<T: Config>() -> Weight {
	let migrations: [(u16, &dyn Fn(Weight) -> Weight); 1] = [(5, &migrate_to_v5::<T>)];

	let on_chain_version = Pallet::<T>::on_chain_storage_version();
	let mut weight: Weight = T::DbWeight::get().reads(1);
	for (i, f) in migrations.into_iter() {
		if on_chain_version < StorageVersion::new(i) {
			weight += f(weight);
		}
	}

	weight
}

fn migrate_to_v5<T: Config>(weight: Weight) -> Weight {
	let weights = T::BlockWeights::get();
	let mut weight = weight;
	let max_weight = weights.max_block - weights.base_block;
	let mut cursor = V5MigrationState::<T>::get();
	weight += T::DbWeight::get().reads_writes(1, 1);
	crate::Pallet::<T>::deposit_event(Event::V5MigrationStarted);
	loop {
		cursor = StoredAttestation::<T>::translate_next::<v4::Attestation, _>(
			cursor.map(|v| v.to_vec()),
			|_, old_value| {
				Some(Attestation {
					cert_ids: old_value.cert_ids,
					content: BoundedAttestationContent::KeyDescription(old_value.key_description),
					validity: old_value.validity,
				})
			},
		)
		.map(|cursor| cursor.try_into().unwrap());
		weight += T::DbWeight::get().reads_writes(1, 1);
		if weight.any_gte(max_weight) || cursor.is_none() {
			crate::Pallet::<T>::deposit_event(Event::V5MigrationProgress(
				weight,
				max_weight.saturating_sub(weight),
			));
			if cursor.is_none() {
				STORAGE_VERSION.put::<Pallet<T>>();
				crate::Pallet::<T>::deposit_event(Event::V5MigrationCompleted);
			}
			V5MigrationState::<T>::put(cursor);
			break;
		}
	}

	weight
}
