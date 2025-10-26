#![cfg_attr(not(feature = "std"), no_std)]

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

mod error;
mod functions;
mod hooks;
mod match_checker;
mod migration;
pub mod payments;
pub mod traits;
pub mod types;
mod utils;
pub mod weights;

use pallet_acurast::JobId;
use sp_std::prelude::*;

#[cfg(feature = "runtime-benchmarks")]
pub use benchmarking::BenchmarkHelper;

#[frame_support::pallet]
pub mod pallet {
	use frame_support::{
		dispatch::DispatchResultWithPostInfo,
		ensure,
		pallet_prelude::*,
		sp_runtime::{
			traits::{CheckedAdd, CheckedSub, Hash},
			FixedPointOperand, FixedU128,
		},
		traits::{tokens::Balance, EnsureOrigin},
		Blake2_128, Blake2_128Concat, PalletId,
	};
	use frame_system::pallet_prelude::*;
	use reputation::BetaParameters;
	use sp_std::prelude::*;

	use pallet_acurast::{
		JobId, JobIdSequence, JobRegistrationFor, Metrics, MultiOrigin, ParameterBound, Script,
		ScriptMutability, StoredJobRegistration,
	};

	use crate::{traits::*, types::*, JobBudget, RewardManager};

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_acurast::Config {
		type RuntimeEvent: From<Event<Self>>
			+ IsType<<Self as pallet_acurast::Config>::RuntimeEvent>
			+ IsType<<Self as frame_system::Config>::RuntimeEvent>;
		/// The max length of the allowed sources list for a registration.
		#[pallet::constant]
		type MaxAllowedConsumers: Get<u32> + ParameterBound;
		/// The maximum competing processors per slot.
		#[pallet::constant]
		type Competing: Get<u32>;
		/// Maximum time delta in ms that a job can be matched before the job's start. Relevant for [`AssignmentStrategy::Single`].
		#[pallet::constant]
		type MatchingCompetingMinInterval: Get<u64>;
		/// Maximum time delta in ms that each job execution can be matched before the execution's start. Relevant for [`AssignmentStrategy::Competing`].
		#[pallet::constant]
		type MatchingCompetingDueDelta: Get<u64>;
		/// The maximum matches that can be proposed with one extrinsic call.
		#[pallet::constant]
		type MaxProposedMatches: Get<u32>;
		/// The maximum execution matches that can be proposed with one extrinsic call.
		#[pallet::constant]
		type MaxProposedExecutionMatches: Get<u32>;
		#[pallet::constant]
		type MaxFinalizeJobs: Get<u32>;
		/// Extra structure to include in the registration of a job.
		type RegistrationExtra: IsType<<Self as pallet_acurast::Config>::RegistrationExtra>
			+ From<JobRequirementsFor<Self>>
			+ Into<JobRequirementsFor<Self>>;
		/// The ID for this pallet
		#[pallet::constant]
		type PalletId: Get<PalletId>;
		/// The ID of the hyperdrive pallet
		#[pallet::constant]
		type HyperdrivePalletId: Get<PalletId>;
		/// The time tolerance in milliseconds. Represents the delta by how much we expect `now` timestamp being stale,
		/// hence `now <= currentmillis <= now + ReportTolerance`.
		///
		/// Should be at least the worst case block time. Otherwise valid reports that are included near the end of a block
		/// would be considered outside of the agreed schedule despite being within schedule.
		#[pallet::constant]
		type ReportTolerance: Get<u64>;
		type Balance: Parameter + From<u64> + IsType<u128> + Balance + FixedPointOperand;
		type ProcessorInfoProvider: ProcessorInfoProvider<Self>;
		/// Logic for locking and paying tokens for job execution
		type RewardManager: RewardManager<Self>;
		/// Hook to act on marketplace related state transitions.
		type MarketplaceHooks: MarketplaceHooks<Self>;
		#[pallet::constant]
		type MaxJobCleanups: Get<u32>;
		/// The hashing system (algorithm) being used to hash deployments (owner + script) (e.g. Blake2).
		type DeploymentHashing: Hash<Output = DeploymentHash> + TypeInfo;
		/// The hashing system (algorithm) being used to generate key ids for deployments (e.g. Blake2).
		type KeyIdHashing: Hash<Output = KeyId> + TypeInfo;
		/// Origin allowed to call update extrinsics
		type UpdateOrigin: EnsureOrigin<Self::RuntimeOrigin>;
		/// Origin allowed to call operational extrinsics
		type OperatorOrigin: EnsureOrigin<Self::RuntimeOrigin>;
		/// WeightInfo
		type WeightInfo: WeightInfo;

		#[cfg(feature = "runtime-benchmarks")]
		type BenchmarkHelper: crate::benchmarking::BenchmarkHelper<Self>;
	}

