#![cfg_attr(not(feature = "std"), no_std)]

pub use functions::*;
pub use pallet::*;
pub use payments::*;
pub use traits::*;
pub use types::*;

#[cfg(test)]
pub mod mock;
#[cfg(any(test, feature = "runtime-benchmarks"))]
mod stub;
#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

mod functions;
mod migration;
pub mod payments;
#[cfg(feature = "std")]
pub mod rpc;
pub mod traits;
pub mod types;
mod utils;
pub mod weights;

pub(crate) use pallet::STORAGE_VERSION;

use pallet_acurast::{Attestation, Environment, JobId, MultiOrigin, ParameterBound};
use sp_std::prelude::*;

#[cfg(feature = "runtime-benchmarks")]
pub use benchmarking::BenchmarkHelper;

#[frame_support::pallet]
pub mod pallet {
    use frame_support::sp_runtime::traits::{CheckedAdd, CheckedDiv, CheckedMul, CheckedSub};
    use frame_support::sp_runtime::{FixedPointOperand, FixedU128, Permill, SaturatedConversion};
    use frame_support::traits::tokens::Balance;
    use frame_support::{
        dispatch::DispatchResultWithPostInfo, ensure, pallet_prelude::*, traits::UnixTime,
        Blake2_128, Blake2_128Concat, PalletId,
    };
    use frame_system::pallet_prelude::*;
    use itertools::Itertools;
    use reputation::{BetaParameters, BetaReputation, ReputationEngine};
    use sp_std::iter::once;
    use sp_std::prelude::*;

    use pallet_acurast::utils::ensure_source_verified;
    use pallet_acurast::{
        AllowedSourcesUpdate, JobHooks, JobId, JobIdSequence, JobRegistrationFor, MultiOrigin,
        ParameterBound, Schedule, StoredJobRegistration,
    };

    use crate::traits::*;
    use crate::types::*;
    use crate::utils::*;
    use crate::{JobBudget, RewardManager};

    #[pallet::config]
    pub trait Config: frame_system::Config + pallet_acurast::Config {
        type RuntimeEvent: From<Event<Self>>
            + IsType<<Self as pallet_acurast::Config>::RuntimeEvent>
            + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        /// The max length of the allowed sources list for a registration.
        #[pallet::constant]
        type MaxAllowedConsumers: Get<u32> + ParameterBound;
        /// The maximum matches that can be proposed with one extrinsic call.
        #[pallet::constant]
        type MaxProposedMatches: Get<u32>;
        #[pallet::constant]
        type MaxFinalizeJobs: Get<u32>;
        /// Extra structure to include in the registration of a job.
        type RegistrationExtra: IsType<<Self as pallet_acurast::Config>::RegistrationExtra>
            + Into<JobRequirementsFor<Self>>;
        /// The ID for this pallet
        #[pallet::constant]
        type PalletId: Get<PalletId>;
        /// The ID of the hyperdrive pallet
        #[pallet::constant]
        type HyperdrivePalletId: Get<PalletId>;
        /// The the time tolerance in milliseconds. Represents the delta by how much we expect `now` timestamp being stale,
        /// hence `now <= currentmillis <= now + ReportTolerance`.
        ///
        /// Should be at least the worst case block time. Otherwise valid reports that are included near the end of a block
        /// would be considered outide of the agreed schedule despite being within schedule.
        #[pallet::constant]
        type ReportTolerance: Get<u64>;
        type Balance: Parameter + From<u64> + IsType<u128> + Balance + FixedPointOperand;
        type ManagerProvider: ManagerProvider<Self>;
        type ProcessorLastSeenProvider: ProcessorLastSeenProvider<Self>;
        /// Logic for locking and paying tokens for job execution
        type RewardManager: RewardManager<Self>;
        /// Hook to act on marketplace related state transitions.
        type MarketplaceHooks: MarketplaceHooks<Self>;
        type WeightInfo: WeightInfo;

        #[cfg(feature = "runtime-benchmarks")]
        type BenchmarkHelper: crate::benchmarking::BenchmarkHelper<Self>;
    }

    pub(crate) const STORAGE_VERSION: StorageVersion = StorageVersion::new(4);

    #[pallet::pallet]
    #[pallet::storage_version(STORAGE_VERSION)]
    pub struct Pallet<T>(_);

    /// The storage for jobs' status as a map [`AccountId`] `(consumer)` -> [`JobId`] -> [`JobStatus`].
    #[pallet::storage]
    #[pallet::getter(fn stored_job_status)]
    pub type StoredJobStatus<T: Config> = StorageDoubleMap<
        _,
        Blake2_128,
        MultiOrigin<T::AccountId>,
        Blake2_128,
        JobIdSequence,
        JobStatus,
    >;

    /// The storage for basic advertisements' restrictions (without pricing). They are stored as a map [`AccountId`] `(source)` -> [`AdvertisementRestriction`] since only one
    /// advertisement per client is allowed.
    #[pallet::storage]
    #[pallet::getter(fn stored_advertisement)]
    pub type StoredAdvertisementRestriction<T: Config> = StorageMap<
        _,
        Blake2_128,
        T::AccountId,
        AdvertisementRestriction<T::AccountId, T::MaxAllowedConsumers>,
    >;

    /// The storage for advertisements' pricings. They are stored as a map [`AccountId`] `(source)` -> [`Pricing`] since only one
    /// advertisement per client, and at most one pricing for each distinct `AssetID` is allowed.
    #[pallet::storage]
    #[pallet::getter(fn stored_advertisement_pricing)]
    pub type StoredAdvertisementPricing<T: Config> =
        StorageMap<_, Blake2_128, T::AccountId, PricingFor<T>>;

    /// The storage for remaining capacity for each source. Can be negative if capacity is reduced beyond the number of jobs currently assigned.
    #[pallet::storage]
    #[pallet::getter(fn stored_storage_capacity)]
    pub type StoredStorageCapacity<T: Config> = StorageMap<_, Blake2_128, T::AccountId, i64>;

