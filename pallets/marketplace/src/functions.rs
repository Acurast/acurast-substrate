use frame_support::{
	ensure, pallet_prelude::DispatchResult, sp_runtime::DispatchError, traits::IsSubType,
};
use pallet_acurast::{
	utils::ensure_source_verified, IsFundableCall, JobId, JobRegistrationFor, StoredJobRegistration,
};
use reputation::{BetaParameters, BetaReputation, ReputationEngine};
use sp_core::Get;
use sp_std::prelude::*;

use crate::{
	AdvertisementFor, AdvertisementRestriction, AssignedProcessors, AssignmentFor, Call, Config,
	Error, ExecutionSpecifier, NextReportIndex, Pallet, RewardManager, StoredAdvertisementPricing,
	StoredAdvertisementRestriction, StoredAverageRewardV3, StoredMatches, StoredReputation,
};

impl<T: Config> Pallet<T> {
	pub fn do_advertise(
		processor: &T::AccountId,
		advertisement: &AdvertisementFor<T>,
	) -> DispatchResult {
		if let Some(allowed_consumers) = &advertisement.allowed_consumers {
			let max_allowed_consumers_len = T::MaxAllowedConsumers::get() as usize;
			ensure!(allowed_consumers.len() > 0, Error::<T>::TooFewAllowedConsumers);
			ensure!(
				allowed_consumers.len() <= max_allowed_consumers_len,
				Error::<T>::TooManyAllowedConsumers
			);
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
		let assignment = Self::update_assignment(processor, job_id)?;

		let registration = <StoredJobRegistration<T>>::get(&job_id.0, job_id.1)
			.ok_or(pallet_acurast::Error::<T>::JobRegistrationNotFound)?;

		let (missing_reports, next_expected_report_index) =
			Self::update_next_report_index_on_report(
				job_id,
				processor,
				&registration,
				&assignment,
			)?;

		T::RewardManager::handle_reward(job_id, assignment.fee_per_execution)?;

		Self::do_update_reputation(processor, &assignment, missing_reports)?;

		// if this is the last report, do cleanup
		if next_expected_report_index.is_none() {
			<StoredMatches<T>>::remove(processor, job_id);
			<AssignedProcessors<T>>::remove(job_id, processor);
		}

		Ok(assignment)
	}

	fn update_next_report_index_on_report(
		job_id: &JobId<T::AccountId>,
		processor: &T::AccountId,
		registration: &JobRegistrationFor<T>,
		assignment: &AssignmentFor<T>,
	) -> Result<(u64, Option<u64>), DispatchError> {
		let now = Self::now()?;
		let execution_index = registration
			.schedule
			.current_execution_index(assignment.start_delay, now)
			.unwrap_or(0);
		<NextReportIndex<T>>::try_mutate_exists(job_id, processor, |value| {
			let mut missing_reports = 0;
			let mut expected_report_index = (*value).unwrap_or(execution_index);
			// check if we already reported this execution.
			ensure!(execution_index >= expected_report_index, Error::<T>::ExecutionAlreadyReported);
			let is_report_timely =
				Self::check_report_is_timely(registration, assignment, now, expected_report_index)
					.is_ok();

			if !is_report_timely && expected_report_index != execution_index {
				Self::check_report_is_timely(registration, assignment, now, execution_index)?;
				missing_reports = execution_index.saturating_sub(expected_report_index);
				expected_report_index = execution_index;
			}

			match assignment.execution {
				ExecutionSpecifier::All => {
					let next_expected_report_index = expected_report_index + 1;

					*value = if next_expected_report_index < assignment.sla.total {
						Some(next_expected_report_index)
					} else {
						None
					};
				},
				ExecutionSpecifier::Index(index) => {
					*value = if expected_report_index != index { Some(index) } else { None };
				},
			}

			Ok::<_, DispatchError>((missing_reports, *value))
		})
	}

	pub(crate) fn check_report_is_timely(
		registration: &JobRegistrationFor<T>,
		assignment: &AssignmentFor<T>,
		now: u64,
		execution_index: u64,
	) -> Result<(), DispatchError> {
		let execution_start_time =
			registration.schedule.nth_start_time(assignment.start_delay, execution_index);

		if execution_start_time.is_none() {
			return Err(Error::<T>::ReportOutsideSchedule.into());
		}

		let execution_start_time = execution_start_time.unwrap();
		ensure!(now >= execution_start_time, Error::<T>::ReportOutsideSchedule);
		let report_max_time = execution_start_time
			.saturating_add(registration.schedule.duration)
			.saturating_add(T::ReportTolerance::get());
		ensure!(now < report_max_time, Error::<T>::ReportOutsideSchedule);
		Ok(())
	}

	fn update_assignment(
		processor: &T::AccountId,
		job_id: &JobId<T::AccountId>,
	) -> Result<AssignmentFor<T>, DispatchError> {
		Ok(<StoredMatches<T>>::try_mutate(
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
		)?)
	}

	pub(crate) fn do_update_reputation(
		processor: &T::AccountId,
		assignment: &AssignmentFor<T>,
		missing_reports: u64,
	) -> Result<(), DispatchError> {
		if ensure_source_verified::<T>(processor).is_ok() {
			// skip reputation update if reward is 0
			if assignment.fee_per_execution > 0u8.into() {
				let average_reward = <StoredAverageRewardV3<T>>::get().unwrap_or_default();

				let mut beta_params =
					<StoredReputation<T>>::get(processor).ok_or(Error::<T>::ReputationNotFound)?;

				beta_params = BetaReputation::update(
					beta_params,
					assignment.sla.met,
					missing_reports,
					assignment.fee_per_execution,
					average_reward.into(),
				)
				.ok_or(Error::<T>::CalculationOverflow)?;

				<StoredReputation<T>>::insert(
					processor,
					BetaParameters { r: beta_params.r, s: beta_params.s },
				);
			}
		}
		Ok(())
	}

	pub(crate) fn do_cleanup_assignments(
		processor: &T::AccountId,
		job_ids: &[JobId<T::AccountId>],
	) -> DispatchResult {
		let now = Self::now()?;
		for job_id in job_ids {
			Self::do_cleanup_assignment(processor, job_id, now)?;
		}
		Ok(())
	}

	pub(crate) fn do_cleanup_assignment(
		processor: &T::AccountId,
		job_id: &JobId<T::AccountId>,
		now: u64,
	) -> DispatchResult {
		let Some(assignment) = <StoredMatches<T>>::get(processor, job_id) else {
			return Ok(());
		};
		let Some(job) = <StoredJobRegistration<T>>::get(&job_id.0, job_id.1) else {
			<StoredMatches<T>>::remove(processor, job_id);
			<AssignedProcessors<T>>::remove(job_id, processor);
			return Ok(());
		};
		if assignment.is_invalid(&job.schedule, now, T::ReportTolerance::get()) {
			<StoredMatches<T>>::remove(processor, job_id);
			<AssignedProcessors<T>>::remove(job_id, processor);
		}
		Ok(())
	}
}

impl<T: Config> IsFundableCall<T::RuntimeCall> for Pallet<T>
where
	T::RuntimeCall: IsSubType<Call<T>>,
{
	fn is_fundable_call(call: &T::RuntimeCall) -> bool {
		let Some(call) = T::RuntimeCall::is_sub_type(call) else {
			return false;
		};
		matches!(
			call,
			Call::advertise { .. }
				| Call::acknowledge_match { .. }
				| Call::acknowledge_execution_match { .. }
				| Call::report { .. }
				| Call::cleanup_assignments { .. }
		)
	}
}