	pub(crate) const STORAGE_VERSION: StorageVersion = StorageVersion::new(7);

	#[pallet::pallet]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T>(_);

	/// The storage for jobs' status as a map [`MultiOrigin`] -> [`JobIdSequence`] -> [`JobStatus`], where the two keys represent consumer's [`JobId`].
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

	/// The storage for jobs' status as a map [`JobId`] -> `execution_index` -> [`JobStatus`].
	#[pallet::storage]
	#[pallet::getter(fn stored_job_execution_status)]
	pub type StoredJobExecutionStatus<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		JobId<T::AccountId>,
		Blake2_128Concat,
		u64,
		JobStatus,
		ValueQuery,
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

	#[pallet::storage]
	#[pallet::getter(fn next_report_index)]
	pub type NextReportIndex<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		JobId<T::AccountId>,
		Blake2_128Concat,
		T::AccountId,
		u64,
	>;

	/// Editors are accounts with the right to update a job's script. They are stored as a map [`JobId`] -> [`T::AccountId`] `(editor)`.
	///
	/// If a job has an editor it makes the Job's script as well as other information kept in the script bundle, such as restart policy, editable.
	///
	/// A job's editor defaults to the job's owner but can be transferred to one or multiple accounts (including multisig accounts).
	/// The transfer of the editor role has to be initiate by the current editor.
	/// Thus if the original owner (and editor) transfers this permission he also looses the permission to assign the editors. This is so to ensure a specific entity, the editor, has control
	/// over what gets executed under usage of a set of keys (identified by a key ID).
	#[pallet::storage]
	#[pallet::getter(fn editors)]
	pub type Editors<T: Config> =
		StorageMap<_, Blake2_128Concat, JobId<T::AccountId>, T::AccountId>;

	/// Key IDs stored as a map [`JobId`] -> [`KeyId`].
	///
	/// Key IDs are hashes that are used to lookup (sets of) keys on processor. The same key ID will produce the same set of keys on the same processor (until processor's key store is reset).
	#[pallet::storage]
	#[pallet::getter(fn job_key_ids)]
	pub type JobKeyIds<T: Config> = StorageMap<_, Blake2_128Concat, JobId<T::AccountId>, KeyId>;

	/// Deployment hashes stored as a map [`MultiOrigin`] -> [`JobIdSequence`] -> [`KeyId`]. The key is split up to allow for bulk deletion.
	#[pallet::storage]
	#[pallet::getter(fn deployment_hashes)]
	pub type DeploymentHashes<T: Config> = StorageDoubleMap<
		_,
		Blake2_128,
		MultiOrigin<T::AccountId>,
		Blake2_128,
		JobIdSequence,
		DeploymentHash,
	>;

	/// Key IDs for future job registrations (continuing the deployments), reusing keys stored as a map [`DeploymentHash`] -> [`KeyId`].
	///
	/// The key of this map is of type [`DeploymentHash`] to ensure future job deployments of same owner + script will result in same key ID,
	/// even if the job that lead to the creation of this key ID has ended or was cancelled.
	#[pallet::storage]
	#[pallet::getter(fn deployment_key_ids)]
	pub type DeploymentKeyIds<T: Config> = StorageMap<_, Identity, DeploymentHash, KeyId>;

