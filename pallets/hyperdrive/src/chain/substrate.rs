#![cfg_attr(all(feature = "alloc", not(feature = "std"), not(test)), no_std)]

use core::marker::PhantomData;
use derive_more::Display;
use frame_support::pallet_prelude::*;
use parity_scale_codec::{Decode, Encode};
use scale_info::TypeInfo;
use sp_io::hashing::blake2_256;

use sp_core::{bounded::BoundedVec, RuntimeDebug};
use sp_std::prelude::*;

use pallet_acurast::{
	AllowedSources, Environment, JobModule, JobModules, JobRegistration, MultiOrigin, Schedule,
	Script,
};
use pallet_acurast_marketplace::{
	AssignmentStrategy, JobRequirements, PlannedExecution, PlannedExecutions, PubKey, PubKeyBytes,
	RegistrationExtra,
};

use crate::{IncomingAction, Message, MessageDecoder, MessageEncoder, ParsedAction};
use acurast_core_ink::types::{
	AssignProcessorPayloadV1, AssignmentStrategyV1, FinalizeJobPayloadV1,
	IncomingAction as IncomingActionOnProxy, IncomingActionPayloadV1, OutgoingAction,
	OutgoingActionPayloadV1 as ActionPayloadV1, PlannedExecutionV1, VersionedIncomingActionPayload,
	VersionedOutgoingActionPayload,
};

#[derive(RuntimeDebug, Encode, Decode, TypeInfo, Clone, Eq, PartialEq)]
#[scale_info(skip_type_params(AccountConverter))]
pub struct SubstrateMessageDecoder<I, AccountConverter, AccountId> {
	#[cfg(any(test, feature = "runtime-benchmarks"))]
	pub marker: PhantomData<(I, AccountConverter, AccountId)>,
	#[cfg(not(any(test, feature = "runtime-benchmarks")))]
	marker: PhantomData<(I, AccountConverter, AccountId)>,
}

/// Errors returned by this crate.
#[derive(RuntimeDebug, Encode, Decode, MaxEncodedLen, TypeInfo, Clone, Eq, PartialEq, Display)]
pub enum SubstrateMessageDecoderError {
	CouldNotDecodeAction,
	TooManyPlannedExecutions,
	TooManyAllowedSources,
	InvalidJobModule,
	TooManyJobModules,
	CouldNotConvertAccountId,
}

impl<T, I: 'static, AccountConverter> MessageDecoder<T>
	for SubstrateMessageDecoder<I, AccountConverter, T::AccountId>
