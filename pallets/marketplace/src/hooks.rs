use core::iter::once;

use crate::*;
use frame_support::{ensure, pallet_prelude::*};
use pallet_acurast::{
	AccountLookup, AllowedSourcesUpdate, JobHooks, JobRegistrationFor, StoredJobRegistration,
};

impl<T: Config> JobHooks<T> for Pallet<T> {
	/// Registers a job in the marketplace by providing a [JobRegistration].
	/// If a job for the same `(accountId, script)` was previously registered, it will be overwritten.
	fn register_hook(
		job_id: &JobId<T::AccountId>,
		registration: &JobRegistrationFor<T>,
	) -> DispatchResultWithPostInfo {
		let e: <T as Config>::RegistrationExtra = registration.extra.clone().into();
		let requirements: JobRequirementsFor<T> = e.into();

		ensure!(registration.schedule.duration > 0, Error::<T>::JobRegistrationZeroDuration);
		let execution_count = registration.schedule.execution_count();
		ensure!(
			execution_count <= MAX_EXECUTIONS_PER_JOB,
			Error::<T>::JobRegistrationScheduleExceedsMaximumExecutions
		);
		ensure!(execution_count > 0, Error::<T>::JobRegistrationScheduleContainsZeroExecutions);
		ensure!(
			registration.schedule.duration < registration.schedule.interval,
			Error::<T>::JobRegistrationDurationExceedsInterval
		);
		ensure!(
			registration.schedule.start_time >= Self::now()?,
			Error::<T>::JobRegistrationStartInPast
		);
		ensure!(
			registration.schedule.start_time <= registration.schedule.end_time,
			Error::<T>::JobRegistrationEndBeforeStart
		);
		ensure!(requirements.slots > 0, Error::<T>::JobRegistrationZeroSlots);
		ensure!(
			requirements.slots as u32 <= <T as pallet_acurast::Config>::MaxSlots::get(),
			Error::<T>::TooManySlots
		);

		if let Some(job_status) = <StoredJobStatus<T>>::get(&job_id.0, job_id.1) {
			ensure!(job_status == JobStatus::Open, Error::<T>::JobRegistrationUnmodifiable);
		} else {
			<StoredJobStatus<T>>::insert(&job_id.0, job_id.1, JobStatus::default());
		}

		match requirements.assignment_strategy {
			AssignmentStrategy::Single(instant_match) => {
				if let Some(sources) = instant_match {
					// ignore remaining rewards; do not pay out the matcher which is the same as the one registering
					let _ = Self::process_matching(once(&crate::types::Match {
						job_id: job_id.clone(),
						sources,
					}))?;
				}
			},
			AssignmentStrategy::Competing => {
				// ensure the interval is big enough for matchings and acknowledgments to happen

				ensure!(
					registration.schedule.execution_count() <= 1
						|| registration.schedule.interval >= T::MatchingCompetingMinInterval::get(),
					Error::<T>::JobRegistrationIntervalBelowMinimum
				);
			},
		}

		// - lock only after all other steps succeeded without errors because locking reward is not revertable
		// - reward is understood per slot and execution, so calculate total_reward_amount first
		// - lock the complete reward inclusive the matcher share and potential gap to actual fee that will be refunded during job finalization
		T::RewardManager::lock_reward(job_id, Self::total_reward_amount(registration)?)?;

		Ok(().into())
	}

