use frame_support::{pallet_prelude::*, storage::bounded_vec::BoundedVec};
use pallet_acurast_marketplace::PubKey;
use parity_scale_codec::{Decode, Encode};
use scale_info::TypeInfo;
use sp_core::ConstU32;
use sp_runtime::RuntimeDebug;
use sp_std::prelude::*;
use strum_macros::{EnumString, IntoStaticStr};

use pallet_acurast::{EnvironmentFor, JobId, JobIdSequence, JobRegistration};

pub const PROXY_ADDRESS_MAX_LENGTH: u32 = 64;
/// The Acurast proxy address on the target chain, such as a ink contract address.
pub type ProxyAddress = BoundedVec<u8, ConstU32<PROXY_ADDRESS_MAX_LENGTH>>;

pub enum ProxyChain {
	AlephZero,
	Vara,
	// Tezos,
	// Ethereum,
}

#[derive(
	RuntimeDebug, Encode, Decode, TypeInfo, Clone, Eq, PartialEq, EnumString, IntoStaticStr,
)]
pub enum RawIncomingAction {
	#[strum(serialize = "REGISTER_JOB")]
	RegisterJob,
	#[strum(serialize = "DEREGISTER_JOB")]
	DeregisterJob,
	#[strum(serialize = "FINALIZE_JOB")]
	FinalizeJob,
	#[strum(serialize = "SET_JOB_ENVIRONMENT")]
	SetJobEnvironment,
	#[strum(serialize = "NOOP")]
	Noop = 255,
}

/// Convert an index to a RawIncomingAction
impl TryFrom<u16> for RawIncomingAction {
	type Error = Vec<u8>;

	fn try_from(value: u16) -> Result<Self, Self::Error> {
		match value {
			o if o == RawIncomingAction::RegisterJob as u16 => Ok(RawIncomingAction::RegisterJob),
			o if o == RawIncomingAction::DeregisterJob as u16 => {
				Ok(RawIncomingAction::DeregisterJob)
			},
			o if o == RawIncomingAction::FinalizeJob as u16 => Ok(RawIncomingAction::FinalizeJob),
			o if o == RawIncomingAction::SetJobEnvironment as u16 => {
				Ok(RawIncomingAction::SetJobEnvironment)
			},
			o if o == RawIncomingAction::Noop as u16 => Ok(RawIncomingAction::Noop),
			_ => Err(b"Unknown action index".to_vec()),
		}
	}
}

impl<T: pallet_acurast::Config> From<&ParsedAction<T>> for RawIncomingAction {
	fn from(action: &ParsedAction<T>) -> Self {
		match action {
			ParsedAction::RegisterJob(_, _) => RawIncomingAction::RegisterJob,
			ParsedAction::DeregisterJob(_) => RawIncomingAction::DeregisterJob,
			ParsedAction::FinalizeJob(_) => RawIncomingAction::FinalizeJob,
			ParsedAction::SetJobEnvironment(_, _) => RawIncomingAction::SetJobEnvironment,
			ParsedAction::Noop => RawIncomingAction::Noop,
		}
	}
}

#[derive(RuntimeDebug, Encode, Decode, TypeInfo, Clone, PartialEq)]
#[scale_info(skip_type_params(T))]
pub enum ParsedAction<T: pallet_acurast::Config> {
	RegisterJob(
		JobId<T::AccountId>,
		JobRegistration<T::AccountId, T::MaxAllowedSources, T::RegistrationExtra>,
	),
	DeregisterJob(JobId<T::AccountId>),
	FinalizeJob(Vec<JobId<T::AccountId>>),
	SetJobEnvironment(
		JobId<T::AccountId>,
		BoundedVec<(T::AccountId, EnvironmentFor<T>), T::MaxSlots>,
	),
	Noop,
}

pub type JobRegistrationFor<T> = JobRegistration<
	<T as frame_system::Config>::AccountId,
	<T as pallet_acurast::Config>::RegistrationExtra,
	<T as pallet_acurast::Config>::MaxAllowedSources,
>;

pub trait MessageDecoder<T: pallet_acurast::Config> {
	type Error;

	fn decode(encoded: &[u8], chain: ProxyChain) -> Result<ParsedAction<T>, Self::Error>;
}

pub trait MessageEncoder {
	type Error;

	fn encode(message: &Message) -> Result<Vec<u8>, Self::Error>;
}

pub trait ActionExecutor<T: pallet_acurast::Config> {
	fn execute(action: ParsedAction<T>) -> DispatchResultWithPostInfo;
}

/// Tracks the progress during `submit_message`, intended to be included in events.
#[derive(RuntimeDebug, Encode, Decode, TypeInfo, Clone, PartialEq)]
pub enum ProcessMessageResult {
	ParsingValueFailed,
	ActionFailed(RawIncomingAction),
	ActionSuccess,
	ProcessingFailed(DispatchError),
}

impl From<DispatchError> for ProcessMessageResult {
	fn from(value: DispatchError) -> Self {
		ProcessMessageResult::ProcessingFailed(value)
	}
}

/// Message that is transferred to target chains.
#[derive(RuntimeDebug, Encode, Decode, TypeInfo, Eq, PartialEq, Clone)]
pub struct Message {
	pub id: u64,
	pub action: IncomingAction,
}

/// The encodable version of an [`Action`].
#[derive(
	RuntimeDebug, Encode, Decode, TypeInfo, Clone, Eq, PartialEq, EnumString, IntoStaticStr,
)]
pub enum RawOutgoingAction {
	#[strum(serialize = "ASSIGN_JOB_PROCESSOR")]
	AssignJob,
	#[strum(serialize = "FINALIZE_JOB")]
	FinalizeJob,
	#[strum(serialize = "NOOP")]
	Noop = 255,
}

impl From<&IncomingAction> for RawOutgoingAction {
	fn from(action: &IncomingAction) -> Self {
		match action {
			IncomingAction::AssignJob(_, _) => RawOutgoingAction::AssignJob,
			IncomingAction::FinalizeJob(_, _) => RawOutgoingAction::FinalizeJob,
			IncomingAction::Noop => RawOutgoingAction::Noop,
		}
	}
}

/// Convert [RawOutgoingAction] to an index
impl Into<u16> for RawOutgoingAction {
	fn into(self: Self) -> u16 {
		self as u16
	}
}

/// The action is triggered in Acurast Proxy on target chain upon a hyperdrive message.
#[derive(RuntimeDebug, Encode, Decode, TypeInfo, Eq, PartialEq, Clone)]
pub enum IncomingAction {
	/// Assigns a job on target chain.
	///
	/// Consists of `(Job ID, processor address)`,
	/// where `Job ID` is the subset of [`pallet_acurast::JobId`] for jobs created externally.
	AssignJob(JobIdSequence, PubKey), // (u128, public_key)
	/// Finalizes a job on target chain.
	///
	/// Consists of `(Job ID, refund amount)`,
	/// where `Job ID` is the subset of [`pallet_acurast::JobId`] for jobs created externally.
	FinalizeJob(JobIdSequence, u128), // (u128, u128)
	/// A noop action that solely suits the purpose of testing that messages get sent.
	Noop,
}
