use frame_support::{pallet_prelude::*, storage::bounded_vec::BoundedVec, PalletError};
use sp_std::prelude::*;

use pallet_acurast::{
    AllowedSources, JobId, JobModules, JobRegistration, MultiOrigin, ParameterBound, Schedule,
};

use core::fmt::Debug;
use serde::{Deserialize, Serialize};

use crate::Config;

pub(crate) const MAX_EXECUTIONS_PER_JOB: u64 = 6_308_000; // run a job every 5 seconds for a year

pub(crate) const EXECUTION_OPERATION_HASH_MAX_LENGTH: u32 = 256;
pub(crate) const EXECUTION_FAILURE_MESSAGE_MAX_LENGTH: u32 = 1024;

pub type ExecutionOperationHash = BoundedVec<u8, ConstU32<EXECUTION_OPERATION_HASH_MAX_LENGTH>>;
pub type ExecutionFailureMessage = BoundedVec<u8, ConstU32<EXECUTION_FAILURE_MESSAGE_MAX_LENGTH>>;
pub type PlannedExecutions<AccountId, MaxSlots> = BoundedVec<PlannedExecution<AccountId>, MaxSlots>;

pub type JobRegistrationForMarketplace<T> = JobRegistration<
    <T as frame_system::Config>::AccountId,
    <T as pallet_acurast::Config>::MaxAllowedSources,
    <T as Config>::RegistrationExtra,
>;

pub type PartialJobRegistrationForMarketplace<T> = PartialJobRegistration<
    <T as Config>::Balance,
    <T as frame_system::Config>::AccountId,
    <T as pallet_acurast::Config>::MaxAllowedSources,
>;

pub type MatchFor<T> =
    Match<<T as frame_system::Config>::AccountId, <T as pallet_acurast::Config>::MaxSlots>;

/// Struct defining the extra fields for a `JobRegistration`.
#[derive(
    RuntimeDebug,
    Encode,
    Decode,
    MaxEncodedLen,
    TypeInfo,
    Clone,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
)]
pub struct RegistrationExtra<Reward, AccountId, MaxSlots: ParameterBound> {
    pub requirements: JobRequirements<Reward, AccountId, MaxSlots>,
}

impl<Reward, AccountId, MaxSlots: ParameterBound>
    From<RegistrationExtra<Reward, AccountId, MaxSlots>>
    for JobRequirements<Reward, AccountId, MaxSlots>
{
    fn from(extra: RegistrationExtra<Reward, AccountId, MaxSlots>) -> Self {
        extra.requirements
    }
}

/// The resource advertisement by a source containing pricing and capacity announcements.
#[derive(RuntimeDebug, Encode, Decode, MaxEncodedLen, TypeInfo, Clone, PartialEq, Eq)]
pub struct Advertisement<AccountId, Reward, MaxAllowedConsumers: Get<u32>> {
    /// The reward token accepted. Understood as one-of per job assigned.
    pub pricing: Pricing<Reward>,
    /// Maximum memory in bytes not to be exceeded during any job's execution.
    pub max_memory: u32,
    /// Maximum network requests per second not to be exceeded.
    pub network_request_quota: u8,
    /// Storage capacity in bytes not to be exceeded in matching. The associated fee is listed in [pricing].
    pub storage_capacity: u32,
    /// An optional array of the [AccountId]s of consumers whose jobs should get accepted. If the array is [None], then jobs from all consumers are accepted.
    pub allowed_consumers: Option<BoundedVec<MultiOrigin<AccountId>, MaxAllowedConsumers>>,
    /// The modules available to the job on processor.
    pub available_modules: JobModules,
}

pub type AdvertisementFor<T> = Advertisement<
    <T as frame_system::Config>::AccountId,
    <T as Config>::Balance,
    <T as Config>::MaxAllowedConsumers,
>;