	/// Deregisters a job.
	///
	/// The final act of removing the job from [`StoredJobRegistration`] is the responsibility of the caller,
	/// since this storage point is owned by pallet_acurast.
	fn deregister_hook(job_id: &JobId<T::AccountId>) -> DispatchResultWithPostInfo {
		let job_status =
			<StoredJobStatus<T>>::get(&job_id.0, job_id.1).ok_or(Error::<T>::JobStatusNotFound)?;
		let registration = <StoredJobRegistration<T>>::get(&job_id.0, job_id.1)
			.ok_or(pallet_acurast::Error::<T>::JobRegistrationNotFound)?;

		<JobKeyIds<T>>::remove(job_id);

		match job_status {
			JobStatus::Open => {
				T::MarketplaceHooks::finalize_job(job_id, T::RewardManager::refund(job_id)?)?;

				<StoredJobStatus<T>>::remove(&job_id.0, job_id.1);
				let _ = <StoredJobExecutionStatus<T>>::clear_prefix(
					job_id,
					registration.schedule.execution_count() as u32,
					None,
				);
			},
			JobStatus::Matched => {
				T::MarketplaceHooks::finalize_job(job_id, T::RewardManager::refund(job_id)?)?;

				// Remove matching data and increase processor capacity
				for (p, _) in <AssignedProcessors<T>>::drain_prefix(job_id) {
					<StoredMatches<T>>::remove(&p, job_id);

					<Pallet<T> as StorageTracker<T>>::unlock(&p, &registration)?;
				}

				<StoredJobStatus<T>>::remove(&job_id.0, job_id.1);
				let _ = <StoredJobExecutionStatus<T>>::clear_prefix(
					job_id,
					registration.schedule.execution_count() as u32,
					None,
				);
			},
			JobStatus::Assigned(_) => {
				// Get the job requirements
				let registration = <StoredJobRegistration<T>>::get(&job_id.0, job_id.1)
					.ok_or(pallet_acurast::Error::<T>::JobRegistrationNotFound)?;
				let now = Self::now()?;

				// Pay reward to the processor and clear matching data
				for (processor, _) in <AssignedProcessors<T>>::drain_prefix(job_id) {
					// find assignment
					let assignment = <StoredMatches<T>>::take(&processor, job_id);
					<Pallet<T> as StorageTracker<T>>::unlock(&processor, &registration)?;

					if let Some(assignment) = assignment {
						if let ExecutionSpecifier::Index(index) = assignment.execution {
							let next_execution_index = registration
								.schedule
								.next_execution_index(assignment.start_delay, now);
							if index != next_execution_index {
								continue;
							}
						}

						// Compensate processor for acknowledging the job
						if assignment.acknowledged {
							// the manager might have unpaired the processor in which case reward payment is skipped
							if let Some(manager) = T::ManagerProvider::lookup(&processor) {
								T::RewardManager::pay_reward(
									job_id,
									assignment.fee_per_execution,
									&manager,
								)?;
							};
						}
					}
				}

				// The job creator will only receive the amount that could not be divided between the acknowledged processors
				T::MarketplaceHooks::finalize_job(job_id, T::RewardManager::refund(job_id)?)?;

				<StoredJobStatus<T>>::remove(&job_id.0, job_id.1);
				let _ = <StoredJobExecutionStatus<T>>::clear_prefix(job_id, 10, None);
				let _ = <NextReportIndex<T>>::clear_prefix(
					job_id,
					<T as pallet_acurast::Config>::MaxSlots::get(),
					None,
				);
			},
		}

		Ok(().into())
	}

	/// Updates the allowed sources list of a [JobRegistration].
	fn update_allowed_sources_hook(
		_who: &T::AccountId,
		job_id: &JobId<T::AccountId>,
		_updates: &[AllowedSourcesUpdate<T::AccountId>],
	) -> DispatchResultWithPostInfo {
		let job_status =
			<StoredJobStatus<T>>::get(&job_id.0, job_id.1).ok_or(Error::<T>::JobStatusNotFound)?;

		let registration = <StoredJobRegistration<T>>::get(&job_id.0, job_id.1)
			.ok_or(pallet_acurast::Error::<T>::JobRegistrationNotFound)?;
		let e: <T as Config>::RegistrationExtra = registration.extra.clone().into();
		let requirements: JobRequirementsFor<T> = e.into();

		if let AssignmentStrategy::Single(_) = requirements.assignment_strategy {
			ensure!(job_status == JobStatus::Open, Error::<T>::JobRegistrationUnmodifiable);
		}

		Ok(().into())
	}
}