    /// Reputation as a map [`AccountId`] `(source)` -> [`BetaParameters`].
    #[pallet::storage]
    #[pallet::getter(fn stored_reputation)]
    pub type StoredReputation<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, BetaParameters<FixedU128>>;

    /// Number of total jobs assigned.
    #[pallet::storage]
    #[pallet::getter(fn total_assigned)]
    pub type StoredTotalAssignedV3<T: Config> = StorageValue<_, u128>;

    /// Average job reward.
    #[pallet::storage]
    #[pallet::getter(fn average_reward)]
    pub type StoredAverageRewardV3<T> = StorageValue<_, u128>;

    /// Job matches as a map [`AccountId`] `(source)` -> [`JobId`] -> [`AssignmentFor<T>`]
    #[pallet::storage]
    #[pallet::getter(fn stored_matches)]
    pub type StoredMatches<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        Blake2_128Concat,
        JobId<T::AccountId>,
        AssignmentFor<T>,
    >;

    /// Job matches as a map [`JobId`] -> [`AccountId`] `(source)` -> `()`.
    ///
    /// This map can serve as a reverse index into `StoredMatches` to achieve a mapping [`JobId`] -> [[`AssignmentFor<T>`]] with one assignment per slot that is not yet finalized.
    #[pallet::storage]
    #[pallet::getter(fn assigned_processors)]
    pub type AssignedProcessors<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        JobId<T::AccountId>,
        Blake2_128Concat,
        T::AccountId,
        (),
    >;

    /// Tracks reward amounts locked for each job on pallet account as a map [`JobId`] -> [`T::Balance`]
    #[pallet::storage]
    #[pallet::getter(fn job_budgets)]
    pub type JobBudgets<T: Config> =
        StorageMap<_, Blake2_128, JobId<T::AccountId>, T::Balance, ValueQuery>;

