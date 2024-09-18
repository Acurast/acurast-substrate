use frame_support::{
	traits::{GetStorageVersion, StorageVersion},
	weights::Weight,
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
	let migrations: [(u16, &dyn Fn() -> Weight); 1] = [(5, &migrate_to_v5::<T>)];

	let on_chain_version = Pallet::<T>::on_chain_storage_version();
	let mut weight: Weight = Default::default();
	for (i, f) in migrations.into_iter() {
		if on_chain_version < StorageVersion::new(i) {
			weight += f();
		}
	}

	STORAGE_VERSION.put::<Pallet<T>>();
	weight + T::DbWeight::get().writes(1)
}

fn migrate_to_v5<T: Config>() -> Weight {
	StoredAttestation::<T>::translate_values::<v4::Attestation, _>(|old_value| {
		Some(Attestation {
			cert_ids: old_value.cert_ids,
			content: BoundedAttestationContent::KeyDescription(old_value.key_description),
			validity: old_value.validity,
		})
	});
	let count = StoredAttestation::<T>::iter_values().count() as u64;
	T::DbWeight::get().reads_writes(count + 1, count + 1)
}
