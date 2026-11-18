use frame_support::{
	traits::{GetStorageVersion, StorageVersion},
	weights::Weight,
};
use parity_scale_codec::Decode;
use sp_core::Get;

use crate::{BalanceFor, Config, OnboardingSettings, Pallet, ProcessorOnboardingSettings};

#[derive(Decode)]
struct OnboardingSettingsV1<Balance, AccountId> {
	funds: Balance,
	max_funds: Balance,
	_funds_account: AccountId,
}

const TARGET: StorageVersion = StorageVersion::new(2);

/// Drops `funds_account` from [`ProcessorOnboardingSettings`].
pub fn on_runtime_upgrade<T: Config>() -> Weight {
	if Pallet::<T>::on_chain_storage_version() >= TARGET {
		return T::DbWeight::get().reads(1);
	}
	let _ = ProcessorOnboardingSettings::<T>::translate(
		|old: Option<OnboardingSettingsV1<BalanceFor<T>, T::AccountId>>| {
			old.map(|old| OnboardingSettings { funds: old.funds, max_funds: old.max_funds })
		},
	);
	TARGET.put::<Pallet<T>>();
	T::DbWeight::get().reads_writes(2, 2)
}