    #[pallet::event]
    #[pallet::generate_deposit(pub (super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// A registration was successfully matched. [Match]
        JobRegistrationMatched(MatchFor<T>),
        /// A registration was successfully matched. [JobId, SourceId, Assignment]
        JobRegistrationAssigned(JobId<T::AccountId>, T::AccountId, AssignmentFor<T>),
        /// A report for an execution has arrived. [JobId, SourceId, Assignment]
        Reported(JobId<T::AccountId>, T::AccountId, AssignmentFor<T>),
        /// A advertisement was successfully stored. [advertisement, who]
        AdvertisementStored(AdvertisementFor<T>, T::AccountId),
        /// A registration was successfully removed. [who]
        AdvertisementRemoved(T::AccountId),
        /// An execution is reported to be successful.
        ExecutionSuccess(JobId<T::AccountId>, ExecutionOperationHash),
        /// An execution is reported to have failed.
        ExecutionFailure(JobId<T::AccountId>, ExecutionFailureMessage),
        /// This event is emitted when a job is finalized.
        JobFinalized(JobId<T::AccountId>),
    }

    #[pallet::error]
    pub enum Error<T> {
        /// Generic overflow during a calculating with checked operatios.
        CalculationOverflow,
        /// Generic for unexpected checked calculation errors.
        UnexpectedCheckedCalculation,
        /// The job registration must specify non-zero `duration`.
        JobRegistrationZeroDuration,
        /// The job registration must specify a schedule that contains a maximum of [MAX_EXECUTIONS_PER_JOB] executions.
        JobRegistrationScheduleExceedsMaximumExecutions,
        /// The job registration must specify a schedule that contains at least one execution.
        JobRegistrationScheduleContainsZeroExecutions,
        /// The job registration's must specify `duration` < `interval`.
        JobRegistrationDurationExceedsInterval,
        /// The job registration's must specify `start` in the future.
        JobRegistrationStartInPast,
        /// The job registration's must specify `end` >= `start`.
        JobRegistrationEndBeforeStart,
        /// The job registration's must specify non-zero `slots`.
        JobRegistrationZeroSlots,
        /// Job status not found. SEVERE error
        JobStatusNotFound,
        /// The job registration can't be modified/deregistered if it passed the Open state.
        JobRegistrationUnmodifiable,
        /// The job registration can't be finalized given its current state.
        CannotFinalizeJob(JobStatus),
        /// Acknowledge cannot be called for a job that does not have `JobStatus::Matched` status.
        CannotAcknowledgeWhenNotMatched,
        /// Report cannot be called for a job that was not acknowledged.
        CannotReportWhenNotAcknowledged,
        /// Advertisement not found when attempt to delete it.
        AdvertisementNotFound,
        /// Advertisement not found when attempt to delete it.
        AdvertisementPricingNotFound,
        /// The allowed consumers list for a registration exeeded the max length.
        TooManyAllowedConsumers,
        /// The allowed consumers list for a registration cannot be empty if provided.
        TooFewAllowedConsumers,
        /// The allowed number of slots is exceeded.
        TooManySlots,
        /// Advertisement cannot be deleted while matched to at least one job.
        ///
        /// Pricing and capacity can be updated, e.g. the capacity can be set to 0 no no longer receive job matches.
        CannotDeleteAdvertisementWhileMatched,
        /// Failed to retrieve funds from pallet account to pay source. SEVERE error
        FailedToPay,
        /// Asset is not allowed by `AssetBarrier`.
        AssetNotAllowedByBarrier,
        /// Capacity not known for a source. SEVERE error
        CapacityNotFound,
        /// Match is invalid due to the start time already passed.
        OverdueMatch,
        /// Match is invalid due to incorrect source count.
        IncorrectSourceCountInMatch,
        /// Match is invalid due to a duplicate source for distinct slots.
        DuplicateSourceInMatch,
        /// Match is invalid due to an unverfied source while `allow_only_verified_sources` is true.
        UnverifiedSourceInMatch,
        /// Match is invalid due to a source's maximum memory exceeded.
        SchedulingWindowExceededInMatch,
        /// Match is invalid due to a source's maximum memory exceeded.
        MaxMemoryExceededInMatch,
        /// Match is invalid due to a source's maximum memory exceeded.
        NetworkRequestQuotaExceededInMatch,
        /// Match is invalid due to a source not having enough capacity.
        InsufficientStorageCapacityInMatch,
        /// Match is invalid due to a source not part of the provided whitelist.
        SourceNotAllowedInMatch,
        /// Match is invalid due to a consumer not part of the provided whitelist.
        ConsumerNotAllowedInMatch,
        /// Match is invalid due to insufficient reward regarding the current source pricing.
        InsufficientRewardInMatch,
        /// Match is invalid due to insufficient reputation of a proposed source.
        InsufficientReputationInMatch,
        /// Match is invalid due to overlapping schedules.
        ScheduleOverlapInMatch,
        /// Received a report from a source that is not assigned.
        ReportFromUnassignedSource,
        /// More reports than expected total.
        MoreReportsThanExpected,
        /// Report received outside of schedule.
        ReportOutsideSchedule,
        /// Reputation not known for a source. SEVERE error
        ReputationNotFound,
        /// Job required module not available.
        ModuleNotAvailableInMatch,
        /// The job is not assigned to the given processor
        JobNotAssigned,
        /// The job cannot be finalized yet.
        JobCannotBeFinalized,
        /// Nested Acurast error.
        PalletAcurast(pallet_acurast::Error<T>),
    }

    impl<T> From<pallet_acurast::Error<T>> for Error<T> {
        fn from(e: pallet_acurast::Error<T>) -> Self {
            Error::<T>::PalletAcurast(e)
        }
    }

    impl<T> Error<T> {
        /// Returns true if the error is due to invalid matching proposal, i.e. *not* a hard internal error.
        fn is_matching_error(self: &Self) -> bool {
            match self {
                Error::OverdueMatch => true,
                Error::IncorrectSourceCountInMatch => true,
                Error::DuplicateSourceInMatch => true,
                Error::UnverifiedSourceInMatch => true,
                Error::SchedulingWindowExceededInMatch => true,
                Error::MaxMemoryExceededInMatch => true,
                Error::NetworkRequestQuotaExceededInMatch => true,
                Error::InsufficientStorageCapacityInMatch => true,
                Error::SourceNotAllowedInMatch => true,
                Error::ConsumerNotAllowedInMatch => true,
                Error::InsufficientRewardInMatch => true,
                Error::InsufficientReputationInMatch => true,
                Error::ScheduleOverlapInMatch => true,
                Error::ModuleNotAvailableInMatch => true,
                Error::PalletAcurast(e) => match *e {
                    pallet_acurast::Error::FulfillSourceNotAllowed => true,
                    pallet_acurast::Error::FulfillSourceNotVerified => true,
                    pallet_acurast::Error::AttestationCertificateNotValid => true,
                    pallet_acurast::Error::AttestationUsageExpired => true,
                    pallet_acurast::Error::RevokedCertificate => true,
                    _ => false,
                },
                Error::CapacityNotFound => true,

                Error::CalculationOverflow => false,
                Error::UnexpectedCheckedCalculation => false,
                Error::JobRegistrationZeroDuration => false,
                Error::JobRegistrationScheduleExceedsMaximumExecutions => false,
                Error::JobRegistrationScheduleContainsZeroExecutions => false,
                Error::JobRegistrationDurationExceedsInterval => false,
                Error::JobRegistrationStartInPast => false,
                Error::JobRegistrationEndBeforeStart => false,
                Error::JobRegistrationZeroSlots => false,
                Error::JobStatusNotFound => false,
                Error::JobRegistrationUnmodifiable => false,
                Error::CannotFinalizeJob(_) => false,
                Error::CannotAcknowledgeWhenNotMatched => false,
                Error::CannotReportWhenNotAcknowledged => false,
                Error::AdvertisementNotFound => false,
                Error::AdvertisementPricingNotFound => false,
                Error::TooManyAllowedConsumers => false,
                Error::TooFewAllowedConsumers => false,
                Error::TooManySlots => false,
                Error::CannotDeleteAdvertisementWhileMatched => false,
                Error::FailedToPay => false,
                Error::AssetNotAllowedByBarrier => false,
                Error::ReportFromUnassignedSource => false,
                Error::MoreReportsThanExpected => false,
                Error::ReportOutsideSchedule => false,
                Error::ReputationNotFound => false,
                Error::JobNotAssigned => false,
                Error::JobCannotBeFinalized => false,

                Error::__Ignore(_, _) => false,
            }
        }
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_runtime_upgrade() -> frame_support::weights::Weight {
            crate::migration::migrate::<T>()
        }
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Advertise resources by providing a [AdvertisementFor].
        ///
        /// If the source has another active advertisement, the advertisement is updated given the updates does not
        /// violate any system invariants. For example, if the ad is currently assigned, changes to pricing are prohibited
        /// and only capacity updates will be tolerated.
        #[pallet::call_index(0)]
        #[pallet::weight(< T as Config >::WeightInfo::advertise())]
        pub fn advertise(
            origin: OriginFor<T>,
            advertisement: AdvertisementFor<T>,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;

            Self::do_advertise(&who, &advertisement)?;

            Self::deposit_event(Event::AdvertisementStored(advertisement, who));
            Ok(().into())
        }

        /// Delete advertisement.
        #[pallet::call_index(1)]
        #[pallet::weight(< T as Config >::WeightInfo::delete_advertisement())]
        pub fn delete_advertisement(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;

            <StoredAdvertisementRestriction<T>>::get(&who)
                .ok_or(Error::<T>::AdvertisementNotFound)?;

            // prohibit updates as long as jobs assigned
            ensure!(
                !Self::has_matches(&who),
                Error::<T>::CannotDeleteAdvertisementWhileMatched
            );

            let _ = <StoredAdvertisementPricing<T>>::remove(&who);
            <StoredStorageCapacity<T>>::remove(&who);
            <StoredAdvertisementRestriction<T>>::remove(&who);

            Self::deposit_event(Event::AdvertisementRemoved(who));
            Ok(().into())
        }

        /// Proposes processors to match with a job. The match fails if it conflicts with the processor's schedule.
        #[pallet::call_index(2)]
        #[pallet::weight(< T as Config >::WeightInfo::propose_matching(matches.len() as u32))]
        pub fn propose_matching(
            origin: OriginFor<T>,
            matches: BoundedVec<MatchFor<T>, <T as Config>::MaxProposedMatches>,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;

            let remaining_rewards = Self::process_matching(&matches)?;

            // pay part of accumulated remaining reward (unspent to consumer) to matcher
            T::RewardManager::pay_matcher_reward(remaining_rewards, &who)?;

            Ok(().into())
        }

        /// Acknowledges a matched job. It fails if the origin is not the account that was matched for the job.
        #[pallet::call_index(3)]
        #[pallet::weight(< T as Config >::WeightInfo::acknowledge_match())]
        pub fn acknowledge_match(
            origin: OriginFor<T>,
            job_id: JobId<T::AccountId>,
            pub_keys: PubKeys,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;

            let (changed, assignment) = <StoredMatches<T>>::try_mutate(
                &who,
                &job_id,
                |m| -> Result<(bool, AssignmentFor<T>), Error<T>> {
                    // CHECK that job was matched previously to calling source
                    let assignment = m
                        .as_mut()
                        .ok_or(Error::<T>::CannotAcknowledgeWhenNotMatched)?;
                    let changed = !assignment.acknowledged;
                    assignment.acknowledged = true;
                    assignment.pub_keys = pub_keys;
                    Ok((changed, assignment.to_owned()))
                },
            )?;

            if changed {
                <StoredJobStatus<T>>::try_mutate(
                    &job_id.0,
                    &job_id.1,
                    |s| -> Result<(), Error<T>> {
                        let status = s.ok_or(Error::<T>::JobStatusNotFound)?;
                        *s = Some(match status {
                            JobStatus::Open => Err(Error::<T>::CannotAcknowledgeWhenNotMatched)?,
                            JobStatus::Matched => JobStatus::Assigned(1),
                            JobStatus::Assigned(count) => JobStatus::Assigned(count + 1),
                        });

                        Ok(())
                    },
                )?;

                // activate hook so implementing side can react on job assignment
                T::MarketplaceHooks::assign_job(&job_id, &assignment.pub_keys)?;

                Self::deposit_event(Event::JobRegistrationAssigned(
                    job_id,
                    who,
                    assignment.clone(),
                ));
            }
            Ok(().into())
        }

        /// Report on completion of fulfillments done on target chain for a previously registered and matched job.
        /// Reward is payed out to source if timing of this call is within expected interval. More precisely,
        /// the report is accepted if `[now, now + tolerance]` overlaps with an execution of the schedule agreed on.
        /// `tolerance` is a pallet config value.
        #[pallet::call_index(4)]
        #[pallet::weight(< T as Config >::WeightInfo::report())]
        pub fn report(
            origin: OriginFor<T>,
            job_id: JobId<T::AccountId>,
            execution_result: ExecutionResult,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;

            // find assignment
            let assignment = <StoredMatches<T>>::try_mutate(
                &who,
                &job_id,
                |a| -> Result<AssignmentFor<T>, Error<T>> {
                    // NOTE: the None case is the "good case", used when there is *no entry yet and thus no duplicate assignment so far*.
                    if let Some(assignment) = a.as_mut() {
                        // CHECK that job is assigned
                        ensure!(
                            assignment.acknowledged,
                            Error::<T>::CannotReportWhenNotAcknowledged
                        );

                        // CHECK that we don't accept more reports than expected
                        ensure!(
                            assignment.sla.met < assignment.sla.total,
                            Error::<T>::MoreReportsThanExpected
                        );

                        assignment.sla.met += 1;
                        return Ok(assignment.to_owned());
                    } else {
                        return Err(Error::<T>::ReportFromUnassignedSource);
                    }
                },
            )?;

            let registration = <StoredJobRegistration<T>>::get(&job_id.0, &job_id.1)
                .ok_or(pallet_acurast::Error::<T>::JobRegistrationNotFound)?;

            let now = Self::now()?;
            let now_max = now
                .checked_add(T::ReportTolerance::get())
                .ok_or(Error::<T>::CalculationOverflow)?;

            ensure!(
                registration
                    .schedule
                    .overlaps(
                        assignment.start_delay,
                        registration
                            .schedule
                            .range(assignment.start_delay)
                            .ok_or(Error::<T>::CalculationOverflow)?
                            .0,
                        now_max
                    )
                    .ok_or(Error::<T>::CalculationOverflow)?,
                Error::<T>::ReportOutsideSchedule
            );

            // pay only after all other steps succeeded without errors because paying reward is not revertable

            match T::ManagerProvider::manager_of(&who) {
                Ok(manager) => {
                    T::RewardManager::pay_reward(
                        &job_id,
                        assignment.fee_per_execution.clone(),
                        &manager,
                    )?;

                    match execution_result {
                        ExecutionResult::Success(operation_hash) => Self::deposit_event(
                            Event::ExecutionSuccess(job_id.clone(), operation_hash),
                        ),
                        ExecutionResult::Failure(message) => {
                            Self::deposit_event(Event::ExecutionFailure(job_id.clone(), message))
                        }
                    }

                    Self::deposit_event(Event::Reported(job_id, who, assignment.clone()));
                    Ok(().into())
                }
                Err(err_result) => Err(err_result.into()),
            }
        }

        /// Called by processors when the assigned job can be finalized.
        #[pallet::call_index(5)]
        #[pallet::weight(<T as Config>::WeightInfo::finalize_job())]
        pub fn finalize_job(
            origin: OriginFor<T>,
            job_id: JobId<T::AccountId>,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;

            let registration = <StoredJobRegistration<T>>::get(&job_id.0, &job_id.1)
                .ok_or(pallet_acurast::Error::<T>::JobRegistrationNotFound)?;

            // find assignment
            let assignment =
                <StoredMatches<T>>::get(&who, &job_id).ok_or(Error::<T>::JobNotAssigned)?;

            ensure!(
                Self::actual_schedule_ended(&registration.schedule, &assignment)?,
                Error::<T>::JobCannotBeFinalized
            );

            let unmet: u64 = assignment.sla.total - assignment.sla.met;

            // update reputation since we don't expect further reports for this job
            // (only update for attested devices!)
            if ensure_source_verified::<T>(&who).is_ok() {
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
                        <StoredReputation<T>>::get(&who).ok_or(Error::<T>::ReputationNotFound)?;

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
                        &who,
                        BetaParameters {
                            r: beta_params.r,
                            s: beta_params.s,
                        },
                    );
                }
            }

            // only remove storage point indexed by a single processor (corresponding to the completed duties for the assigned slot)
            <StoredMatches<T>>::remove(&who, &job_id);
            <AssignedProcessors<T>>::remove(&job_id, &who);

            // increase capacity
            <StoredStorageCapacity<T>>::mutate(&who, |c| {
                *c = c.unwrap_or(0).checked_add(registration.storage.into())
            });

            Self::deposit_event(Event::JobFinalized(job_id));
            Ok(().into())
        }

        /// Called by a consumer whenever he wishes to finalizes some of his jobs and get unused rewards refunded.
        ///
        /// For details see [`Pallet<T>::finalize_jobs_for`].
        #[pallet::call_index(6)]
        #[pallet::weight(<T as Config>::WeightInfo::finalize_jobs(job_ids.len() as u32))]
        pub fn finalize_jobs(
            origin: OriginFor<T>,
            job_ids: BoundedVec<JobIdSequence, T::MaxFinalizeJobs>,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;

            Self::finalize_jobs_for(
                job_ids
                    .into_iter()
                    .map(|job_id_seq| (MultiOrigin::Acurast(who.clone()), job_id_seq)),
            )
        }
    }

    impl<T: Config> From<Error<T>> for pallet_acurast::Error<T> {
        fn from(_: Error<T>) -> Self {
            Self::JobHookFailed
        }
    }

    impl<T: Config> JobHooks<T> for Pallet<T> {
        /// Registers a job in the marketplace by providing a [JobRegistration].
        /// If a job for the same `(accountId, script)` was previously registered, it will be overwritten.
        fn register_hook(
            _who: &MultiOrigin<T::AccountId>,
            job_id: &JobId<T::AccountId>,
            registration: &JobRegistrationFor<T>,
        ) -> DispatchResultWithPostInfo {
            let e: <T as Config>::RegistrationExtra = registration.extra.clone().into();
            let requirements: JobRequirementsFor<T> = e.into();

            ensure!(
                registration.schedule.duration > 0,
                Error::<T>::JobRegistrationZeroDuration
            );
            let execution_count = registration.schedule.execution_count();
            ensure!(
                execution_count <= MAX_EXECUTIONS_PER_JOB,
                Error::<T>::JobRegistrationScheduleExceedsMaximumExecutions
            );
            ensure!(
                execution_count > 0,
                Error::<T>::JobRegistrationScheduleContainsZeroExecutions
            );
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

            if let Some(job_status) = <StoredJobStatus<T>>::get(&job_id.0, &job_id.1) {
                ensure!(
                    job_status == JobStatus::Open,
                    Error::<T>::JobRegistrationUnmodifiable
                );
            } else {
                <StoredJobStatus<T>>::insert(&job_id.0, &job_id.1, JobStatus::default());
            }

            match requirements.instant_match {
                Some(sources) => {
                    // ignore remaining rewards; do not pay out the matcher which is the same as the one registering
                    let _ = Self::process_matching(once(&Match {
                        job_id: job_id.clone(),
                        sources,
                    }))?;
                }
                None => {}
            }

            // - lock only after all other steps succeeded without errors because locking reward is not revertable
            // - reward is understood per slot and execution, so calculate total_reward_amount first
            // - lock the complete reward inclusive the matcher share and potential gap to actual fee that will be refunded during job finalization
            T::RewardManager::lock_reward(&job_id, Self::total_reward_amount(registration)?)?;

            Ok(().into())
        }

        /// Deregisters a job.
        ///
        /// The final act of removing the job from [`StoredJobRegistration`] is the responsibility of the caller,
        /// since this storage point is owned by pallet_acurast.
        fn deregister_hook(job_id: &JobId<T::AccountId>) -> DispatchResultWithPostInfo {
            let job_status = <StoredJobStatus<T>>::get(&job_id.0, &job_id.1)
                .ok_or(Error::<T>::JobStatusNotFound)?;
            match job_status {
                JobStatus::Open => {
                    T::MarketplaceHooks::finalize_job(job_id, T::RewardManager::refund(job_id)?)?;

                    <StoredJobStatus<T>>::remove(&job_id.0, &job_id.1);
                }
                JobStatus::Matched => {
                    T::MarketplaceHooks::finalize_job(job_id, T::RewardManager::refund(job_id)?)?;

                    // Get the job requirements
                    let registration = <StoredJobRegistration<T>>::get(&job_id.0, &job_id.1)
                        .ok_or(pallet_acurast::Error::<T>::JobRegistrationNotFound)?;

                    // Remove matching data and increase processor capacity
                    for (p, _) in <AssignedProcessors<T>>::iter_prefix(&job_id) {
                        <StoredMatches<T>>::remove(&p, &job_id);
                        // increase capacity
                        <StoredStorageCapacity<T>>::mutate(&p, |c| {
                            *c = c.unwrap_or(0).checked_add(registration.storage.into())
                        });
                    }

                    let _ = <AssignedProcessors<T>>::clear_prefix(
                        &job_id,
                        <T as pallet_acurast::Config>::MaxSlots::get(),
                        None,
                    );
                    <StoredJobStatus<T>>::remove(&job_id.0, &job_id.1);
                }
                JobStatus::Assigned(_) => {
                    // Get the job requirements
                    let registration = <StoredJobRegistration<T>>::get(&job_id.0, &job_id.1)
                        .ok_or(pallet_acurast::Error::<T>::JobRegistrationNotFound)?;
                    let extra: <T as Config>::RegistrationExtra = registration.extra.clone().into();
                    let requirements: JobRequirementsFor<T> = extra.into();

                    // Compute the reward amount to be payed to each assigned processor
                    let remaining_reward = Self::reserved(job_id);
                    let reward_per_processor = remaining_reward
                        .checked_div(&(requirements.slots.into()))
                        .ok_or(Error::<T>::UnexpectedCheckedCalculation)?;

                    // Pay reward to the processor and clear matching data
                    for (processor, _) in <AssignedProcessors<T>>::iter_prefix(&job_id) {
                        // find assignment
                        let assignment = <StoredMatches<T>>::get(&processor, &job_id)
                            .ok_or(Error::<T>::JobNotAssigned)?;
                        // Compensate processor for acknowledging the job
                        if assignment.acknowledged {
                            match T::ManagerProvider::manager_of(&processor) {
                                Ok(manager) => T::RewardManager::pay_reward(
                                    &job_id,
                                    reward_per_processor,
                                    &manager,
                                ),
                                Err(err_result) => Err(err_result.into()),
                            }?;
                        }
                        // Remove match
                        <StoredMatches<T>>::remove(&processor, &job_id);
                        // increase capacity
                        <StoredStorageCapacity<T>>::mutate(&processor, |c| {
                            *c = c.unwrap_or(0).checked_add(registration.storage.into())
                        });
                    }

                    // The job creator will only receive the amount that could not be divided between the acknowledged processors
                    T::MarketplaceHooks::finalize_job(job_id, T::RewardManager::refund(job_id)?)?;

                    let _ = <AssignedProcessors<T>>::clear_prefix(
                        &job_id,
                        <T as pallet_acurast::Config>::MaxSlots::get(),
                        None,
                    );
                    <StoredJobStatus<T>>::remove(&job_id.0, &job_id.1);
                }
            }

            Ok(().into())
        }

        /// Updates the allowed sources list of a [JobRegistration].
        fn update_allowed_sources_hook(
            _who: &T::AccountId,
            job_id: &JobId<T::AccountId>,
            _updates: &Vec<AllowedSourcesUpdate<T::AccountId>>,
        ) -> DispatchResultWithPostInfo {
            let job_status = <StoredJobStatus<T>>::get(&job_id.0, &job_id.1)
                .ok_or(Error::<T>::JobStatusNotFound)?;

            ensure!(
                job_status == JobStatus::Open,
                Error::<T>::JobRegistrationUnmodifiable
            );

            Ok(().into())
        }
    }

    impl<T: Config> JobBudget<T> for Pallet<T> {
        fn reserve(job_id: &JobId<T::AccountId>, reward: T::Balance) -> Result<(), ()> {
            <JobBudgets<T>>::mutate(job_id, |amount| {
                *amount = amount.checked_add(&reward).ok_or(())?;
                Ok(())
            })
        }

        fn unreserve(job_id: &JobId<T::AccountId>, reward: T::Balance) -> Result<(), ()> {
            <JobBudgets<T>>::mutate(job_id, |amount| {
                if reward > *amount {
                    return Err(());
                }
                *amount = amount.checked_sub(&reward).ok_or(())?;
                Ok(())
            })
        }

        fn unreserve_remaining(job_id: &JobId<T::AccountId>) -> T::Balance {
            <JobBudgets<T>>::mutate(job_id, |amount| {
                let remaining = amount.clone();
                *amount = 0u8.into();
                remaining
            })
        }

        fn reserved(job_id: &JobId<T::AccountId>) -> T::Balance {
            <JobBudgets<T>>::get(job_id)
        }
    }

    impl<T: Config> Pallet<T> {
        /// Checks if a Processor - Job match is possible and returns the remaining job rewards by `job_id`.
        ///
        /// If the job is no longer in status [`JobStatus::Open`], the matching is skipped without returning an error.
        /// **The returned vector does not include an entry for skipped matches.**
        ///
        /// Every other invalidity in a provided [`Match`] fails the entire call.
        fn process_matching<'a>(
            matching: impl IntoIterator<Item = &'a MatchFor<T>>,
        ) -> Result<Vec<(JobId<T::AccountId>, T::Balance)>, DispatchError> {
            let mut remaining_rewards: Vec<(JobId<T::AccountId>, T::Balance)> = Default::default();

            for m in matching {
                let job_status = <StoredJobStatus<T>>::get(&m.job_id.0, &m.job_id.1)
                    .ok_or(Error::<T>::JobStatusNotFound)?;

                if job_status != JobStatus::Open {
                    // skip but don't fail this match
                    continue;
                }

                let registration = <StoredJobRegistration<T>>::get(&m.job_id.0, &m.job_id.1)
                    .ok_or(pallet_acurast::Error::<T>::JobRegistrationNotFound)?;
                let e: <T as Config>::RegistrationExtra = registration.extra.clone().into();
                let requirements: JobRequirementsFor<T> = e.into();

                let now = Self::now()?;
                ensure!(
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
                    ensure!(
                        ad.max_memory >= registration.memory,
                        Error::<T>::MaxMemoryExceededInMatch
                    );

                    // CHECK network request quota sufficient
                    Self::check_network_request_quota_sufficient(
                        &ad,
                        &registration.schedule,
                        registration.network_requests,
                    )?;

                    // CHECK remaining storage capacity sufficient
                    let capacity = <StoredStorageCapacity<T>>::get(&planned_execution.source)
                        .ok_or(Error::<T>::CapacityNotFound)?;
                    ensure!(
                        capacity >= registration.storage as i64,
                        Error::<T>::InsufficientStorageCapacityInMatch
                    );

                    // CHECK source is whitelisted
                    ensure!(
                        is_source_whitelisted::<T>(
                            &planned_execution.source,
                            &registration.allowed_sources
                        ),
                        Error::<T>::SourceNotAllowedInMatch
                    );

                    // CHECK consumer is whitelisted
                    ensure!(
                        is_consumer_whitelisted::<T>(&m.job_id.0, &ad.allowed_consumers),
                        Error::<T>::ConsumerNotAllowedInMatch
                    );

                    // CHECK reputation sufficient
                    Self::check_min_reputation(
                        requirements.min_reputation,
                        &planned_execution.source,
                    )?;

                    // CHECK schedule
                    Self::fits_schedule(
                        &planned_execution.source,
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
                    ensure!(
                        fee_per_execution <= reward_amount,
                        Error::<T>::InsufficientRewardInMatch
                    );

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
                                        start_delay: planned_execution.start_delay,
                                        fee_per_execution,
                                        acknowledged: false,
                                        sla: SLA {
                                            total: execution_count,
                                            met: 0,
                                        },
                                        pub_keys: PubKeys::default(),
                                    });
                                    Ok(())
                                }
                            }?;
                            Ok(())
                        },
                    )?;
                    <AssignedProcessors<T>>::insert(&m.job_id, &planned_execution.source, ());
                    <StoredStorageCapacity<T>>::set(
                        &planned_execution.source,
                        capacity.checked_sub(registration.storage.into()),
                    );
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

                <StoredJobStatus<T>>::insert(&m.job_id.0, &m.job_id.1, JobStatus::Matched);
                Self::deposit_event(Event::JobRegistrationMatched(m.clone()));
            }
            return Ok(remaining_rewards);
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
                }
                SchedulingWindow::Delta(delta) => {
                    ensure!(
                        now.checked_add(*delta)
                            .ok_or(Error::<T>::CalculationOverflow)?
                            >= schedule
                                .end_time
                                .checked_add(start_delay)
                                .ok_or(Error::<T>::CalculationOverflow)?,
                        Error::<T>::SchedulingWindowExceededInMatch
                    );
                }
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
                schedule
                    .duration
                    .checked_mul(ad.network_request_quota.into())
                    .unwrap_or(0u64)
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
                            T::ProcessorLastSeenProvider::last_seen(&p)
                                .map(|last_seen| last_seen >= latest_seen_after)
                                .unwrap_or(false)
                        } else {
                            true
                        }
                    }
                    Err(e) => {
                        if !e.is_matching_error() {
                            return Err(RuntimeApiError::FilterMatchingSources);
                        }

                        false
                    }
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
                    || ensure_source_verified::<T>(&source).is_ok(),
                Error::<T>::UnverifiedSourceInMatch
            );

            let ad = <StoredAdvertisementRestriction<T>>::get(&source)
                .ok_or(Error::<T>::AdvertisementNotFound)?;

            for required_module in &registration.required_modules {
                ensure!(
                    ad.available_modules.contains(required_module),
                    Error::<T>::ModuleNotAvailableInMatch
                );
            }

            let pricing = <StoredAdvertisementPricing<T>>::get(&source)
                .ok_or(Error::<T>::AdvertisementPricingNotFound)?;

            if let Some(schedule) = &registration.schedule {
                let now = Self::now()?;
                ensure!(now < schedule.start_time, Error::<T>::OverdueMatch);

                // CHECK the scheduling_window allow to schedule this job
                Self::check_scheduling_window(&pricing.scheduling_window, schedule, now, 0)?;

                // CHECK schedule
                Self::fits_schedule(&source, &schedule, 0)?;

                // CHECK network request quota sufficient
                if let Some(network_requests) = registration.network_requests {
                    Self::check_network_request_quota_sufficient(&ad, &schedule, network_requests)?;
                }

                // CHECK remaining storage capacity sufficient
                if let Some(storage) = &registration.storage {
                    // calculate fee
                    let fee_per_execution = Self::fee_per_execution(&schedule, *storage, &pricing)?;

                    // CHECK price not exceeding reward
                    ensure!(
                        fee_per_execution <= registration.reward.clone(),
                        Error::<T>::InsufficientRewardInMatch
                    );
                }
            }

            // CHECK memory sufficient
            if let Some(memory) = &registration.memory {
                ensure!(
                    ad.max_memory >= *memory,
                    Error::<T>::MaxMemoryExceededInMatch
                );
            }

            // CHECK remaining storage capacity sufficient
            if let Some(storage) = &registration.storage {
                let capacity =
                    <StoredStorageCapacity<T>>::get(&source).ok_or(Error::<T>::CapacityNotFound)?;
                ensure!(
                    capacity >= *storage as i64,
                    Error::<T>::InsufficientStorageCapacityInMatch
                );
            }

            // CHECK source is whitelisted
            ensure!(
                is_source_whitelisted::<T>(&source, &registration.allowed_sources),
                Error::<T>::SourceNotAllowedInMatch
            );

            // CHECK consumer is whitelisted
            if let Some(consumer) = consumer {
                ensure!(
                    is_consumer_whitelisted::<T>(&consumer, &ad.allowed_consumers),
                    Error::<T>::ConsumerNotAllowedInMatch
                );
            }

            // CHECK reputation sufficient
            Self::check_min_reputation(registration.min_reputation, &source)?;

            Ok(())
        }

        /// Returns true if the source has currently at least one match (not necessarily assigned).
        fn has_matches(source: &T::AccountId) -> bool {
            // NOTE we use a trick to check if map contains *any* secondary key: we use `any` to short-circuit
            // whenever we encounter the first - so at least one - element in the iterator.
            <StoredMatches<T>>::iter_prefix_values(&source).any(|_| true)
        }

        /// Checks of a new job schedule fits with the existing schedule for a processor.
        fn fits_schedule(
            source: &T::AccountId,
            schedule: &Schedule,
            start_delay: u64,
        ) -> Result<(), Error<T>> {
            for (job_id, assignment) in <StoredMatches<T>>::iter_prefix(&source) {
                // TODO decide tradeoff: we could save this lookup at the cost of storing the schedule along with the match or even completly move it from StoredJobRegistration into StoredMatches
                let other = <StoredJobRegistration<T>>::get(&job_id.0, &job_id.1)
                    .ok_or(pallet_acurast::Error::<T>::JobRegistrationNotFound)?;

                // check if the whole schedule periods have an overlap
                if schedule.start_time >= other.schedule.end_time
                    || schedule.end_time <= other.schedule.start_time
                {
                    // periods don't overlap
                    continue;
                }

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
            }

            Ok(().into())
        }

        /// Calculates if the job ended considering the given assignment.
        fn actual_schedule_ended(
            schedule: &Schedule,
            assignment: &AssignmentFor<T>,
        ) -> Result<bool, Error<T>> {
            let now = Self::now()?
                .checked_add(T::ReportTolerance::get())
                .ok_or(Error::<T>::CalculationOverflow)?;
            let (_actual_start, actual_end) = schedule
                .range(assignment.start_delay)
                .ok_or(Error::<T>::CalculationOverflow)?;
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
        fn total_reward_amount(
            registration: &JobRegistrationFor<T>,
        ) -> Result<T::Balance, Error<T>> {
            let e: <T as Config>::RegistrationExtra = registration.extra.clone().into();
            let requirements: JobRequirementsFor<T> = e.into();

            Ok(requirements
                .reward
                .checked_mul(&((requirements.slots as u128).into()))
                .ok_or(Error::<T>::CalculationOverflow)?
                .checked_mul(&registration.schedule.execution_count().into())
                .ok_or(Error::<T>::CalculationOverflow)?)
        }

        /// Calculates the fee per job execution.
        fn fee_per_execution(
            schedule: &Schedule,
            storage: u32,
            pricing: &PricingFor<T>,
        ) -> Result<T::Balance, Error<T>> {
            Ok(pricing
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
                .ok_or(Error::<T>::CalculationOverflow)?)
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
                let job_status = <StoredJobStatus<T>>::get(&job_id.0, &job_id.1)
                    .ok_or(Error::<T>::JobStatusNotFound)?;

                let registration = <StoredJobRegistration<T>>::get(&job_id.0, &job_id.1)
                    .ok_or(pallet_acurast::Error::<T>::JobRegistrationNotFound)?;

                match job_status {
                    JobStatus::Open => Err(Error::<T>::CannotFinalizeJob(job_status))?,
                    JobStatus::Matched => {
                        let match_overdue = Self::now()? >= registration.schedule.start_time;
                        if !match_overdue {
                            Err(Error::<T>::CannotFinalizeJob(job_status))?;
                        }
                    }
                    JobStatus::Assigned(_) => {
                        // in the "good case" when all processors finalized their slot we can accept the finalization independent of schedule's latest end
                        let some_assigned = <AssignedProcessors<T>>::iter_prefix(&job_id)
                            .next()
                            .is_some();
                        if some_assigned && !Self::schedule_ended(&registration.schedule)? {
                            Err(Error::<T>::CannotFinalizeJob(job_status))?;
                        }
                    }
                }

                // removed completed job from remaining storage points
                for (p, _) in <AssignedProcessors<T>>::iter_prefix(&job_id) {
                    <StoredMatches<T>>::remove(&p, &job_id);

                    // increase capacity
                    <StoredStorageCapacity<T>>::mutate(&p, |c| {
                        *c = c.unwrap_or(0).checked_add(registration.storage.into())
                    });
                }
                let _ = <AssignedProcessors<T>>::clear_prefix(
                    &job_id,
                    <T as pallet_acurast::Config>::MaxSlots::get(),
                    None,
                );

                T::MarketplaceHooks::finalize_job(&job_id, T::RewardManager::refund(&job_id)?)?;

                pallet_acurast::Pallet::<T>::clear_environment_for(&job_id);
                <StoredJobStatus<T>>::remove(&job_id.0, &job_id.1);
                <StoredJobRegistration<T>>::remove(&job_id.0, &job_id.1);

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
                    let job = <StoredJobRegistration<T>>::get(&job_id.0, &job_id.1)
                        .ok_or(RuntimeApiError::MatchedJobs)?;
                    Ok(JobAssignment {
                        job_id,
                        job,
                        assignment,
                    })
                })
                .collect()
        }

        /// Returns the current timestamp.
        pub fn now() -> Result<u64, Error<T>> {
            Ok(<T as pallet_acurast::Config>::UnixTime::now()
                .as_millis()
                .try_into()
                .map_err(|_| pallet_acurast::Error::<T>::FailedTimestampConversion)?)
        }
    }
}

