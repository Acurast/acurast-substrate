use acurast_common::{
	AccountLookup, ManagerIdProvider, OnboardingCounterProvider, ProcessorVersionProvider, Version,
};
use frame_support::{
	pallet_prelude::{DispatchResult, Zero},
	sp_runtime::{traits::CheckedAdd, DispatchError, SaturatedConversion},
	traits::{Currency, ExistenceRequirement},
};

use crate::{
	BalanceFor, Config, Error, LastManagerId, ManagedProcessors, Pallet,
	ProcessorRewardDistributionWindow, ProcessorToManagerIdIndex, RewardDistributionWindow,
};

impl<T: Config> Pallet<T> {
	/// Returns the manager id for the given manager account. If a manager id does not exist it is first created.
	pub fn do_get_or_create_manager_id(
		manager: &T::AccountId,
	) -> Result<(T::ManagerId, bool), DispatchError> {
		T::ManagerIdProvider::manager_id_for(manager)
			.map(|id| (id, false))
			.or_else::<DispatchError, _>(|_| {
				let id = <LastManagerId<T>>::get()
					.unwrap_or(0u128.into())
					.checked_add(&1u128.into())
					.ok_or(Error::<T>::FailedToCreateManagerId)?;

				T::ManagerIdProvider::create_manager_id(id, manager)?;
				<LastManagerId<T>>::set(Some(id));

				Ok((id, true))
			})
	}

	/// Adds a pairing between the given processor account and manager id. It fails if the manager id does not exists of
	/// if the processor account was already paired.
	pub fn do_add_processor_manager_pairing(
		processor_account: &T::AccountId,
		manager_id: T::ManagerId,
	) -> DispatchResult {
		if let Some(id) = Self::manager_id_for_processor(processor_account) {
			if id == manager_id {
				return Err(Error::<T>::ProcessorAlreadyPaired)?;
			}
			return Err(Error::<T>::ProcessorPairedWithAnotherManager)?;
		}
		<ManagedProcessors<T>>::insert(manager_id, processor_account, ());
		<ProcessorToManagerIdIndex<T>>::insert(processor_account, manager_id);

		Ok(())
	}

	/// Removes the pairing between a processor account and manager id. It fails if the processor account is paired
	/// with a different manager id.
	pub fn do_remove_processor_manager_pairing(
		processor_account: &T::AccountId,
		manager: &T::AccountId,
	) -> DispatchResult {
		let id = Self::ensure_managed(manager, processor_account)?;
		<ManagedProcessors<T>>::remove(id, processor_account);
		<ProcessorToManagerIdIndex<T>>::remove(processor_account);
		Ok(())
	}

	pub(crate) fn do_reward_distribution(processor: &T::AccountId) -> Option<BalanceFor<T>> {
		let Some(distribution_settings) = Self::processor_reward_distribution_settings() else {
			<ProcessorRewardDistributionWindow<T>>::remove(processor);
			return None;
		};
		if distribution_settings.reward_per_distribution.is_zero() {
			<ProcessorRewardDistributionWindow<T>>::remove(processor);
			return None;
		}
		let Some(manager) = T::EligibleRewardAccountLookup::lookup(processor) else {
			<ProcessorRewardDistributionWindow<T>>::remove(processor);
			return None;
		};
		let current_block_number: u32 = <frame_system::Pallet<T>>::block_number().saturated_into();
		let Some(distribution_window) = Self::processor_reward_distribution_window(processor)
		else {
			<ProcessorRewardDistributionWindow<T>>::insert(
				processor,
				RewardDistributionWindow::new(current_block_number, &distribution_settings),
			);
			return None;
		};

		let progress = current_block_number.saturating_sub(distribution_window.start);
		if progress < distribution_window.window_length {
			<ProcessorRewardDistributionWindow<T>>::insert(processor, distribution_window.next());
			return None;
		}

		let mut distributed_amount: Option<BalanceFor<T>> = None;
		let buffer = progress.saturating_sub(distribution_window.window_length);
		if buffer <= distribution_window.tollerance
			&& (distribution_window.heartbeats + 1) >= distribution_window.min_heartbeats
		{
			let result = T::Currency::transfer(
				&distribution_settings.distributor_account,
				&manager,
				distribution_settings.reward_per_distribution,
				ExistenceRequirement::KeepAlive,
			);
			if result.is_ok() {
				distributed_amount = Some(distribution_settings.reward_per_distribution)
			}
		}
		<ProcessorRewardDistributionWindow<T>>::insert(
			processor,
			RewardDistributionWindow::new(current_block_number, &distribution_settings),
		);
		return distributed_amount;
	}

	pub fn ensure_managed(
		manager: &T::AccountId,
		processor: &T::AccountId,
	) -> Result<T::ManagerId, DispatchError> {
		let processor_manager_id =
			Self::manager_id_for_processor(processor).ok_or(Error::<T>::ProcessorHasNoManager)?;

		let processor_manager = T::ManagerIdProvider::owner_for(processor_manager_id)?;

		if manager != &processor_manager {
			return Err(Error::<T>::ProcessorPairedWithAnotherManager)?;
		}

		Ok(processor_manager_id)
	}
}

impl<T: Config> ProcessorVersionProvider<T::AccountId> for Pallet<T> {
	fn processor_version(processor: &T::AccountId) -> Option<Version> {
		Self::processor_version(processor)
	}

	fn min_version_for_reward(platform: u32) -> Option<Version> {
		let build_number = Self::processor_min_version_for_reward(platform);
		build_number.map(|build_number| Version { platform, build_number })
	}
}

impl<T: Config> AccountLookup<T::AccountId> for Pallet<T> {
	fn lookup(processor: &T::AccountId) -> Option<T::AccountId> {
		let manager_id = Self::manager_id_for_processor(processor)?;
		T::ManagerIdProvider::owner_for(manager_id).ok()
	}
}

impl<T: Config> OnboardingCounterProvider<T::AccountId, T::Counter> for Pallet<T> {
	fn counter(manager: &T::AccountId) -> Option<T::Counter> {
		Self::counter_for_manager(manager)
	}
}