/// The resource advertisement by a source containing the base restrictions.
#[derive(RuntimeDebug, Encode, Decode, MaxEncodedLen, TypeInfo, Clone, PartialEq)]
pub struct AdvertisementRestriction<AccountId, MaxAllowedConsumers: ParameterBound> {
    /// Maximum memory in bytes not to be exceeded during any job's execution.
    pub max_memory: u32,
    /// Maximum network requests per second not to be exceeded.
    pub network_request_quota: u8,
    /// Storage capacity in bytes not to be exceeded in matching. The associated fee is listed in [pricing].
    pub storage_capacity: u32,
    /// An optional array of the [AccountId]s of consumers whose jobs should get accepted. If the array is [None], then jobs from all consumers are accepted.
    pub allowed_consumers: Option<BoundedVec<MultiOrigin<AccountId>, MaxAllowedConsumers>>,
    /// The modules available to the job on processor.
    pub available_modules: JobModules,
}

/// Defines the scheduling window in which to accept matches for this pricing,
/// either as an absolute end time (in milliseconds since Unix Epoch)
/// or as a time delta (in milliseconds) added to the current time.
#[derive(RuntimeDebug, Encode, Decode, MaxEncodedLen, TypeInfo, Clone, PartialEq, Eq, Copy)]
pub enum SchedulingWindow {
    /// Latest accepted end time of any matched job in milliseconds since Unix Epoch.
    End(u64),
    /// A time delta (in milliseconds) from now defining the window in which to accept jobs.
    ///
    /// Latest accepted end time of any matched job will be `now + delta`.
    Delta(u64),
}

/// Pricing listing cost per resource unit and slash on SLA violation.
/// Specified in specific asset that is payed out or deducted from stake on complete fulfillment.
#[derive(RuntimeDebug, Encode, Decode, MaxEncodedLen, TypeInfo, Clone, PartialEq, Eq)]
pub struct Pricing<Reward> {
    /// Fee per millisecond in [reward_asset].
    pub fee_per_millisecond: Reward,
    /// Fee per storage byte in [reward_asset].
    pub fee_per_storage_byte: Reward,
    /// A fixed base fee for each execution (for each slot and at each interval) in [reward_asset].
    pub base_fee_per_execution: Reward,
    /// The scheduling window in which to accept matches for this pricing.
    pub scheduling_window: SchedulingWindow,
}

pub type PricingFor<T> = Pricing<<T as Config>::Balance>;

/// A proposed [Match] becomes an [Assignment] once it's acknowledged.
///
/// It's intended use is as part of a storage map that includes the job's and source's ID in its key.
///
/// The pricing agreed at the time of matching is stored along with an assignment.
#[derive(RuntimeDebug, Encode, Decode, MaxEncodedLen, TypeInfo, Clone, PartialEq)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub struct Assignment<Reward> {
    /// The 0-based slot index assigned to the source.
    pub slot: u8,
    /// The start delay for the first execution and all the following executions.
    pub start_delay: u64,
    /// The fee owed to source for each execution.
    pub fee_per_execution: Reward,
    /// If this assignment was acknowledged.
    pub acknowledged: bool,
    /// Keeps track of the SLA.
    pub sla: SLA,
    /// Processor Pub Keys
    pub pub_keys: PubKeys,
}

#[derive(RuntimeDebug, Encode, Decode, TypeInfo, Clone, PartialEq)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub struct JobAssignment<Reward, AccountId, MaxAllowedSources: Get<u32>, Extra> {
    pub job_id: JobId<AccountId>,
    pub job: JobRegistration<AccountId, MaxAllowedSources, Extra>,
    pub assignment: Assignment<Reward>,
}

pub const NUMBER_OF_PUB_KEYS: u32 = 3;
pub const PUB_KEYS_MAX_LENGTH: u32 = 33;

pub type PubKeyBytes = BoundedVec<u8, ConstU32<PUB_KEYS_MAX_LENGTH>>;