sp_api::decl_runtime_apis! {
    /// API to interact with Acurast marketplace pallet.
    pub trait MarketplaceRuntimeApi<Reward: codec::Codec, AccountId: codec::Codec, Extra: codec::Codec, MaxAllowedSources: ParameterBound, MaxEnvVars: ParameterBound, EnvKeyMaxSize: ParameterBound, EnvValueMaxSize: ParameterBound> {
         fn filter_matching_sources(
            registration: PartialJobRegistration<Reward, AccountId, MaxAllowedSources>,
            sources: Vec<AccountId>,
            consumer: Option<MultiOrigin<AccountId>>,
            latest_seen_after: Option<u128>,
        ) -> Result<Vec<AccountId>, RuntimeApiError>;

        fn job_environment(
            job_id: JobId<AccountId>,
            source: AccountId,
        ) -> Result<Option<Environment<MaxEnvVars, EnvKeyMaxSize, EnvValueMaxSize>>, RuntimeApiError>;

        fn matched_jobs(
            source: AccountId,
        ) -> Result<Vec<JobAssignment<Reward, AccountId, MaxAllowedSources, Extra>>, RuntimeApiError>;

        fn attestation(
            source: AccountId,
        ) -> Result<Option<Attestation>, RuntimeApiError>;
    }
}
