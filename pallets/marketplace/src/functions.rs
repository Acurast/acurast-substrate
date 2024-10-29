use frame_support::{ensure, pallet_prelude::DispatchResult, sp_runtime::DispatchError};
use pallet_acurast::{utils::ensure_source_verified, JobId, StoredJobRegistration};
use reputation::{BetaParameters, BetaReputation, ReputationEngine};
use sp_core::Get;

use crate::{
	AdvertisementFor, AdvertisementRestriction, AssignedProcessors, AssignmentFor,
	AssignmentStrategy, Config, Error, JobRequirementsFor, ManagerProvider, Pallet, RewardManager,
	StorageTracker, StoredAdvertisementPricing, StoredAdvertisementRestriction,
	StoredAverageRewardV3, StoredMatches, StoredReputation, StoredStorageCapacity,
	StoredTotalAssignedV3,
};

impl<T: Config> Pallet<T> {
	pub fn do_advertise(
		processor: &T::AccountId,
		advertisement: &AdvertisementFor<T>,
	) -> DispatchResult {
		if let Some(allowed_consumers) = &advertisement.allowed_consumers {
			let max_allowed_consumers_len = T::MaxAllowedSources::get() as usize;
			ensure!(allowed_consumers.len() > 0, Error::<T>::TooFewAllowedConsumers);
			ensure!(
				allowed_consumers.len() <= max_allowed_consumers_len,
				Error::<T>::TooManyAllowedConsumers
			);
		}

		// update capacity to save on operations when checking available capacity
		if let Some(old) = <StoredAdvertisementRestriction<T>>::get(processor) {
			// allow capacity to become negative (in which case source remains assigned but does not receive new jobs assigned)
			<StoredStorageCapacity<T>>::mutate(processor, |c| {
				// new remaining capacity = new total capacity - (old total capacity - old remaining capacity) = old remaining capacity + new total capacity - old total capacity
				*c = Some(
					c.unwrap_or(0)
						.checked_add(advertisement.storage_capacity as i64)
						.unwrap_or(i64::MAX)
						.checked_sub(old.storage_capacity as i64)
						.unwrap_or(0),
				)
			});
		} else {
			<StoredStorageCapacity<T>>::insert(processor, advertisement.storage_capacity as i64);
		}

		<StoredAdvertisementRestriction<T>>::insert(
			processor,
			AdvertisementRestriction {
				max_memory: advertisement.max_memory,
				network_request_quota: advertisement.network_request_quota,
				storage_capacity: advertisement.storage_capacity,
				allowed_consumers: advertisement.allowed_consumers.clone(),
				available_modules: advertisement.available_modules.clone(),
			},
		);
		// update separate pricing index
		<StoredAdvertisementPricing<T>>::insert(processor, advertisement.pricing.clone());
		<StoredReputation<T>>::mutate(processor, |r| {
			if r.is_none() {
				*r = Some(BetaParameters::default());
			}
		});

		Ok(())
	}

