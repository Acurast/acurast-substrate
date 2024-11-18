use frame_support::{
	ensure,
	pallet_prelude::*,
	sp_runtime::{
		traits::{CheckedAdd, CheckedMul, CheckedSub},
		DispatchError, Permill, SaturatedConversion,
	},
	traits::UnixTime,
};
use itertools::Itertools;
use pallet_acurast::{
	utils::{ensure_source_verified, ensure_source_verified_and_of_type},
	JobId, JobRegistrationFor, ProcessorType, Schedule, StoredJobRegistration,
};
use reputation::{BetaReputation, ReputationEngine};

use crate::{
	utils::{is_consumer_allowed, is_processor_allowed},
	*,
};

pub type MatchingResult<T> = Result<
	Vec<(JobId<<T as frame_system::Config>::AccountId>, <T as Config>::Balance)>,
	DispatchError,
>;

impl<T: Config> Pallet<T> {
	/// Checks if a Processor - Job match is possible and returns the remaining job rewards by `job_id`.
	///
	/// If the job is no longer in status [`JobStatus::Open`], the matching is skipped without returning an error.
	/// **The returned vector does not include an entry for skipped matches.**
	///
	/// Every other invalidity in a provided [`Match`] fails the entire call.
	pub(crate) fn process_matching<'a>(
		matching: impl IntoIterator<Item = &'a MatchFor<T>>,
	) -> MatchingResult<T> {
		let mut remaining_rewards: Vec<(JobId<T::AccountId>, T::Balance)> = Default::default();

		for m in matching {
			let job_status = <StoredJobStatus<T>>::get(&m.job_id.0, m.job_id.1)
				.ok_or(Error::<T>::JobStatusNotFound)?;

			if job_status != JobStatus::Open {
				// skip but don't fail this match (another matcher was quicker)
				continue;
			}

			let registration = <StoredJobRegistration<T>>::get(&m.job_id.0, m.job_id.1)
				.ok_or(pallet_acurast::Error::<T>::JobRegistrationNotFound)?;
			let e: <T as Config>::RegistrationExtra = registration.extra.clone().into();
			let requirements: JobRequirementsFor<T> = e.into();

			let now = Self::now()?;

			// CHECK that execution matching happens not after start time
			ensure!(
				// no correction of now is needed since update delay of `now` can make this check being false in real time
				// but only if the propose_matching happens in same block as when time was still in range, which is acceptable
				// assuming the cases where the processor can no longer acknowledge in time are rare.
				now < registration.schedule.start_time,
				Error::<T>::OverdueMatch
			);

			let l: u8 = m.sources.len().try_into().unwrap_or(0);
			ensure!(
				// NOTE: we are checking for duplicates while inserting/mutating StoredMatches below
				l == requirements.slots,
				Error::<T>::IncorrectSourceCountInMatch
			);

			let reward_amount: <T as Config>::Balance = requirements.reward;

			// keep track of total fee in assignments to check later if it exceeds reward
			let mut total_fee: <T as Config>::Balance = 0u8.into();

			// `slot` is used for detecting duplicate source proposed for distinct slots
			// TODO: add global (configurable) maximum of jobs assigned. This would limit the weight of `propose_matching` to a constant, since it depends on the number of active matches.
			for (slot, planned_execution) in m.sources.iter().enumerate() {
				// CHECK attestation
				ensure!(
					!registration.allow_only_verified_sources
						|| ensure_source_verified_and_of_type::<T>(
							&planned_execution.source,
							ProcessorType::Core
						)
						.is_ok(),
					Error::<T>::UnverifiedSourceInMatch
				);

				let ad = <StoredAdvertisementRestriction<T>>::get(&planned_execution.source)
					.ok_or(Error::<T>::AdvertisementNotFound)?;

				for required_module in &registration.required_modules {
					ensure!(
						ad.available_modules.contains(required_module),
						Error::<T>::ModuleNotAvailableInMatch
					);
				}

				let pricing = <StoredAdvertisementPricing<T>>::get(&planned_execution.source)
					.ok_or(Error::<T>::AdvertisementPricingNotFound)?;

				// CHECK the scheduling_window allow to schedule this job
				Self::check_scheduling_window(
					&pricing.scheduling_window,
					&registration.schedule,
					now,
					planned_execution.start_delay,
				)?;

				// CHECK memory sufficient
				ensure!(ad.max_memory >= registration.memory, Error::<T>::MaxMemoryExceededInMatch);

				// CHECK network request quota sufficient
				Self::check_network_request_quota_sufficient(
					&ad,
					&registration.schedule,
					registration.network_requests,
				)?;

				// CHECK remaining storage capacity sufficient and lock if check succeeds
				<Pallet<T> as StorageTracker<T>>::lock(&planned_execution.source, &registration)?;

				// CHECK source is whitelisted
				ensure!(
					is_processor_allowed::<T>(
						&planned_execution.source,
						&registration.allowed_sources
					),
					Error::<T>::SourceNotAllowedInMatch
				);

				// CHECK consumer is whitelisted
				ensure!(
					is_consumer_allowed::<T>(&m.job_id.0, &ad.allowed_consumers),
					Error::<T>::ConsumerNotAllowedInMatch
				);

				// CHECK reputation sufficient
				Self::check_min_reputation(requirements.min_reputation, &planned_execution.source)?;

				Self::check_processor_version(
					&requirements.processor_version,
					&planned_execution.source,
				)?;

				// CHECK schedule
				Self::fits_schedule(
					&planned_execution.source,
					ExecutionSpecifier::All,
					&registration.schedule,
					planned_execution.start_delay,
				)?;

				// calculate fee
				let fee_per_execution = Self::fee_per_execution(
					&registration.schedule,
					registration.storage,
					&pricing,
				)?;

				// CHECK price not exceeding reward
				ensure!(fee_per_execution <= reward_amount, Error::<T>::InsufficientRewardInMatch);

				let execution_count = registration.schedule.execution_count();

				total_fee = total_fee
					.checked_add(
						&fee_per_execution
							.checked_mul(&execution_count.into())
							.ok_or(Error::<T>::CalculationOverflow)?,
					)
					.ok_or(Error::<T>::CalculationOverflow)?;

				// ASSIGN if not yet assigned (equals to CHECK that no duplicate source in a single mutate operation)
				<StoredMatches<T>>::try_mutate(
					&planned_execution.source,
					&m.job_id,
					|s| -> Result<(), Error<T>> {
						// NOTE: the None case is the "good case", used when there is *no entry yet and thus no duplicate assignment so far*.
						match s {
							Some(_) => Err(Error::<T>::DuplicateSourceInMatch),
							None => {
								*s = Some(Assignment {
									slot: slot as u8,
									execution: ExecutionSpecifier::All,
									start_delay: planned_execution.start_delay,
									fee_per_execution,
									acknowledged: false,
									sla: SLA { total: execution_count, met: 0 },
									pub_keys: PubKeys::default(),
								});
								Ok(())
							},
						}?;
						Ok(())
					},
				)?;
				<AssignedProcessors<T>>::insert(&m.job_id, &planned_execution.source, ());
			}

			// CHECK total fee is not exceeding reward
			let total_reward_amount = Self::total_reward_amount(&registration)?;
			let diff = total_reward_amount
				.checked_sub(&total_fee)
				.ok_or(Error::<T>::InsufficientRewardInMatch)?;
			// We better check for diff positive <=> total_fee <= total_reward_amount
			// because we cannot assume that asset amount is an unsigned integer for all future
			ensure!(diff >= 0u32.into(), Error::<T>::InsufficientRewardInMatch);

			remaining_rewards.push((m.job_id.clone(), diff));

			<StoredTotalAssignedV3<T>>::mutate(|t| {
				*t = Some(t.unwrap_or(0u128).saturating_add(1));
			});

			<StoredJobStatus<T>>::insert(&m.job_id.0, m.job_id.1, JobStatus::Matched);
			Self::deposit_event(Event::JobRegistrationMatched(m.clone()));
		}
		Ok(remaining_rewards)
	}

	pub(crate) fn process_execution_matching<'a>(
		matching: impl IntoIterator<Item = &'a ExecutionMatchFor<T>>,
	) -> MatchingResult<T> {
		let mut remaining_rewards: Vec<(JobId<T::AccountId>, T::Balance)> = Default::default();

		for m in matching {
			// if the job_execution_status was never set, the default `Open` is returned
			if <StoredJobExecutionStatus<T>>::get(&m.job_id, m.execution_index) != JobStatus::Open {
				// skip but don't fail this match (another matcher was quicker)
				continue;
			}

			let registration = <StoredJobRegistration<T>>::get(&m.job_id.0, m.job_id.1)
				.ok_or(pallet_acurast::Error::<T>::JobRegistrationNotFound)?;
			let e: <T as Config>::RegistrationExtra = registration.extra.clone().into();
			let requirements: JobRequirementsFor<T> = e.into();

			let now = Self::now()?;

			let nth_start_time_lower_bound = registration
				.schedule
				// we do not enforce a later matching time for a start_delay >= 0 earliest
				.nth_start_time(0u64, m.execution_index)
				.ok_or(Error::<T>::IncorrectExecutionIndex)?;

			// CHECK that execution matching happens not after start time
			ensure!(
				// no correction of now is needed since update delay of `now` can make this check being false in real time
				// but only if the propose_matching happens in same block as when time was still in range, which is acceptable
				// assuming the cases where the processor can no longer acknowledge in time are rare.
				now < nth_start_time_lower_bound,
				Error::<T>::OverdueMatch
			);

			// CHECK that execution matching happens shortly before execution
			ensure!(
				// and no correction of now is needed since update delay of `now` only makes the delta more generous
				now >= nth_start_time_lower_bound
					.saturating_sub(<T as Config>::MatchingCompetingDueDelta::get()),
				Error::<T>::UnderdueMatch
			);

			let l: u8 = m.sources.len().try_into().unwrap_or(0);
			ensure!(
				// NOTE: we are checking for duplicates while inserting/mutating StoredMatches below
				l == requirements.slots,
				Error::<T>::IncorrectSourceCountInMatch
			);

			let reward_amount: <T as Config>::Balance = requirements.reward;

			// keep track of total fee in assignments to check later if it exceeds reward
			let mut total_fee: <T as Config>::Balance = 0u8.into();

			// `slot` is used for detecting duplicate source proposed for distinct slots
			// TODO: add global (configurable) maximum of jobs assigned. This would limit the weight of `propose_execution_matching` to a constant, since it depends on the number of active matches.
			for (slot, planned_execution) in m.sources.iter().enumerate() {
				// CHECK attestation
				ensure!(
					!registration.allow_only_verified_sources
						|| ensure_source_verified::<T>(&planned_execution.source).is_ok(),
					Error::<T>::UnverifiedSourceInMatch
				);

				let ad = <StoredAdvertisementRestriction<T>>::get(&planned_execution.source)
					.ok_or(Error::<T>::AdvertisementNotFound)?;

				for required_module in &registration.required_modules {
					ensure!(
						ad.available_modules.contains(required_module),
						Error::<T>::ModuleNotAvailableInMatch
					);
				}

				let pricing = <StoredAdvertisementPricing<T>>::get(&planned_execution.source)
					.ok_or(Error::<T>::AdvertisementPricingNotFound)?;

				// CHECK the scheduling_window allow to schedule this job
				Self::check_scheduling_window(
					&pricing.scheduling_window,
					&registration.schedule,
					now,
					planned_execution.start_delay,
				)?;

				// CHECK memory sufficient
				ensure!(ad.max_memory >= registration.memory, Error::<T>::MaxMemoryExceededInMatch);

				// CHECK network request quota sufficient
				Self::check_network_request_quota_sufficient(
					&ad,
					&registration.schedule,
					registration.network_requests,
				)?;

				// CHECK remaining storage capacity sufficient and lock if check succeeds
				<Pallet<T> as StorageTracker<T>>::lock(&planned_execution.source, &registration)?;

				// CHECK source is whitelisted
				ensure!(
					is_processor_allowed::<T>(
						&planned_execution.source,
						&registration.allowed_sources
					),
					Error::<T>::SourceNotAllowedInMatch
				);

				// CHECK consumer is whitelisted
				ensure!(
					is_consumer_allowed::<T>(&m.job_id.0, &ad.allowed_consumers),
					Error::<T>::ConsumerNotAllowedInMatch
				);

				// CHECK reputation sufficient
				Self::check_min_reputation(requirements.min_reputation, &planned_execution.source)?;

				Self::check_processor_version(
					&requirements.processor_version,
					&planned_execution.source,
				)?;

				// CHECK schedule
				Self::fits_schedule(
					&planned_execution.source,
					ExecutionSpecifier::Index(m.execution_index),
					&registration.schedule,
					planned_execution.start_delay,
				)?;

				// calculate fee
				let fee_per_execution = Self::fee_per_execution(
					&registration.schedule,
					registration.storage,
					&pricing,
				)?;

				// CHECK price not exceeding reward
				ensure!(fee_per_execution <= reward_amount, Error::<T>::InsufficientRewardInMatch);

				let execution_count = registration.schedule.execution_count();

				total_fee = total_fee
					.checked_add(
						&fee_per_execution
							.checked_mul(&execution_count.into())
							.ok_or(Error::<T>::CalculationOverflow)?,
					)
					.ok_or(Error::<T>::CalculationOverflow)?;

				// ASSIGN if not yet assigned (equals to CHECK that no duplicate source in a single mutate operation)
				<StoredMatches<T>>::try_mutate(
					&planned_execution.source,
					&m.job_id,
					|s| -> Result<(), Error<T>> {
						match s {
							Some(prev_assignment) => {
								if let ExecutionSpecifier::Index(e) = prev_assignment.execution {
									if e == m.execution_index {
										return Err(Error::<T>::DuplicateSourceInMatch);
									}
								}
								*s = Some(Assignment {
									slot: slot as u8,
									execution: ExecutionSpecifier::Index(m.execution_index),
									start_delay: planned_execution.start_delay,
									fee_per_execution,
									acknowledged: false,
									// increment total executions expected
									sla: SLA {
										total: prev_assignment.sla.total.saturating_add(1),
										met: prev_assignment.sla.met,
									},
									pub_keys: PubKeys::default(),
								});
								Ok::<(), Error<T>>(())
							},
							// NOTE: the None case is the "good case", used when there is *no entry yet and thus no duplicate assignment so far*.
							None => {
								*s = Some(Assignment {
									slot: slot as u8,
									execution: ExecutionSpecifier::Index(m.execution_index),
									start_delay: planned_execution.start_delay,
									fee_per_execution,
									acknowledged: false,
									// we start with one total executions expected and increment with every future match
									sla: SLA { total: 1, met: 0 },
									pub_keys: PubKeys::default(),
								});
								Ok(())
							},
						}?;
						Ok(())
					},
				)?;
				<AssignedProcessors<T>>::insert(&m.job_id, &planned_execution.source, ());
			}

			// CHECK total fee is not exceeding reward
			let total_reward_amount = Self::total_reward_amount(&registration)?;
			let diff = total_reward_amount
				.checked_sub(&total_fee)
				.ok_or(Error::<T>::InsufficientRewardInMatch)?;
			// We better check for diff positive <=> total_fee <= total_reward_amount
			// because we cannot assume that asset amount is an unsigned integer for all future
			ensure!(diff >= 0u32.into(), Error::<T>::InsufficientRewardInMatch);

			remaining_rewards.push((m.job_id.clone(), diff));

			// only update average on first execution's match to not have the average proportionally influenced by singe executions that get matched
			if m.execution_index == 0 {
				<StoredTotalAssignedV3<T>>::mutate(|t| {
					*t = Some(t.unwrap_or(0u128).saturating_add(1));
				});
			}

			<StoredJobStatus<T>>::insert(&m.job_id.0, m.job_id.1, JobStatus::Matched);
			<StoredJobExecutionStatus<T>>::insert(&m.job_id, m.execution_index, JobStatus::Matched);

			Self::deposit_event(Event::JobExecutionMatched(m.clone()));
		}
		Ok(remaining_rewards)
	}

	fn check_scheduling_window(
		scheduling_window: &SchedulingWindow,
		schedule: &Schedule,
		now: u64,
		start_delay: u64,
	) -> Result<(), Error<T>> {
		match scheduling_window {
			SchedulingWindow::End(end) => {
				ensure!(
					*end >= schedule
						.end_time
						.checked_add(start_delay)
						.ok_or(Error::<T>::CalculationOverflow)?,
					Error::<T>::SchedulingWindowExceededInMatch
				);
			},
			SchedulingWindow::Delta(delta) => {
				ensure!(
					now.checked_add(*delta).ok_or(Error::<T>::CalculationOverflow)?
						>= schedule
							.end_time
							.checked_add(start_delay)
							.ok_or(Error::<T>::CalculationOverflow)?,
					Error::<T>::SchedulingWindowExceededInMatch
				);
			},
		}

		Ok(())
	}

	fn check_network_request_quota_sufficient(
		ad: &AdvertisementRestriction<T::AccountId, T::MaxAllowedConsumers>,
		schedule: &Schedule,
		network_requests: u32,
	) -> Result<(), Error<T>> {
		// CHECK network request quota sufficient
		ensure!(
			// duration (s) * network_request_quota >= network_requests (per second)
			// <=>
			// duration (ms) / 1000 * network_request_quota >= network_requests (per second)
			// <=>
			// duration (ms) * network_request_quota >= network_requests (per second) * 1000
			schedule.duration.checked_mul(ad.network_request_quota.into()).unwrap_or(0u64)
				>= network_requests
					.saturated_into::<u64>()
					.checked_mul(1000u64)
					.unwrap_or(u64::MAX),
			Error::<T>::NetworkRequestQuotaExceededInMatch
		);
		Ok(())
	}

	fn check_min_reputation(
		min_reputation: Option<u128>,
		source: &T::AccountId,
	) -> Result<(), Error<T>> {
		if let Some(min_reputation) = min_reputation {
			let beta_params =
				<StoredReputation<T>>::get(source).ok_or(Error::<T>::ReputationNotFound)?;

			let reputation = BetaReputation::<u128>::normalize(beta_params)
				.ok_or(Error::<T>::CalculationOverflow)?;

			ensure!(
				reputation >= Permill::from_parts(min_reputation as u32),
				Error::<T>::InsufficientReputationInMatch
			);
		}
		Ok(())
	}

	fn check_processor_version(
		required_processor_version: &Option<
			ProcessorVersionRequirements<T::ProcessorVersion, T::MaxVersions>,
		>,
		source: &T::AccountId,
	) -> Result<(), Error<T>> {
		if let Some(version_req) = required_processor_version {
			if let Some(version) = T::ProcessorInfoProvider::processor_version(source) {
				let matches_version = match version_req {
					ProcessorVersionRequirements::Min(versions) => {
						versions.iter().any(|req_version| &version >= req_version)
					},
				};
				if !matches_version {
					return Err(Error::<T>::ProcessorVersionMismatch);
				}
			} else {
				return Err(Error::<T>::ProcessorVersionMismatch);
			}
		}
		Ok(())
	}

	/// Filters the given `sources` by those recently seen and matching partially specified `registration`
	/// and whitelisting `consumer` if specifying a whitelist.
	///
	/// Intended to be called for providing runtime API, might return corresponding error.
	pub fn filter_matching_sources(
		registration: PartialJobRegistration<T::Balance, T::AccountId, T::MaxAllowedSources>,
		sources: Vec<T::AccountId>,
		consumer: Option<MultiOrigin<T::AccountId>>,
		latest_seen_after: Option<u128>,
	) -> Result<Vec<T::AccountId>, RuntimeApiError> {
		let mut candidates = Vec::new();
		for p in sources {
			let valid_match = match Self::check(&registration, &p, consumer.as_ref()) {
				Ok(()) => {
					if let Some(latest_seen_after) = latest_seen_after {
						T::ProcessorInfoProvider::last_seen(&p)
							.map(|last_seen| last_seen >= latest_seen_after)
							.unwrap_or(false)
					} else {
						true
					}
				},
				Err(e) => {
					if !e.is_matching_error() {
						return Err(RuntimeApiError::FilterMatchingSources);
					}

					false
				},
			};

			if valid_match {
				candidates.push(p);
			}
		}
		Ok(candidates)
	}

	fn check(
		registration: &PartialJobRegistrationForMarketplace<T>,
		source: &T::AccountId,
		consumer: Option<&MultiOrigin<T::AccountId>>,
	) -> Result<(), Error<T>> {
		// CHECK attestation
		ensure!(
			!registration.allow_only_verified_sources
				|| ensure_source_verified::<T>(source).is_ok(),
			Error::<T>::UnverifiedSourceInMatch
		);

		let ad = <StoredAdvertisementRestriction<T>>::get(source)
			.ok_or(Error::<T>::AdvertisementNotFound)?;

		for required_module in &registration.required_modules {
			ensure!(
				ad.available_modules.contains(required_module),
				Error::<T>::ModuleNotAvailableInMatch
			);
		}

		let pricing = <StoredAdvertisementPricing<T>>::get(source)
			.ok_or(Error::<T>::AdvertisementPricingNotFound)?;

		if let Some(schedule) = &registration.schedule {
			let now = Self::now()?;
			ensure!(now < schedule.start_time, Error::<T>::OverdueMatch);

			// CHECK the scheduling_window allow to schedule this job
			Self::check_scheduling_window(&pricing.scheduling_window, schedule, now, 0)?;

			// CHECK schedule
			Self::fits_schedule(source, ExecutionSpecifier::All, schedule, 0)?;

			// CHECK network request quota sufficient
			if let Some(network_requests) = registration.network_requests {
				Self::check_network_request_quota_sufficient(&ad, schedule, network_requests)?;
			}

			// CHECK reward sufficient
			if let Some(storage) = &registration.storage {
				// calculate fee
				let fee_per_execution = Self::fee_per_execution(schedule, *storage, &pricing)?;

				// CHECK price not exceeding reward
				ensure!(
					fee_per_execution <= registration.reward,
					Error::<T>::InsufficientRewardInMatch
				);
			}
		}

		// CHECK memory sufficient
		if let Some(memory) = &registration.memory {
			ensure!(ad.max_memory >= *memory, Error::<T>::MaxMemoryExceededInMatch);
		}

		// CHECK remaining storage capacity sufficient
		<Pallet<T> as StorageTracker<T>>::check(source, registration)?;

		// CHECK source is whitelisted
		ensure!(
			is_processor_allowed::<T>(source, &registration.allowed_sources),
			Error::<T>::SourceNotAllowedInMatch
		);

		// CHECK consumer is whitelisted
		if let Some(consumer) = consumer {
			ensure!(
				is_consumer_allowed::<T>(consumer, &ad.allowed_consumers),
				Error::<T>::ConsumerNotAllowedInMatch
			);
		}

		// CHECK reputation sufficient
		Self::check_min_reputation(registration.min_reputation, source)?;

		Ok(())
	}

	/// Returns true if the source has currently at least one match (not necessarily assigned).
	pub(crate) fn has_matches(source: &T::AccountId) -> bool {
		// NOTE we use a trick to check if map contains *any* secondary key: we use `any` to short-circuit
		// whenever we encounter the first - so at least one - element in the iterator.
		<StoredMatches<T>>::iter_prefix_values(source).any(|_| true)
	}

	/// Checks of a new job schedule fits with the existing schedule for a processor.
	fn fits_schedule(
		source: &T::AccountId,
		execution_specifier: ExecutionSpecifier,
		schedule: &Schedule,
		start_delay: u64,
	) -> Result<(), Error<T>> {
		for (job_id, assignment) in <StoredMatches<T>>::iter_prefix(source) {
			// ignore job registrations not found (shouldn't happen if invariant is kept that assignments are cleared whenever a job is removed)
			// TODO decide tradeoff: we could save this lookup at the cost of storing the schedule along with the match or even completely move it from StoredJobRegistration into StoredMatches
			if let Some(other) = <StoredJobRegistration<T>>::get(&job_id.0, job_id.1) {
				// check if the whole schedule periods have an overlap in worst case scenario for max_start_delay
				if !schedule
					.overlaps(
						start_delay,
						other
							.schedule
							.range(assignment.start_delay)
							.ok_or(Error::<T>::CalculationOverflow)?,
					)
					.ok_or(Error::<T>::CalculationOverflow)?
				{
					// periods don't overlap so no detail (and expensive) checks are necessary
					continue;
				}

				match (execution_specifier, assignment.execution) {
					(ExecutionSpecifier::All, ExecutionSpecifier::All) => {
						let it = schedule
							.iter(start_delay)
							.ok_or(Error::<T>::CalculationOverflow)?
							.map(|start| {
								let end = start.checked_add(schedule.duration)?;
								Some((start, end))
							});

						let other_it = other
							.schedule
							.iter(assignment.start_delay)
							.ok_or(Error::<T>::CalculationOverflow)?
							.map(|start| {
								let end = start.checked_add(other.schedule.duration)?;
								Some((start, end))
							});

						it.merge(other_it).try_fold(0u64, |prev_end, bounds| {
							let (start, end) = bounds.ok_or(Error::<T>::CalculationOverflow)?;

							if prev_end > start {
								Err(Error::<T>::ScheduleOverlapInMatch)
							} else {
								Ok(end)
							}
						})?;
					},
					(ExecutionSpecifier::All, ExecutionSpecifier::Index(other_execution_index)) => {
						let other_start = other
							.schedule
							.nth_start_time(assignment.start_delay, other_execution_index)
							.ok_or(Error::<T>::CalculationOverflow)?;
						let other_end = other_start
							.checked_add(other.schedule.duration)
							.ok_or(Error::<T>::CalculationOverflow)?;

						if schedule
							.overlaps(start_delay, (other_start, other_end))
							.ok_or(Error::<T>::CalculationOverflow)?
						{
							Err(Error::<T>::ScheduleOverlapInMatch)?;
						}
					},
					(ExecutionSpecifier::Index(execution_index), ExecutionSpecifier::All) => {
						let start = schedule
							.nth_start_time(start_delay, execution_index)
							.ok_or(Error::<T>::CalculationOverflow)?;
						let end = start
							.checked_add(schedule.duration)
							.ok_or(Error::<T>::CalculationOverflow)?;

						if other
							.schedule
							.overlaps(start_delay, (start, end))
							.ok_or(Error::<T>::CalculationOverflow)?
						{
							Err(Error::<T>::ScheduleOverlapInMatch)?;
						}
					},
					(
						ExecutionSpecifier::Index(execution_index),
						ExecutionSpecifier::Index(other_execution_index),
					) => {
						let start = schedule
							.nth_start_time(start_delay, execution_index)
							.ok_or(Error::<T>::CalculationOverflow)?;
						let end = start
							.checked_add(schedule.duration)
							.ok_or(Error::<T>::CalculationOverflow)?;
						let other_start = other
							.schedule
							.nth_start_time(assignment.start_delay, other_execution_index)
							.ok_or(Error::<T>::CalculationOverflow)?;
						let other_end = other_start
							.checked_add(other.schedule.duration)
							.ok_or(Error::<T>::CalculationOverflow)?;

						// For a collision we need
						//       ╭overlapping before end
						// ___■■■■______
						// ____ ■■■■____ (other)
						// AND not
						//     ╭ending before start
						// _____■■■■______
						// _■■■■__________ (other)
						if other_start < end && other_end > start {
							Err(Error::<T>::ScheduleOverlapInMatch)?;
						}
					},
				}
			}
		}

		Ok(())
	}

	/// Calculates if the job ended considering the given assignment.
	pub(crate) fn actual_schedule_ended(
		schedule: &Schedule,
		assignment: &AssignmentFor<T>,
	) -> Result<bool, Error<T>> {
		let now = Self::now()?
			.checked_add(T::ReportTolerance::get())
			.ok_or(Error::<T>::CalculationOverflow)?;
		let (_actual_start, actual_end) =
			schedule.range(assignment.start_delay).ok_or(Error::<T>::CalculationOverflow)?;
		Ok(actual_end.lt(&now))
	}

	/// Calculates if the job ended considering the given assignment.
	fn schedule_ended(schedule: &Schedule) -> Result<bool, Error<T>> {
		let now = Self::now()?
			.checked_add(T::ReportTolerance::get())
			.ok_or(Error::<T>::CalculationOverflow)?;
		let (_actual_start, actual_end) = schedule
			.range(schedule.max_start_delay)
			.ok_or(Error::<T>::CalculationOverflow)?;
		Ok(actual_end.lt(&now))
	}

	/// Calculates the total reward amount.
	pub(crate) fn total_reward_amount(
		registration: &JobRegistrationFor<T>,
	) -> Result<T::Balance, Error<T>> {
		let e: <T as Config>::RegistrationExtra = registration.extra.clone().into();
		let requirements: JobRequirementsFor<T> = e.into();

		requirements
			.reward
			.checked_mul(&((requirements.slots as u128).into()))
			.ok_or(Error::<T>::CalculationOverflow)?
			.checked_mul(&registration.schedule.execution_count().into())
			.ok_or(Error::<T>::CalculationOverflow)
	}

	/// Calculates the fee per job execution.
	fn fee_per_execution(
		schedule: &Schedule,
		storage: u32,
		pricing: &PricingFor<T>,
	) -> Result<T::Balance, Error<T>> {
		pricing
			.fee_per_millisecond
			.checked_mul(&schedule.duration.into())
			.ok_or(Error::<T>::CalculationOverflow)?
			.checked_add(
				&pricing
					.fee_per_storage_byte
					.clone()
					.checked_mul(&storage.into())
					.ok_or(Error::<T>::CalculationOverflow)?,
			)
			.ok_or(Error::<T>::CalculationOverflow)?
			.checked_add(&pricing.base_fee_per_execution)
			.ok_or(Error::<T>::CalculationOverflow)
	}

	/// Finalizes jobs and get refunds unused rewards.
	///
	/// It assumes the caller was already authorized and is intended to be used from
	/// * The [`Self::finalize_jobs`] extrinsic of this pallet
	/// * An inter-chain communication protocol like Hyperdrive
	///
	/// Only valid if for all given jobs provided,
	///
	/// * the job was **not** acknowledged by any processor (job is in state [`JobStatus::Matched`]) OR
	/// * the job was acknowledged by **at least one** processor (job is in state [`JobStatus::Assigned`]) AND
	///   * all processors have finalized their corresponding slot OR
	///   * the latest possible reporting time has passed
	///
	/// If the call proceeds, it cleans up the remaining storage entries related to the finalized jobs.
	pub fn finalize_jobs_for(
		job_ids: impl IntoIterator<Item = JobId<T::AccountId>>,
	) -> DispatchResultWithPostInfo {
		for job_id in job_ids {
			let registration = <StoredJobRegistration<T>>::get(&job_id.0, job_id.1)
				.ok_or(pallet_acurast::Error::<T>::JobRegistrationNotFound)?;
			let e: <T as Config>::RegistrationExtra = registration.extra.clone().into();
			let requirements: JobRequirementsFor<T> = e.into();

			match requirements.assignment_strategy {
				AssignmentStrategy::Single(_) => {
					let job_status = <StoredJobStatus<T>>::get(&job_id.0, job_id.1)
						.ok_or(Error::<T>::JobStatusNotFound)?;
					match job_status {
						JobStatus::Open => Err(Error::<T>::CannotFinalizeJob(job_status))?,
						JobStatus::Matched => {
							let match_overdue = Self::now()? >= registration.schedule.start_time;
							if !match_overdue {
								Err(Error::<T>::CannotFinalizeJob(job_status))?;
							}
						},
						JobStatus::Assigned(_) => {
							// in the "good case" when all processors finalized their slot we can accept the finalization independent of schedule's latest end
							let some_assigned =
								<AssignedProcessors<T>>::iter_prefix(&job_id).next().is_some();
							if some_assigned && !Self::schedule_ended(&registration.schedule)? {
								Err(Error::<T>::CannotFinalizeJob(job_status))?;
							}
						},
					}
				},
				AssignmentStrategy::Competing => {
					let last_execution_index =
						registration.schedule.execution_count().saturating_sub(1);

					let last_match_overdue = Self::now()?
						>= registration
							.schedule
							.nth_start_time(
								registration.schedule.max_start_delay,
								last_execution_index,
							)
							.unwrap();

					// check last execution's status
					let job_status =
						<StoredJobExecutionStatus<T>>::get(&job_id, last_execution_index);
					match job_status {
						JobStatus::Open | JobStatus::Matched => {
							if !last_match_overdue {
								Err(Error::<T>::CannotFinalizeJob(job_status))?;
							}
						},
						JobStatus::Assigned(_) => {
							// in the "good case" when all processors finalized their slot we can accept the finalization independent of schedule's latest end
							let some_assigned =
								<AssignedProcessors<T>>::iter_prefix(&job_id).next().is_some();
							if some_assigned && !Self::schedule_ended(&registration.schedule)? {
								Err(Error::<T>::CannotFinalizeJob(job_status))?;
							}
						},
					};
				},
			}

			// removed completed job from remaining storage points
			for (p, _) in <AssignedProcessors<T>>::iter_prefix(&job_id) {
				<StoredMatches<T>>::remove(&p, &job_id);

				<Pallet<T> as StorageTracker<T>>::unlock(&p, &registration)?;
			}
			let _ = <AssignedProcessors<T>>::clear_prefix(
				&job_id,
				<T as pallet_acurast::Config>::MaxSlots::get(),
				None,
			);

			T::MarketplaceHooks::finalize_job(&job_id, T::RewardManager::refund(&job_id)?)?;

			pallet_acurast::Pallet::<T>::clear_environment_for(&job_id);
			<StoredJobStatus<T>>::remove(&job_id.0, job_id.1);
			let _ = <StoredJobExecutionStatus<T>>::clear_prefix(
				&job_id,
				registration.schedule.execution_count() as u32,
				None,
			);
			<StoredJobRegistration<T>>::remove(&job_id.0, job_id.1);

			Self::deposit_event(Event::JobFinalized(job_id.clone()));
		}

		Ok(().into())
	}

	/// Returns the stored matches for a source.
	///
	/// Intended to be called for providing runtime API, might return corresponding error.
	pub fn stored_matches_for_source(
		source: T::AccountId,
	) -> Result<Vec<JobAssignmentFor<T>>, RuntimeApiError> {
		<StoredMatches<T>>::iter_prefix(source)
			.map(|(job_id, assignment)| {
				let job = <StoredJobRegistration<T>>::get(&job_id.0, job_id.1)
					.ok_or(RuntimeApiError::MatchedJobs)?;
				Ok(JobAssignment { job_id, job, assignment })
			})
			.collect()
	}

	pub fn process_acknowledge_match(
		who: T::AccountId,
		job_id: JobId<T::AccountId>,
		// makes this extrinsic idempotent: if execution is not the current one stored in StoredMatches for the acknowledging source, this call will fail
		execution: ExecutionSpecifier,
		pub_keys: PubKeys,
	) -> DispatchResultWithPostInfo {
		let (changed, assignment) = <StoredMatches<T>>::try_mutate(
			&who,
			&job_id,
			|m| -> Result<(bool, AssignmentFor<T>), Error<T>> {
				// CHECK that job was matched previously to calling source
				let assignment = m.as_mut().ok_or(Error::<T>::CannotAcknowledgeWhenNotMatched)?;

				// CHECK that acknowledge is for the current execution (for idempotency of this extrinsic)
				ensure!(
					assignment.execution == execution,
					Error::<T>::CannotAcknowledgeForOtherThanCurrentExecution
				);

				let changed = !assignment.acknowledged;
				assignment.acknowledged = true;
				assignment.pub_keys = pub_keys;
				Ok((changed, assignment.to_owned()))
			},
		)?;

		if changed {
			match execution {
				ExecutionSpecifier::All => {
					<StoredJobStatus<T>>::try_mutate(
						&job_id.0,
						job_id.1,
						|s| -> Result<(), Error<T>> {
							let status = s.ok_or(Error::<T>::JobStatusNotFound)?;
							*s = Some(match status {
								JobStatus::Open => {
									Err(Error::<T>::CannotAcknowledgeWhenNotMatched)?
								},
								JobStatus::Matched => JobStatus::Assigned(1),
								JobStatus::Assigned(count) => JobStatus::Assigned(count + 1),
							});

							Ok(())
						},
					)?;
				},
				ExecutionSpecifier::Index(execution_index) => {
					let new_status = <StoredJobExecutionStatus<T>>::try_mutate(
						&job_id,
						execution_index,
						|status| -> Result<JobStatus, Error<T>> {
							*status = match status {
								JobStatus::Open => {
									Err(Error::<T>::CannotAcknowledgeWhenNotMatched)?
								},
								JobStatus::Matched => JobStatus::Assigned(1),
								JobStatus::Assigned(count) => JobStatus::Assigned(*count + 1),
							};

							Ok(*status)
						},
					)?;
					// reflect latest execution's status in StoredJobStatus for completeness
					<StoredJobStatus<T>>::insert(&job_id.0, job_id.1, new_status);
				},
			}

			// activate hook so implementing side can react on job assignment
			T::MarketplaceHooks::assign_job(&job_id, &assignment.pub_keys)?;

			Self::deposit_event(Event::JobRegistrationAssigned(job_id, who, assignment.clone()));
		}
		Ok(().into())
	}

	/// Returns the current timestamp.
	pub fn now() -> Result<u64, Error<T>> {
		Ok(<T as pallet_acurast::Config>::UnixTime::now()
			.as_millis()
			.try_into()
			.map_err(|_| pallet_acurast::Error::<T>::FailedTimestampConversion)?)
	}
}
