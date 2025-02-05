use acurast_common::{ManagerIdProvider, Version};
use frame_support::{
	pallet_prelude::DispatchResult,
	sp_runtime::{traits::CheckedAdd, DispatchError, SaturatedConversion},
};

use crate::{
	Config, Error, LastManagerId, ManagedProcessors, Pallet, ProcessorRewardDistributionWindow,
	ProcessorRewardDistributor, ProcessorToManagerIdIndex, RewardDistributionWindow,
};

impl<T: Config> Pallet<T> {
	/// Returns the manager account id (if any) for the given processor account.
	pub fn manager_for_processor(processor_account: &T::AccountId) -> Option<T::AccountId> {
		let id = Self::manager_id_for_processor(processor_account)?;
		<T::ManagerIdProvider>::owner_for(id).ok()
	}

	/// Returns the manager id for the given manager account. If a manager id does not exist it is first created.
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

	pub(crate) fn is_elegible_for_reward(processor: &T::AccountId, version: &Version) -> bool {
		if !T::ProcessorRewardDistributor::is_elegible_for_reward(processor) {
			return false;
		}

		if let Some(min_req_version) = Self::processor_min_version_for_reward(version.platform) {
			if version.build_number < min_req_version {
				return false;
			}
		}

		return true;
	}

	pub(crate) fn do_reward_distribution(
		processor: &T::AccountId,
		version: &Version,
	) -> Option<T::Balance> {
		if !Self::is_elegible_for_reward(processor, version) {
			return None;
		}

		if let Some(distribution_settings) = Self::processor_reward_distribution_settings() {
			if let Some(manager) = Self::manager_for_processor(processor) {
				let current_block_number: u32 =
					<frame_system::Pallet<T>>::block_number().saturated_into();

				if let Some(distribution_window) =
					Self::processor_reward_distribution_window(processor)
				{
					let progress = current_block_number.saturating_sub(distribution_window.start);
					if progress >= distribution_window.window_length {
						let mut distributed_amount: Option<T::Balance> = None;
						let buffer = progress.saturating_sub(distribution_window.window_length);
						if buffer <= distribution_window.tollerance
							&& (distribution_window.heartbeats + 1)
								>= distribution_window.min_heartbeats
						{
							let result = T::ProcessorRewardDistributor::distribute_reward(
								&manager,
								distribution_settings.reward_per_distribution,
								&distribution_settings.distributor_account,
							);
							if result.is_ok() {
								distributed_amount =
									Some(distribution_settings.reward_per_distribution)
							}
						}
						<ProcessorRewardDistributionWindow<T>>::insert(
							processor,
							RewardDistributionWindow::new(
								current_block_number,
								&distribution_settings,
							),
						);
						return distributed_amount;
					} else {
						<ProcessorRewardDistributionWindow<T>>::insert(
							processor,
							distribution_window.next(),
						);
					}
				} else {
					<ProcessorRewardDistributionWindow<T>>::insert(
						processor,
						RewardDistributionWindow::new(current_block_number, &distribution_settings),
					);
				}
			}
		}
		None
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