where
	T: crate::pallet::Config<I>,
	T::RegistrationExtra: From<
		RegistrationExtra<
			T::Balance,
			T::AccountId,
			T::MaxSlots,
			T::ProcessorVersion,
			T::MaxVersions,
		>,
	>,
	AccountConverter: TryFrom<Vec<u8>> + Into<T::AccountId>,
{
	type Error = SubstrateMessageDecoderError;

	fn decode(encoded: &[u8]) -> Result<ParsedAction<T>, Self::Error> {
		let action =
			OutgoingAction::decode(encoded).map_err(|_err| Self::Error::CouldNotDecodeAction)?;

		let origin = MultiOrigin::AlephZero(convert_account_id::<T::AccountId, AccountConverter>(
			&action.origin,
		)?);

		fn convert_account_id<Account, AccountConverter: TryFrom<Vec<u8>> + Into<Account>>(
			bytes: &[u8; 32],
		) -> Result<Account, SubstrateMessageDecoderError> {
			let parsed: AccountConverter = bytes
				.to_vec()
				.try_into()
				.map_err(|_| SubstrateMessageDecoderError::CouldNotConvertAccountId)?;
			Ok(parsed.into())
		}

		let parsed_action: ParsedAction<T> = match action.payload {
			VersionedOutgoingActionPayload::V1(action) => match action {
				ActionPayloadV1::RegisterJob(job_payload) => {
					let j = job_payload.job_registration;

					let assignment_strategy = match j.extra.assignment_strategy {
						AssignmentStrategyV1::Single(executions) =>
							AssignmentStrategy::Single(if let Some(e) = executions {
								Some(
									PlannedExecutions::try_from(
										e.into_iter()
											.map(|m: PlannedExecutionV1| {
												Ok(PlannedExecution {
													source: convert_account_id::<
														T::AccountId,
														AccountConverter,
													>(&m.source)?,
													start_delay: m.start_delay,
												})
											})
											.collect::<Result<
												Vec<PlannedExecution<T::AccountId>>,
												Self::Error,
											>>()?,
									)
									.map_err(|_| Self::Error::TooManyPlannedExecutions)?,
								)
							} else {
								None
							}),
						AssignmentStrategyV1::Competing => AssignmentStrategy::Competing,
					};
					let extra: T::RegistrationExtra = RegistrationExtra {
						requirements: JobRequirements {
							assignment_strategy,
							slots: j.extra.slots.into(),
							reward: T::Balance::from(j.extra.reward),
							min_reputation: j.extra.min_reputation,
							processor_version: None,
							min_cpu_score: None,
						},
					}
					.into();
					let allowed_sources = if let Some(a) = j.allowed_sources {
						Some(
							AllowedSources::try_from(
								a.iter()
									.map(|s| {
										convert_account_id::<T::AccountId, AccountConverter>(s)
									})
									.collect::<Result<Vec<T::AccountId>, Self::Error>>()?,
							)
							.map_err(|_| Self::Error::TooManyAllowedSources)?,
						)
					} else {
						None
					};
					let registration = JobRegistration {
						script: Script::truncate_from(j.script),
						allowed_sources,
						allow_only_verified_sources: j.allow_only_verified_sources,
						schedule: Schedule {
							duration: j.schedule.duration,
							start_time: j.schedule.start_time,
							end_time: j.schedule.end_time,
							interval: j.schedule.interval,
							max_start_delay: j.schedule.max_start_delay,
						},
						memory: j.memory,
						network_requests: j.network_requests,
						storage: j.storage,
						required_modules: JobModules::try_from(
							j.required_modules
								.iter()
								.map(|item| {
									Ok(JobModule::try_from(*item as u32)
										.map_err(|_| Self::Error::InvalidJobModule)?)
								})
								.collect::<Result<Vec<_>, Self::Error>>()?,
						)
						.map_err(|_| Self::Error::TooManyJobModules)?,
						extra,
					};

					let job_id = (origin, job_payload.job_id);

					ParsedAction::RegisterJob(job_id, registration)
				},
				ActionPayloadV1::DeregisterJob(job_id) =>
					ParsedAction::DeregisterJob((origin, job_id as u128)),
				ActionPayloadV1::FinalizeJob(payload) => ParsedAction::FinalizeJob(
					payload.iter().map(|id| (origin.clone(), *id as u128)).collect(),
				),
				ActionPayloadV1::SetJobEnvironment(payload) => {
					let job_id = (origin, payload.job_id as u128);

					let variables = payload
						.processors
						.iter()
						.map(|processor| {
							let processor_address = convert_account_id::<
								T::AccountId,
								AccountConverter,
							>(&processor.address)?;
							let env = Environment {
								public_key: BoundedVec::truncate_from(payload.public_key.clone()),
								variables: BoundedVec::truncate_from(
									processor
										.variables
										.iter()
										.map(|(key, value)| {
											(
												BoundedVec::truncate_from(key.clone()),
												BoundedVec::truncate_from(value.clone()),
											)
										})
										.collect(),
								),
							};

							Ok((processor_address, env))
						})
						.collect::<Result<Vec<_>, Self::Error>>()?;

					ParsedAction::SetJobEnvironment(job_id, BoundedVec::truncate_from(variables))
				},
				ActionPayloadV1::Noop => ParsedAction::Noop,
			},
		};

		Ok(parsed_action)
	}
}

#[derive(RuntimeDebug, Encode, Decode, MaxEncodedLen, TypeInfo, Clone, Eq, PartialEq, Display)]
pub enum SubstrateMessageEncoderError {
	UnexpectedPublicKey,
}

pub struct SubstrateMessageEncoder;

impl MessageEncoder for SubstrateMessageEncoder {
	type Error = SubstrateMessageEncoderError;

	/// Encodes the given message for Substrate.
	fn encode(message: &Message) -> Result<Vec<u8>, Self::Error> {
		let payload = match &message.action {
			IncomingAction::AssignJob(job_id, processor_public_key) => {
				let address_bytes = match processor_public_key {
					PubKey::SECP256k1(pk) => public_key_to_address_bytes(pk),
					_ => Err(Self::Error::UnexpectedPublicKey)?,
				};

				let payload =
					AssignProcessorPayloadV1 { job_id: *job_id, processor: address_bytes };
				IncomingActionPayloadV1::AssignJobProcessor(payload)
			},
			IncomingAction::FinalizeJob(job_id, refund_amount) => {
				let payload =
					FinalizeJobPayloadV1 { job_id: *job_id, unused_reward: *refund_amount };

				IncomingActionPayloadV1::FinalizeJob(payload)
			},
			IncomingAction::Noop => IncomingActionPayloadV1::Noop,
		};
		let message = IncomingActionOnProxy {
			id: message.id,
			payload: VersionedIncomingActionPayload::V1(payload),
		};

		Ok(message.encode())
	}
}

/// Helper function to covert the BoundedVec [`PubKeyBytes`] to an Substrate address.
pub fn public_key_to_address_bytes(pub_key: &PubKeyBytes) -> [u8; 32] {
	let account_id_bytes = blake2_256(pub_key);

	account_id_bytes
}
