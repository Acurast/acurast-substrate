use crate::*;

pub const fn deposit(items: u32, bytes: u32) -> Balance {
	items as Balance * 1 * UNIT * SUPPLY_FACTOR + (bytes as Balance) * STORAGE_BYTE_FEE
}

/// The version information used to identify this runtime when compiled natively.
#[cfg(feature = "std")]
pub fn native_version() -> NativeVersion {
	NativeVersion { runtime_version: VERSION, can_author_with: Default::default() }
}

pub fn get_fee_payer(
	who: &<Runtime as frame_system::Config>::AccountId,
	call: &<Runtime as frame_system::Config>::RuntimeCall,
) -> <Runtime as frame_system::Config>::AccountId {
	let mut manager = AcurastProcessorManager::manager_for_processor(who);

	if manager.is_none() {
		if let RuntimeCall::AcurastProcessorManager(
			pallet_acurast_processor_manager::Call::pair_with_manager { pairing },
		) = call
		{
			if pairing.validate_timestamp::<Runtime>() {
				let counter = AcurastProcessorManager::counter_for_manager(&pairing.account)
					.unwrap_or(0)
					.checked_add(1);
				if let Some(counter) = counter {
					if pairing.validate_signature::<Runtime>(&pairing.account, counter) {
						manager = Some(pairing.account.clone());
					}
				}
			}
		}
	}

	manager.unwrap_or(who.clone())
}
