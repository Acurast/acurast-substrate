extern crate alloc;

use alloc::{format, vec::Vec};
use scale::{Decode, Encode};
use scale_info::prelude::cmp::Ordering;

#[derive(Clone, Eq, PartialEq, Encode, Decode)]
pub enum Version {
	V1 = 1,
}

#[derive(Debug, Clone, Eq, PartialEq, Encode, Decode)]
pub struct IncomingAction {
	pub id: u64,
	pub payload: VersionedIncomingActionPayload,
}

impl Ord for IncomingAction {
	fn cmp(&self, other: &Self) -> Ordering {
		self.id.cmp(&other.id)
	}
}

impl PartialOrd for IncomingAction {
	fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
		Some(self.cmp(other))
	}
}

#[derive(Debug, Clone, Eq, PartialEq, Encode, Decode)]
pub enum VersionedIncomingActionPayload {
	V1(IncomingActionPayloadV1),
}

#[derive(Debug, Clone, Eq, PartialEq, Encode, Decode)]
pub enum IncomingActionPayloadV1 {
	AssignJobProcessor(AssignProcessorPayloadV1),
	FinalizeJob(FinalizeJobPayloadV1),
	Noop,
}

#[derive(Debug, Clone, Eq, PartialEq, Encode, Decode)]
pub struct AssignProcessorPayloadV1 {
	pub job_id: u128,
	pub processor: [u8; 32],
}

#[derive(Debug, Clone, Eq, PartialEq, Encode, Decode)]
pub struct FinalizeJobPayloadV1 {
	pub job_id: u128,
	pub unused_reward: u128,
}

#[derive(Clone, Eq, PartialEq, Encode, Decode)]
pub struct RawOutgoingAction {
	pub id: u64,
	pub origin: [u8; 32],
	pub payload_version: u16,
	pub payload: Vec<u8>,
}

#[derive(Clone, Eq, PartialEq)]
pub struct OutgoingAction {
	pub id: u64,
	pub origin: [u8; 32], // AccountId
	pub payload: VersionedOutgoingActionPayload,
}

impl OutgoingAction {
	pub fn decode(mut payload: &[u8]) -> Result<OutgoingAction, scale::Error> {
		match RawOutgoingAction::decode(&mut payload) {
			Err(err) => Err(err),
			Ok(action) => Ok(OutgoingAction {
				id: action.id,
				origin: action.origin,
				payload: VersionedOutgoingActionPayload::decode(action)?,
			}),
		}
	}
}

#[derive(Clone, Eq, PartialEq, Decode)]
pub enum VersionedOutgoingActionPayload {
	V1(OutgoingActionPayloadV1),
}

impl VersionedOutgoingActionPayload {
	fn decode(action: RawOutgoingAction) -> Result<VersionedOutgoingActionPayload, scale::Error> {
		match action.payload_version {
			v if v == Version::V1 as u16 => {
				let action = OutgoingActionPayloadV1::decode(&mut action.payload.as_slice())?;

				Ok(VersionedOutgoingActionPayload::V1(action))
			},
			v => {
				let msg: &str = format!("Unknown VersionedOutgoingActionPayload: {:?}", v).leak();
				Err(scale::Error::from(msg))
			},
		}
	}
}

#[derive(Clone, Eq, PartialEq, Encode, Decode)]
pub enum OutgoingActionPayloadV1 {
	RegisterJob(RegisterJobPayloadV1),
	DeregisterJob(u128),
	FinalizeJob(Vec<u128>),
	SetJobEnvironment(SetJobEnvironmentPayloadV1),
	Noop,
}

#[derive(Clone, Eq, PartialEq, Encode, Decode)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub struct PlannedExecutionV1 {
	pub source: [u8; 32], // AccountId
	pub start_delay: u64,
}

#[derive(Clone, Eq, PartialEq, Encode, Decode)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub struct RegisterJobPayloadV1 {
	pub job_id: u128,
	pub job_registration: JobRegistrationV1,
}

#[derive(Clone, Eq, PartialEq, Encode, Decode)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub struct JobRegistrationV1 {
	pub script: Vec<u8>,
	pub allowed_sources: Option<Vec<[u8; 32]>>, // Vec<AccountId>
	pub allow_only_verified_sources: bool,
	pub schedule: ScheduleV1,
	pub memory: u32,
	pub network_requests: u32,
	pub storage: u32,
	pub required_modules: Vec<u16>,
	pub extra: JobRequirementsV1,
}

#[derive(Copy, Clone, Eq, PartialEq, Encode, Decode)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub struct ScheduleV1 {
	pub duration: u64,
	pub start_time: u64,
	pub end_time: u64,
	pub interval: u64,
	pub max_start_delay: u64,
}

#[derive(Clone, Eq, PartialEq, Encode, Decode)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub struct JobRequirementsV1 {
	pub assignment_strategy: AssignmentStrategyV1,
	pub slots: u8,
	pub reward: u128,
	pub min_reputation: Option<u128>,
}

#[derive(Clone, Eq, PartialEq, Encode, Decode)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub enum AssignmentStrategyV1 {
	Single(Option<Vec<PlannedExecutionV1>>),
	Competing,
}

#[derive(Clone, Eq, PartialEq, Encode, Decode)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub struct SetProcessorJobEnvironmentV1 {
	pub address: [u8; 32], // AccountId
	pub variables: Vec<(Vec<u8>, Vec<u8>)>,
}

#[derive(Clone, Eq, PartialEq, Encode, Decode)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub struct SetJobEnvironmentPayloadV1 {
	pub job_id: u128,
	pub public_key: Vec<u8>,
	pub processors: Vec<SetProcessorJobEnvironmentV1>,
}
