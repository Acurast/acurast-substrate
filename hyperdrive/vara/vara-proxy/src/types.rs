pub use acurast_hyperdrive_substrate_core::types::*;
use sails_rs::prelude::{scale_codec::*, *};

use crate::storage::*;
use crate::utils::ProxyError;

pub type AccountId = ActorId;

#[derive(Clone, Eq, PartialEq, Encode, Decode, TypeInfo)]
pub struct SetJobEnvironmentProcessor {
	pub address: AccountId,
	pub variables: Vec<(Vec<u8>, Vec<u8>)>,
}

#[derive(Clone, Eq, PartialEq, Encode, Decode, TypeInfo)]
pub struct SetJobEnvironmentUserInput {
	pub job_id: u128,
	pub public_key: Vec<u8>,
	pub processors: Vec<SetJobEnvironmentProcessor>,
}

#[derive(Clone, Eq, PartialEq, Encode, Decode, TypeInfo)]
pub struct RegisterJobUserInput {
	pub job_registration: JobRegistrationV1,
	pub destination: AccountId,
	pub expected_fulfillment_fee: u128,
}

#[derive(Clone, Eq, PartialEq, Encode, Decode, TypeInfo)]
pub enum UserAction {
	RegisterJob(RegisterJobUserInput),
	DeregisterJob(u128),
	FinalizeJob(Vec<u128>),
	SetJobEnvironment(SetJobEnvironmentUserInput),
	Noop,
}

#[derive(Clone, Eq, PartialEq, Encode, Decode, TypeInfo)]
pub struct RawOutgoingAction {
	pub id: u64,
	pub origin: AccountId,
	pub payload_version: u16,
	pub payload: Vec<u8>,
}

#[derive(Clone, Eq, PartialEq, Decode, TypeInfo)]
pub struct RawIncomingAction {
	pub id: u64,
	pub payload_version: u16,
	pub payload: Vec<u8>,
}

#[derive(Clone, Eq, PartialEq, Encode, Decode, TypeInfo)]
pub enum JobStatus {
	/// Status after a job got registered.
	Open = 0,
	/// Status after a valid match for a job got submitted.
	Matched = 1,
	/// Status after all processors have acknowledged the job.
	Assigned = 2,
	/// Status when a job has been finalized or cancelled
	FinalizedOrCancelled = 3,
}

#[derive(Clone, Eq, PartialEq, Encode, Decode, TypeInfo)]
pub struct JobInformationV1 {
	pub schedule: ScheduleV1,
	pub creator: AccountId,
	pub destination: AccountId,
	pub processors: Vec<AccountId>,
	pub expected_fulfillment_fee: u128,
	pub remaining_fee: u128,
	pub maximum_reward: u128,
	pub status: JobStatus,
	pub slots: u8,
}

#[derive(Clone, Eq, PartialEq, Encode, Decode, TypeInfo)]
pub enum JobInformation {
	V1(JobInformationV1),
}

impl JobInformation {
	pub fn from_id(job_id: u128) -> Result<Self, ProxyError> {
		match Storage::get_job(job_id)? {
			(Version::V1, job_bytes) => {
				let job = JobInformationV1::decode(&mut job_bytes.as_slice()).map_err(|err| {
					ProxyError::Verbose(format!("Cannot decode job information V1 {:?}", err))
				})?;

				Ok(Self::V1(job))
			},
		}
	}
}

#[derive(Encode, Decode, TypeInfo)]
pub enum ConfigureArgument {
	Owner(AccountId),
	IBCContract(AccountId),
	AcurastPalletAccount(AccountId),
	Paused(bool),
	PayloadVersion(u16),
	MaxMessageBytes(u16),
	ExchangeRatio(ExchangeRatio),
}

#[derive(Debug, Clone, Encode, Decode, TypeInfo, Default)]
pub struct ExchangeRatio {
	pub numerator: u16,
	pub denominator: u16,
}

impl ExchangeRatio {
	pub fn exchange_price(&self, expected_acurast_amount: u128) -> u128 {
		// Calculate how many azero is required to cover for the job cost
		let n = (self.numerator as u128) * expected_acurast_amount;
		let d = self.denominator as u128;

		if n % d == 0 {
			n / d
		} else {
			n / d + 1
		}
	}
}

/// Contract configurations are contained in this structure
#[derive(Debug, Clone, Encode, Decode, TypeInfo, Default)]
pub struct Config {
	/// Address allowed to manage the contract
	pub owner: ActorId,
	/// The IBC contract
	pub ibc: ActorId,
	/// the recipient on Acurast parachain (a pallet account derived from a constant AcurastPalletId)
	pub acurast_pallet_account: AccountId,
	/// Flag that states if the contract is paused or not
	pub paused: bool,
	/// Payload versioning
	pub payload_version: u16,
	/// Maximum size per action
	pub max_message_bytes: u16,
	/// Exchange ratio ( AZERO / ACU )
	pub exchange_ratio: ExchangeRatio,
}