	pub fn do_report(
		job_id: &JobId<T::AccountId>,
		processor: &T::AccountId,
	) -> Result<AssignmentFor<T>, DispatchError> {
		let assignment = <StoredMatches<T>>::try_mutate(
			processor,
			job_id,
			|a| -> Result<AssignmentFor<T>, Error<T>> {
				if let Some(assignment) = a.as_mut() {
					// CHECK that job is assigned
					ensure!(assignment.acknowledged, Error::<T>::CannotReportWhenNotAcknowledged);

					// CHECK that we don't accept more reports than expected
					ensure!(
						assignment.sla.met < assignment.sla.total,
						Error::<T>::MoreReportsThanExpected
					);

					assignment.sla.met += 1;
					Ok(assignment.to_owned())
				} else {
					Err(Error::<T>::ReportFromUnassignedSource)
				}
			},
		)?;

		let registration = <StoredJobRegistration<T>>::get(&job_id.0, job_id.1)
			.ok_or(pallet_acurast::Error::<T>::JobRegistrationNotFound)?;
		let e: <T as Config>::RegistrationExtra = registration.extra.clone().into();
		let requirements: JobRequirementsFor<T> = e.into();

		if let AssignmentStrategy::Competing = requirements.assignment_strategy {
			// The tracking deviates for the competing assignment strategy:
			// already unlock storage after each reported execution since we do not support cross-execution persistence for the competing assignment model
			<Pallet<T> as StorageTracker<T>>::unlock(processor, &registration)?;
		};

		let now = Self::now()?;
		let now_max = now
			.checked_add(T::ReportTolerance::get())
			.ok_or(Error::<T>::CalculationOverflow)?;

		ensure!(
			registration
				.schedule
				.overlaps(
					assignment.start_delay,
					(
						registration
							.schedule
							.range(assignment.start_delay)
							.ok_or(Error::<T>::CalculationOverflow)?
							.0,
						now_max
					)
				)
				.ok_or(Error::<T>::CalculationOverflow)?,
			Error::<T>::ReportOutsideSchedule
		);

		// the manager might have unpaired the processor in which case reward payment is skipped
		if let Ok(manager) = T::ManagerProvider::manager_of(processor) {
			T::RewardManager::pay_reward(job_id, assignment.fee_per_execution, &manager)?;
		}

		Ok(assignment)
	}

	pub fn do_finalize_job(
		job_id: &JobId<T::AccountId>,
		processor: &T::AccountId,
	) -> Result<(), DispatchError> {
		let registration = <StoredJobRegistration<T>>::get(&job_id.0, job_id.1)
			.ok_or(pallet_acurast::Error::<T>::JobRegistrationNotFound)?;
		let e: <T as Config>::RegistrationExtra = registration.extra.clone().into();
		let requirements: JobRequirementsFor<T> = e.into();

		// find assignment
		let assignment =
			<StoredMatches<T>>::get(processor, job_id).ok_or(Error::<T>::JobNotAssigned)?;

		ensure!(
			Self::actual_schedule_ended(&registration.schedule, &assignment)?,
			Error::<T>::JobCannotBeFinalized
		);

		let unmet: u64 = assignment.sla.total - assignment.sla.met;

		// update reputation since we don't expect further reports for this job
		// (only update for attested devices!)
		if ensure_source_verified::<T>(processor).is_ok() {
			// skip reputation update if reward is 0
			if assignment.fee_per_execution > 0u8.into() {
				let average_reward = <StoredAverageRewardV3<T>>::get().unwrap_or(0);
				let total_assigned = <StoredTotalAssignedV3<T>>::get().unwrap_or_default();

				let total_reward = average_reward
					.checked_mul(total_assigned - 1u128)
					.ok_or(Error::<T>::CalculationOverflow)?;

				let new_total_rewards = total_reward
					.checked_add(assignment.fee_per_execution.into())
					.ok_or(Error::<T>::CalculationOverflow)?;

				let mut beta_params =
					<StoredReputation<T>>::get(processor).ok_or(Error::<T>::ReputationNotFound)?;

				beta_params = BetaReputation::update(
					beta_params,
					assignment.sla.met,
					unmet,
					assignment.fee_per_execution,
					average_reward.into(),
				)
				.ok_or(Error::<T>::CalculationOverflow)?;

				let new_average_reward = new_total_rewards
					.checked_div(total_assigned)
					.ok_or(Error::<T>::CalculationOverflow)?;

				<StoredAverageRewardV3<T>>::set(Some(new_average_reward));
				<StoredReputation<T>>::insert(
					processor,
					BetaParameters { r: beta_params.r, s: beta_params.s },
				);
			}
		}

		// only remove storage point indexed by a single processor (corresponding to the completed duties for the assigned slot)
		<StoredMatches<T>>::remove(processor, job_id);
		<AssignedProcessors<T>>::remove(job_id, processor);

		// for single assigned slots we support cross-execution persistence and only unlock storage on job finalization
		if let AssignmentStrategy::Single(_) = requirements.assignment_strategy {
			<Pallet<T> as StorageTracker<T>>::unlock(processor, &registration)?;
		}

		Ok(())
	}
}
