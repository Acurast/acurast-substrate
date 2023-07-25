use crate::{staking_info, Balance, Perbill};
use core::marker::PhantomData;
use frame_support::{
	pallet_prelude::*,
	traits::{OnRuntimeUpgrade, OriginTrait, ReservableCurrency},
};
use frame_system::limits::BlockWeights;
use sp_std::vec::Vec;

const MAX_SELECTED_COLLATORS: u32 = 128;

/// Initialize pallet staking
pub struct StakingPalletBootstrapping<T>(PhantomData<T>);
impl<T> OnRuntimeUpgrade for StakingPalletBootstrapping<T>
where
	T: frame_system::Config + pallet_parachain_staking::Config + pallet_collator_selection::Config,
	<T as frame_system::Config>::AccountId: From<[u8; 32]>,
	pallet_parachain_staking::BalanceOf<T>: Into<Balance> + From<Balance>,
{
	fn on_runtime_upgrade() -> Weight {
		let invulnerables = pallet_collator_selection::Pallet::<T>::invulnerables();
		let _ = pallet_collator_selection::Pallet::<T>::set_invulnerables(
			<T as frame_system::Config>::RuntimeOrigin::root(),
			Vec::new(),
		);

		let _ = pallet_collator_selection::Pallet::<T>::set_desired_candidates(
			<T as frame_system::Config>::RuntimeOrigin::root(),
			invulnerables.len() as u32,
		);

		// Reserve enough funds to candidate as collator
		for candidate in invulnerables.clone() {
			let _ = <T as pallet_parachain_staking::Config>::Currency::reserve(
				&candidate,
				staking_info::MINIMUM_COLLATOR_STAKE.into(),
			);
		}

		// Use the next block since migration code is executed before
		// the on_initialize hook
		let next_block = frame_system::Pallet::<T>::block_number() + 1u32.into();
		pallet_parachain_staking::Pallet::<T>::initialize_pallet(
			next_block,
			invulnerables.to_vec(),
			crate::staking_info::DEFAULT_INFLATION_CONFIG,
			Perbill::from_percent(20),
		)
		.unwrap_or_else(|err| {
			log::error!("pallet_parachain_staking initialization failed with {:?}.", err);
		});

		let _ = pallet_parachain_staking::Pallet::<T>::set_total_selected(
			<T as frame_system::Config>::RuntimeOrigin::root(),
			MAX_SELECTED_COLLATORS,
		);

		// Reserve 50% of the block for the upgrade
		Perbill::from_percent(50) * BlockWeights::default().max_block
	}
}