/// The public keys of the processor revealed when a job is acknowledged.
pub type PubKeys = BoundedVec<PubKey, ConstU32<NUMBER_OF_PUB_KEYS>>;

/// The public key revealed by a processor.
#[derive(RuntimeDebug, Encode, Decode, MaxEncodedLen, TypeInfo, Clone, Eq, PartialEq)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub enum PubKey {
    SECP256r1(PubKeyBytes),
    SECP256k1(PubKeyBytes),
    ED25519(PubKeyBytes),
}

pub type AssignmentFor<T> = Assignment<<T as Config>::Balance>;

pub type JobAssignmentFor<T> = JobAssignment<
    <T as Config>::Balance,
    <T as frame_system::Config>::AccountId,
    <T as pallet_acurast::Config>::MaxAllowedSources,
    <T as pallet_acurast::Config>::RegistrationExtra,
>;

/// The allowed sources update operation.
#[derive(
    RuntimeDebug, Encode, Decode, MaxEncodedLen, TypeInfo, Clone, PartialEq, Copy, PalletError,
)]
pub enum JobStatus {
    /// Status after a job got registered.
    Open,
    /// Status after a valid match for a job got submitted.
    Matched,
    /// Status after a number of acknowledgments were submitted by sources.
    Assigned(u8),
    // The implicit final status leads to removal of job from status storage.
}

impl Default for JobStatus {
    fn default() -> Self {
        JobStatus::Open
    }
}

/// Keeps track of the SLA during and after a job's schedule is completed.
///
/// Also used to ensure that Acurast does not accept more than the expected number of reports (and pays out no more rewards).
#[derive(RuntimeDebug, Encode, Decode, MaxEncodedLen, TypeInfo, Clone, PartialEq, Copy)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub struct SLA {
    pub total: u64,
    pub met: u64,
}

pub type JobRequirementsFor<T> = JobRequirements<
    <T as Config>::Balance,
    <T as frame_system::Config>::AccountId,
    <T as pallet_acurast::Config>::MaxSlots,
>;

/// Structure representing a job registration.
#[derive(
    RuntimeDebug,
    Encode,
    Decode,
    MaxEncodedLen,
    TypeInfo,
    Clone,
    Eq,
    PartialEq,
    Serialize,
    Deserialize,
)]
pub struct JobRequirements<Reward, AccountId, MaxSlots: ParameterBound> {
    /// The number of execution slots to be assigned to distinct sources. Either all or no slot get assigned by matching.
    pub slots: u8,
    /// Reward offered for each slot and scheduled execution of the job.
    pub reward: Reward,
    /// Minimum reputation required to process job, in parts per million, `r ∈ [0, 1_000_000]`.
    pub min_reputation: Option<u128>,
    /// Optional match provided with the job requirements. If provided, it gets processed instantaneously during
    /// registration call and validation errors lead to abortion of the call.
    pub instant_match: Option<PlannedExecutions<AccountId, MaxSlots>>,
}

/// A (one-sided) matching of a job to sources such that the requirements of both sides, consumer and source, are met.
#[derive(RuntimeDebug, Encode, Decode, MaxEncodedLen, TypeInfo, Clone, Eq, PartialEq)]
pub struct Match<AccountId, MaxSlots: ParameterBound> {
    /// The job to match.
    pub job_id: JobId<AccountId>,
    /// The sources to match each of the job's slots with.
    pub sources: PlannedExecutions<AccountId, MaxSlots>,
}

