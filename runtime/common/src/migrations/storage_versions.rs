use core::marker::PhantomData;

use frame_support::{
	storage::migration::{clear_storage_prefix, have_storage_value},
	traits::{Get, GetStorageVersion, OnRuntimeUpgrade, PalletInfoAccess, StorageVersion},
};
use sp_runtime::Weight;

/// Legacy storage item of `cumulus_pallet_aura_ext`, replaced by `RelaySlotInfo`. Its `v0 -> v1`
/// migration does nothing but remove it.
const AURA_EXT_LEGACY_SLOT_INFO: &[u8] = b"SlotInfo";

/// One-shot repair of stale on-chain [`StorageVersion`] markers of upstream polkadot-sdk pallets.
pub struct StorageVersionBackfill<T>(PhantomData<T>);

impl<T> OnRuntimeUpgrade for StorageVersionBackfill<T>
where
	T: frame_system::Config
		+ cumulus_pallet_parachain_system::Config
		+ cumulus_pallet_xcmp_queue::Config
		+ cumulus_pallet_aura_ext::Config
		+ pallet_xcm::Config
		+ pallet_scheduler::Config
		+ pallet_collator_selection::Config
		+ pallet_session::Config
		+ pallet_uniques::Config,
{
	fn on_runtime_upgrade() -> Weight {
		let mut writes: u64 = 0;

		writes += backfill::<cumulus_pallet_parachain_system::Pallet<T>>() as u64;
		writes += backfill::<cumulus_pallet_xcmp_queue::Pallet<T>>() as u64;
		writes += backfill::<pallet_xcm::Pallet<T>>() as u64;
		writes += backfill::<pallet_scheduler::Pallet<T>>() as u64;
		writes += backfill::<pallet_collator_selection::Pallet<T>>() as u64;
		writes += backfill::<pallet_session::Pallet<T>>() as u64;
		writes += backfill::<cumulus_pallet_aura_ext::Pallet<T>>() as u64;
		writes += backfill::<pallet_uniques::Pallet<T>>() as u64;

		writes += kill_aura_ext_legacy_slot_info::<T>() as u64;

		<T as frame_system::Config>::DbWeight::get().reads_writes(9, writes)
	}

	#[cfg(feature = "try-runtime")]
	fn post_upgrade(_state: sp_std::vec::Vec<u8>) -> Result<(), sp_runtime::TryRuntimeError> {
		ensure_in_sync::<cumulus_pallet_parachain_system::Pallet<T>>()?;
		ensure_in_sync::<cumulus_pallet_xcmp_queue::Pallet<T>>()?;
		ensure_in_sync::<pallet_xcm::Pallet<T>>()?;
		ensure_in_sync::<pallet_scheduler::Pallet<T>>()?;
		ensure_in_sync::<pallet_collator_selection::Pallet<T>>()?;
		ensure_in_sync::<pallet_session::Pallet<T>>()?;
		ensure_in_sync::<cumulus_pallet_aura_ext::Pallet<T>>()?;
		ensure_in_sync::<pallet_uniques::Pallet<T>>()?;

		frame_support::ensure!(
			!have_storage_value(
				<cumulus_pallet_aura_ext::Pallet<T> as PalletInfoAccess>::name().as_bytes(),
				AURA_EXT_LEGACY_SLOT_INFO,
				b"",
			),
			"legacy AuraExt::SlotInfo still present after backfill"
		);

		Ok(())
	}
}

/// Sets the on-chain storage version of pallet `P` to its in-code declared version if the on-chain
/// marker lags behind. Returns whether a write happened.
fn backfill<P>() -> bool
where
	P: GetStorageVersion<InCodeStorageVersion = StorageVersion> + PalletInfoAccess,
{
	let on_chain = P::on_chain_storage_version();
	let in_code = P::in_code_storage_version();

	if on_chain < in_code {
		log::info!("storage version backfill: {} {:?} -> {:?}", P::name(), on_chain, in_code);
		in_code.put::<P>();
		true
	} else {
		false
	}
}

/// Removes the legacy `cumulus_pallet_aura_ext::SlotInfo` value, the one piece of cleanup owed by
/// the `v0 -> v1` migration that the marker backfill skips. Returns whether a write happened.
fn kill_aura_ext_legacy_slot_info<T>() -> bool
where
	T: cumulus_pallet_aura_ext::Config,
{
	let pallet = <cumulus_pallet_aura_ext::Pallet<T> as PalletInfoAccess>::name();

	if have_storage_value(pallet.as_bytes(), AURA_EXT_LEGACY_SLOT_INFO, b"") {
		log::info!("storage version backfill: removing legacy {}::SlotInfo", pallet);
		let _ = clear_storage_prefix(pallet.as_bytes(), AURA_EXT_LEGACY_SLOT_INFO, b"", None, None);
		true
	} else {
		false
	}
}

/// Asserts that pallet `P`'s on-chain marker matches its in-code declared version.
#[cfg(feature = "try-runtime")]
fn ensure_in_sync<P>() -> Result<(), sp_runtime::TryRuntimeError>
where
	P: GetStorageVersion<InCodeStorageVersion = StorageVersion> + PalletInfoAccess,
{
	frame_support::ensure!(
		P::on_chain_storage_version() == P::in_code_storage_version(),
		"storage version marker not in sync after backfill"
	);

	Ok(())
}