	#[pallet::storage]
	#[pallet::getter(fn min_fee_per_millisecond)]
	pub type MinFeePerMillisecond<T: Config> = StorageValue<_, T::Balance, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn v7_migration_state)]
	pub type V7MigrationState<T: Config> =
		StorageValue<_, BoundedVec<u8, ConstU32<80>>, OptionQuery>;

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
		/// A registration was successfully matched. [Match]
		JobExecutionMatched(ExecutionMatchFor<T>),
		/// Job script edited. [job_id]
		JobScriptEdited(JobId<T::AccountId>),
		/// Editor transferred. [job_id, new_editor]
		EditorTransferred(JobId<T::AccountId>, T::AccountId),
		/// Job became immutable since editor was cleared. [job_id]
		JobBecameImmutable(JobId<T::AccountId>),
		/// Min fee per millisecond updated
		MinFeePerMillisecondUpdated(<T as Config>::Balance),
		/// A registration was successfully matched. [JobId]
		JobRegistrationMatchedV2(JobId<T::AccountId>),
		/// A registration was successfully matched. [JobId, SourceId]
		JobRegistrationAssignedV2(JobId<T::AccountId>, T::AccountId),
		/// A report for an execution has arrived. [JobId, SourceId]
		ReportedV2(JobId<T::AccountId>, T::AccountId),
		/// A advertisement was successfully stored. [who]
		AdvertisementStoredV2(T::AccountId),
		/// A registration was successfully matched. [JobId]
		JobExecutionMatchedV2(JobId<T::AccountId>),
		/// Migration started.
		V7MigrationStarted,
		/// Migration completed.
		V7MigrationCompleted,
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
		/// The job registration can't have a interval below minimum for competing assignment strategy.
		JobRegistrationIntervalBelowMinimum,
		/// Job status not found. SEVERE error
		JobStatusNotFound,
		/// The job registration can't be modified/deregistered if it passed the Open state.
		JobRegistrationUnmodifiable,
		/// The job registration can't be finalized given its current state.
		CannotFinalizeJob(JobStatus),
		/// Acknowledge cannot be called for a job that does not have `JobStatus::Matched` status.
		CannotAcknowledgeWhenNotMatched,
		/// Acknowledge cannot be called for a job that does not have `JobStatus::Matched` status.
		CannotAcknowledgeForOtherThanCurrentExecution,
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
		/// Match is invalid due to the start time being too much in future.
		UnderdueMatch,
		/// Match is invalid due to incorrect source count.
		IncorrectSourceCountInMatch,
		/// Match is invalid due to incorrect execution index.
		IncorrectExecutionIndex,
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
		/// Processor version mismatch.
		ProcessorVersionMismatch,
		/// Processor CPU score mismatch.
		ProcessorCpuScoreMismatch,
		/// Match failed because a processor does not meet the mininum metrics. [pool_id]
		ProcessorMinMetricsNotMet(u8),
		/// Keys cannot be reused from the job requested, maybe because they were already transferred away.
		CannotReuseKeysFrom,
		/// Job not found.
		JobNotFound,
		/// The provided script value is not valid. The value needs to be and ipfs:// url.
		InvalidScriptValue,
		/// Job to reuse keys from was not found.
		JobForKeyReuseNotFound,
		/// A immutable job cannot be edited.
		ImmutableJobCannotBeEdited,
		/// A mutable job cannot be made immutable during.
		MutableJobCannotBeMadeImmutable,
		/// A mutable job can only be edited by the current editor.
		OnlyEditorCanEditScript,
		/// Only editor can transfer editing permissions.
		OnlyEditorCanTransferEditor,
		/// A mutable job can only be extended (new job registered reusing keys) by same owner.
		OnlyOwnerCanExtendJob,
		/// Execution already reported.
		ExecutionAlreadyReported,
		/// Cannot acknowledge a job after its start time
		CannotAcknowledgeAfterStartTime,
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T>
	where
		<T as pallet_acurast::Config>::RegistrationExtra: IsType<
			RegistrationExtra<
				T::Balance,
				T::AccountId,
				T::MaxSlots,
				T::ProcessorVersion,
				T::MaxVersions,
			>,
		>,
	{
		fn on_runtime_upgrade() -> frame_support::weights::Weight {
			crate::migration::migrate::<T>()
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Advertise resources by providing a [AdvertisementFor].
		#[pallet::call_index(0)]
		#[pallet::weight(< T as Config >::WeightInfo::advertise())]
		pub fn advertise(
			origin: OriginFor<T>,
			advertisement: AdvertisementFor<T>,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;

			Self::do_advertise(&who, &advertisement)?;

			Self::deposit_event(Event::AdvertisementStoredV2(who));
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
			ensure!(!Self::has_matches(&who), Error::<T>::CannotDeleteAdvertisementWhileMatched);

			<StoredAdvertisementPricing<T>>::remove(&who);
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
			Self::process_acknowledge_match(who, job_id, ExecutionSpecifier::All, pub_keys)
		}

		/// Acknowledges a matched job. It fails if the origin is not the account that was matched for the job.
		#[pallet::call_index(8)]
		#[pallet::weight(< T as Config >::WeightInfo::acknowledge_execution_match())]
		pub fn acknowledge_execution_match(
			origin: OriginFor<T>,
			job_id: JobId<T::AccountId>,
			// makes this extrinsic idempotent: if execution is not the current one stored in StoredMatches for the acknowledging source, this call will fail
			execution_index: u64,
			pub_keys: PubKeys,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			Self::process_acknowledge_match(
				who,
				job_id,
				ExecutionSpecifier::Index(execution_index),
				pub_keys,
			)
		}

		/// Report on completion of fulfillments done on target chain for a previously registered and matched job.
		/// Reward is paid out to source if timing of this call is within expected interval. More precisely,
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

			_ = Self::do_report(&job_id, &who)?;

			match execution_result {
				ExecutionResult::Success(operation_hash) => {
					Self::deposit_event(Event::ExecutionSuccess(job_id.clone(), operation_hash))
				},
				ExecutionResult::Failure(message) => {
					Self::deposit_event(Event::ExecutionFailure(job_id.clone(), message))
				},
			}

			Self::deposit_event(Event::ReportedV2(job_id, who));
			Ok(().into())
		}

		/// Called by processors when the assigned job can be finalized.
		///
		/// DEPRECATED: this call is not needed anymore. The cleanup logic has been moved to the final `report` extrinsic call.
		#[pallet::call_index(5)]
		#[pallet::weight(< T as Config >::WeightInfo::finalize_job())]
		pub fn finalize_job(
			origin: OriginFor<T>,
			job_id: JobId<T::AccountId>,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;

			Self::do_cleanup_assignment(&who, &job_id)?;

			Self::deposit_event(Event::JobFinalized(job_id));
			Ok(().into())
		}

		/// Called by a consumer whenever he wishes to finalizes some of his jobs and get unused rewards refunded.
		///
		/// DEPRECATED: this call is a no-op. Use the `deregister` extrinsic on `pallet-acurast` instead.
		///
		/// For details see [`Pallet<T>::finalize_jobs_for`].
		#[pallet::call_index(6)]
		#[pallet::weight(< T as Config >::WeightInfo::finalize_jobs(job_ids.len() as u32))]
		pub fn finalize_jobs(
			origin: OriginFor<T>,
			job_ids: BoundedVec<JobIdSequence, T::MaxFinalizeJobs>,
		) -> DispatchResultWithPostInfo {
			let _who = ensure_signed(origin)?;

			log::warn!("Calling deprecated extrinsics with params: {:?}", job_ids);

			Ok(().into())
		}

		/// Proposes processors to match with a job's execution.
		#[pallet::call_index(7)]
		#[pallet::weight(< T as Config >::WeightInfo::propose_execution_matching(matches.len() as u32))]
		pub fn propose_execution_matching(
			origin: OriginFor<T>,
			matches: BoundedVec<ExecutionMatchFor<T>, <T as Config>::MaxProposedExecutionMatches>,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;

			let remaining_rewards = Self::process_execution_matching(&matches)?;

			// pay part of accumulated remaining reward (unspent to consumer) to matcher
			T::RewardManager::pay_matcher_reward(remaining_rewards, &who)?;

			Ok(().into())
		}

		#[pallet::call_index(9)]
		#[pallet::weight(< T as Config >::WeightInfo::cleanup_storage((*max_iterations) as u32))]
		pub fn cleanup_storage(
			origin: OriginFor<T>,
			job_id: JobId<T::AccountId>,
			max_iterations: u8,
		) -> DispatchResultWithPostInfo {
			T::OperatorOrigin::ensure_origin(origin)?;
			let maybe_job = <StoredJobRegistration<T>>::get(&job_id.0, job_id.1);
			if maybe_job.is_none() && max_iterations > 0 {
				let mut remaining_iterations = max_iterations;
				for (processor, _) in <AssignedProcessors<T>>::drain_prefix(&job_id) {
					<StoredMatches<T>>::remove(&processor, &job_id);
					remaining_iterations -= 1;
					if remaining_iterations == 0 {
						break;
					}
				}
			}
			Ok(().into())
		}

		#[pallet::call_index(10)]
		#[pallet::weight(< T as Config >::WeightInfo::cleanup_assignments(job_ids.len() as u32))]
		pub fn cleanup_assignments(
			origin: OriginFor<T>,
			job_ids: BoundedVec<JobId<T::AccountId>, T::MaxJobCleanups>,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			Self::do_cleanup_assignments(&who, job_ids.into())?;
			Ok(().into())
		}

		/// Registers a deployment, optionally transfering (reusing) keys from a previously registered job, inheriting its editor.
		///
		/// When `reuse_keys_from` is provided, the registration attempts to reuse keys from the previously registered job `j`.
		/// If the script is different from original job, the keys are transferred, meaning no other job can reuse them
		/// (but the original job will use them until the end of its lifetime).
		///
		/// Some rules have to be followed:
		///
		/// - if this job is mutable then the `j` has to be mutable
		/// - the owners of this job and `j` need to match
		/// - the editors of this job and `j` need to match
		///
		/// This extrinsic could replace [`Self::register`] since this extrinsic has the same effect if `registration_type ==` [`ScriptMutability::Immutable`] and `reuse_keys_from == None`.
		#[pallet::call_index(11)]
		#[pallet::weight(< T as Config >::WeightInfo::deploy())]
		pub fn deploy(
			origin: OriginFor<T>,
			registration: JobRegistrationFor<T>,
			mutability: ScriptMutability<T::AccountId>,
			reuse_keys_from: Option<JobId<T::AccountId>>,
			min_metrics: Option<Metrics>,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;

			let multi_origin = MultiOrigin::Acurast(who.clone());
			let job_id = (multi_origin, pallet_acurast::Pallet::<T>::next_job_id());

			let deployment_hash = T::DeploymentHashing::hash(
				&(job_id.0.clone(), registration.script.clone()).encode(),
			);

			match mutability.clone() {
				ScriptMutability::Immutable => {
					if let Some(original_job_id) = reuse_keys_from {
						// For jobs registered after editors were introduced: recover original deployment hash to check if script changed (without the need of old script being present in pallet_acurast -> StoredJobRegistration)
						if let Some(original_deployment_hash) =
							<DeploymentHashes<T>>::get(&original_job_id.0, original_job_id.1)
						{
							ensure!(
								original_deployment_hash == deployment_hash,
								Error::<T>::ImmutableJobCannotBeEdited
							);
						}
						// if None, it's not an error since the user tries to extend a job registered before job editors were introduced
						// for backwards compatibility not changing the behaviour, we rely on the key reuse by the hashing on the processor

						ensure!(
							<Editors<T>>::get(&original_job_id).is_none(),
							Error::<T>::MutableJobCannotBeMadeImmutable
						);
					}
					// in None we might still reuse keys implicitly by registering the same script
				},
				ScriptMutability::Mutable(editor_) => {
					let (derived_editor, key_id) = if let Some(original_job_id) = reuse_keys_from {
						// we disallow a change of owner in the current permission model
						ensure!(original_job_id.0 == job_id.0, Error::<T>::OnlyOwnerCanExtendJob);

						// every mutable job must have an entry in Editors since it got deployed after editors were introduced
						let original_job_editor = <Editors<T>>::get(&original_job_id)
							.ok_or(Error::<T>::ImmutableJobCannotBeEdited)?;
						let derived_editor = if let Some(editor) = editor_ {
							// attempt to set potentially different editor so check if allowed

							ensure!(
								job_id.0 == MultiOrigin::Acurast(original_job_editor.clone())
									|| original_job_editor == editor,
								Error::<T>::OnlyEditorCanTransferEditor
							);

							editor
						} else {
							// keep same editor for new job
							original_job_editor.clone()
						};

						// recover original deployment hash to check if script changed (without the need of old script being present in pallet_acurast -> StoredJobRegistration)
						let original_deployment_hash =
							<DeploymentHashes<T>>::get(&original_job_id.0, original_job_id.1)
								.ok_or(Error::<T>::JobForKeyReuseNotFound)?;

						// we allow the script to change, but only if the owner is also the editor
						ensure!(
							job_id.0 == MultiOrigin::Acurast(original_job_editor)
								|| original_deployment_hash == deployment_hash,
							Error::<T>::OnlyEditorCanEditScript
						);

						let key_id =
							Self::transfer_key_id(original_deployment_hash, deployment_hash)?;

						(derived_editor, key_id)
					} else {
						// to create a new unique key_id without using randomness, we hash the full job id and the script
						let key_id = T::KeyIdHashing::hash(&job_id.encode());
						(editor_.unwrap_or(who), key_id)
					};
					<Editors<T>>::insert(&job_id, derived_editor);

					<DeploymentKeyIds<T>>::insert(deployment_hash, key_id);

					// we only ever write once to his map on the creation of the job, so the key_id is immutable for the lifetime of a job
					<JobKeyIds<T>>::insert(&job_id, key_id);
				},
			};

			// insert deployment hash
			<DeploymentHashes<T>>::insert(&job_id.0, job_id.1, deployment_hash);

			pallet_acurast::Pallet::<T>::register_for(job_id, registration, min_metrics)
		}

		/// Edits a job's script. Preserves the set of keys used on processors.
		#[pallet::call_index(12)]
		#[pallet::weight(< T as Config >::WeightInfo::edit_script())]
		pub fn edit_script(
			origin: OriginFor<T>,
			job_id: JobId<T::AccountId>,
			script: Script,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;

			let editor =
				<Editors<T>>::get(&job_id).ok_or(Error::<T>::ImmutableJobCannotBeEdited)?;
			ensure!(editor == who, Error::<T>::OnlyEditorCanTransferEditor);

			ensure!(pallet_acurast::is_valid_script(&script), Error::<T>::InvalidScriptValue);

			let old_script =
				<StoredJobRegistration<T>>::mutate(&job_id.0, job_id.1, |registration| {
					if let Some(r) = registration.as_mut() {
						let old_script = r.script.clone();
						r.script = script.clone();
						Some(old_script)
					} else {
						None
					}
				})
				.ok_or(Error::<T>::JobNotFound)?;

			// resets the key ID for old deployment hash and inserts the updated deployment
			let original_deployment_hash =
				T::DeploymentHashing::hash(&(job_id.0.clone(), old_script).encode());
			let updated_deployment_hash =
				T::DeploymentHashing::hash(&(job_id.0.clone(), script).encode());

			let _key_id = Self::transfer_key_id(original_deployment_hash, updated_deployment_hash)?;
			<DeploymentHashes<T>>::insert(&job_id.0, job_id.1, updated_deployment_hash);
			// DO NOT update JobKeyIds to keep using previous keys DESPITE the job script update

			Self::deposit_event(Event::JobScriptEdited(job_id));
			Ok(().into())
		}

		/// Transfers the editor role for a job.
		#[pallet::call_index(13)]
		#[pallet::weight(< T as Config >::WeightInfo::transfer_editor())]
		pub fn transfer_editor(
			origin: OriginFor<T>,
			job_id: JobId<T::AccountId>,
			new_editor: Option<T::AccountId>,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;

			let editor =
				<Editors<T>>::get(&job_id).ok_or(Error::<T>::ImmutableJobCannotBeEdited)?;
			ensure!(editor == who, Error::<T>::OnlyEditorCanEditScript);

			if let Some(e) = new_editor {
				<Editors<T>>::insert(&job_id, e.clone());
				Self::deposit_event(Event::EditorTransferred(job_id, e));
			} else {
				<Editors<T>>::remove(&job_id);
				Self::deposit_event(Event::JobBecameImmutable(job_id));
			}

			Ok(().into())
		}

		#[pallet::call_index(14)]
		#[pallet::weight(< T as Config >::WeightInfo::update_min_fee_per_millisecond())]
		pub fn update_min_fee_per_millisecond(
			origin: OriginFor<T>,
			new_min_fee: T::Balance,
		) -> DispatchResultWithPostInfo {
			<T as Config>::UpdateOrigin::ensure_origin(origin)?;
			<MinFeePerMillisecond<T>>::set(new_min_fee);
			Self::deposit_event(Event::MinFeePerMillisecondUpdated(new_min_fee));
			Ok(().into())
		}
	}

	impl<T: Config> Pallet<T> {
		fn transfer_key_id(
			original_deployment_hash: DeploymentHash,
			updated_deployment_hash: DeploymentHash,
		) -> Result<KeyId, Error<T>> {
			let key_id = <DeploymentKeyIds<T>>::mutate(original_deployment_hash, |id_| {
				// clear only if still pointing to the keyId of the job updated
				if let Some(id) = *id_ {
					*id_ = None;
					Ok(id)
				} else {
					Err(Error::<T>::CannotReuseKeysFrom)
				}
			})?;
			<DeploymentKeyIds<T>>::insert(updated_deployment_hash, key_id);
			Ok(key_id)
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
				let remaining = *amount;
				*amount = 0u8.into();
				remaining
			})
		}

		fn reserved(job_id: &JobId<T::AccountId>) -> T::Balance {
			<JobBudgets<T>>::get(job_id)
		}
	}
}