/// Structure representing a job registration partially specified.
///
/// Useful for frontend to filter for processors that would match.
#[derive(RuntimeDebug, Encode, Decode, TypeInfo, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PartialJobRegistration<Reward, AccountId, MaxAllowedSources: Get<u32>> {
    /// An optional array of the [AccountId]s allowed to fulfill the job. If the array is [None], then all sources are allowed.
    pub allowed_sources: Option<AllowedSources<AccountId, MaxAllowedSources>>,
    /// A boolean indicating if only verified sources can fulfill the job. A verified source is one that has provided a valid key attestation.
    pub allow_only_verified_sources: bool,
    /// The schedule describing the desired (multiple) execution(s) of the script.
    pub schedule: Option<Schedule>,
    /// Maximum memory bytes used during a single execution of the job.
    pub memory: Option<u32>,
    /// Maximum network request used during a single execution of the job.
    pub network_requests: Option<u32>,
    /// Maximum storage bytes used during the whole period of the job's executions.
    pub storage: Option<u32>,
    /// The modules required for the job.
    pub required_modules: JobModules,
    /// Job requirements: The number of execution slots to be assigned to distinct sources. Either all or no slot get assigned by matching.
    pub slots: Option<u8>,
    /// Job requirements: Reward offered for each slot and scheduled execution of the job.
    pub reward: Reward,
    /// Job requirements: Minimum reputation required to process job, in parts per million, `r ∈ [0, 1_000_000]`.
    pub min_reputation: Option<u128>,
}

/// The details for a single planned slot execution with the delay.
#[derive(
    RuntimeDebug,
    Encode,
    Decode,
    MaxEncodedLen,
    TypeInfo,
    Clone,
    Eq,
    PartialEq,
    Serialize,
    Deserialize,
)]
pub struct PlannedExecution<AccountId> {
    /// The source.
    pub source: AccountId,
    /// The start delay for the first execution and all the following executions.
    pub start_delay: u64,
}

#[derive(RuntimeDebug, Encode, Decode, MaxEncodedLen, TypeInfo, Clone, PartialEq)]
pub enum ExecutionResult {
    /// Success with operation hash.
    Success(ExecutionOperationHash),
    /// Failure with message.
    Failure(ExecutionFailureMessage),
}

/// Allows to hook additional logic for marketplace related state transitions.
pub trait MarketplaceHooks<T: Config> {
    fn assign_job(
        job_id: &JobId<<T as frame_system::Config>::AccountId>,
        pub_keys: &PubKeys,
    ) -> DispatchResultWithPostInfo;

    fn finalize_job(
        job_id: &JobId<<T as frame_system::Config>::AccountId>,
        refund: T::Balance,
    ) -> DispatchResultWithPostInfo;
}

impl<T: Config> MarketplaceHooks<T> for () {
    fn assign_job(
        _job_id: &JobId<<T as frame_system::Config>::AccountId>,
        _pub_keys: &PubKeys,
    ) -> DispatchResultWithPostInfo {
        Ok(().into())
    }

    fn finalize_job(
        _job_id: &JobId<<T as frame_system::Config>::AccountId>,
        _refund: T::Balance,
    ) -> DispatchResultWithPostInfo {
        Ok(().into())
    }
}

/// Runtime API error.
#[cfg_attr(feature = "std", derive(thiserror::Error))]
#[derive(RuntimeDebug, codec::Encode, codec::Decode, PartialEq, Eq, TypeInfo)]
pub enum RuntimeApiError {
    /// Error when filtering matching sources failed.
    #[cfg_attr(feature = "std", error("Filtering matching sources failed."))]
    FilterMatchingSources,
    /// Error when retrieving matched jobs.
    #[cfg_attr(feature = "std", error("Retriving matched jobs failed."))]
    MatchedJobs,
}

impl RuntimeApiError {
    /// Consume given error `e` with `self` and generate a native log entry with error details.
    pub fn log_error(self, e: impl Debug) -> Self {
        log::error!(
            target: "runtime::acurast_marketplace",
            "[{:?}] error: {:?}",
            self,
            e,
        );
        self
    }

    /// Consume given error `e` with `self` and generate a native log entry with error details.
    pub fn log_debug(self, e: impl Debug) -> Self {
        log::debug!(
            target: "runtime::acurast_marketplace",
            "[{:?}] error: {:?}",
            self,
            e,
        );
        self
    }
}
