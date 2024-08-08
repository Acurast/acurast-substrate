#![cfg_attr(not(feature = "std"), no_std, no_main)]
#![allow(clippy::arithmetic_side_effects)]

#[ink::contract]
mod proxy {
	use acurast_ibc_ink::ibc::{Layer, Subject};
	#[cfg(feature = "std")]
	use ink::storage::traits::StorageLayout;
	use ink::{
		env::{
			call::{build_call, ExecutionInput},
			hash::Blake2x256,
			DefaultEnvironment,
		},
		prelude::{
			format,
			string::{String, ToString},
			vec::Vec,
		},
		storage::Mapping,
	};

	use scale::{Decode, Encode};

	use acurast_core_ink::types::{
		IncomingAction, IncomingActionPayloadV1, JobRegistrationV1, OutgoingActionPayloadV1,
		RegisterJobPayloadV1, ScheduleV1, SetJobEnvironmentPayloadV1, SetProcessorJobEnvironmentV1,
		Version, VersionedIncomingActionPayload,
	};

	pub type OuterError<T> = Result<Result<T, ink::LangError>, ink::env::Error>;

	#[derive(Clone, Eq, PartialEq, Encode, Decode)]
	#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
	pub struct SetJobEnvironmentProcessor {
		pub address: AccountId,
		pub variables: Vec<(Vec<u8>, Vec<u8>)>,
	}

	#[derive(Clone, Eq, PartialEq, Encode, Decode)]
	#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
	pub struct SetJobEnvironmentUserInput {
		pub job_id: u128,
		pub public_key: Vec<u8>,
		pub processors: Vec<SetJobEnvironmentProcessor>,
	}

	#[derive(Clone, Eq, PartialEq, Encode, Decode)]
	#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
	pub struct RegisterJobUserInput {
		job_registration: JobRegistrationV1,
		destination: AccountId,
		expected_fulfillment_fee: u128,
	}

	#[derive(Clone, Eq, PartialEq, Encode, Decode)]
	#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
	pub enum UserAction {
		RegisterJob(RegisterJobUserInput),
		DeregisterJob(u128),
		FinalizeJob(Vec<u128>),
		SetJobEnvironment(SetJobEnvironmentUserInput),
		Noop,
	}

	#[derive(Clone, Eq, PartialEq, Encode, Decode)]
	pub struct RawOutgoingAction {
		pub id: u64,
		pub origin: AccountId,
		pub payload_version: u16,
		pub payload: Vec<u8>,
	}

	#[derive(Clone, Eq, PartialEq, Decode)]
	pub struct RawIncomingAction {
		id: u64,
		payload_version: u16,
		payload: Vec<u8>,
	}

	fn decode_incoming_action(payload: &Vec<u8>) -> Result<IncomingAction, Error> {
		match IncomingAction::decode(&mut payload.as_slice()) {
			Err(err) => Err(Error::InvalidIncomingAction(format!("{:?}", err))),
			Ok(action) => Ok(action),
		}
	}

	#[derive(Clone, Eq, PartialEq, Encode, Decode)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
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

	#[derive(Clone, Eq, PartialEq, Encode, Decode)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
	pub struct JobInformationV1 {
		schedule: ScheduleV1,
		creator: AccountId,
		destination: AccountId,
		processors: Vec<AccountId>,
		expected_fulfillment_fee: u128,
		remaining_fee: u128,
		maximum_reward: u128,
		status: JobStatus,
		slots: u8,
	}

	#[derive(Clone, Eq, PartialEq, Encode, Decode)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
	pub enum JobInformation {
		V1(JobInformationV1),
	}

	impl JobInformation {
		fn decode(instance: &Proxy, job_id: u128) -> Result<Self, Error> {
			match instance.get_job(job_id)? {
				(Version::V1, job_bytes) => {
					let job =
						JobInformationV1::decode(&mut job_bytes.as_slice()).map_err(|err| {
							Error::Verbose(format!("Cannot decode job information V1 {:?}", err))
						})?;

					Ok(Self::V1(job))
				},
			}
		}
	}

	#[derive(Encode, Decode)]
	#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
	pub enum ConfigureArgument {
		Owner(AccountId),
		IBCContract(AccountId),
		AcurastPalletAccount(AccountId),
		Paused(bool),
		PayloadVersion(u16),
		MaxMessageBytes(u16),
		ExchangeRatio(ExchangeRatio),
		Code(Hash),
	}

	#[ink(event)]
	pub struct IncomingActionProcessed {
		action_id: u64,
	}

	/// Errors returned by the contract's methods.
	#[derive(scale_info::TypeInfo, Debug, PartialEq, Eq, Encode, Decode)]
	pub enum Error {
		UnknownJobVersion(u16),
		JobAlreadyFinished,
		NotJobProcessor,
		UnknownJob,
		ContractPaused,
		NotOwner,
		NotJobCreator,
		CannotFinalizeJob,
		OutgoingActionTooBig,
		Verbose(String),
		InvalidIncomingAction(String),
		/// Error wrappers
		IBCError(acurast_ibc_ink::Error),
		ConsumerError(String),
		LangError(String),
	}

	#[derive(Debug, Clone, Encode, Decode)]
	#[cfg_attr(feature = "std", derive(scale_info::TypeInfo, ink::storage::traits::StorageLayout))]
	pub struct ExchangeRatio {
		pub numerator: u16,
		pub denominator: u16,
	}

	impl ExchangeRatio {
		fn exchange_price(&self, expected_acurast_amount: u128) -> u128 {
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
	#[derive(Debug, Clone, Encode, Decode)]
	#[cfg_attr(feature = "std", derive(scale_info::TypeInfo, ink::storage::traits::StorageLayout))]
	pub struct Config {
		/// Address allowed to manage the contract
		owner: AccountId,
		/// The IBC contract
		ibc: AccountId,
		/// the recipient on Acurast parachain (a pallet account derived from a constant AcurastPalletId)
		acurast_pallet_account: AccountId,
		/// Flag that states if the contract is paused or not
		paused: bool,
		/// Payload versioning
		payload_version: u16,
		/// Maximum size per action
		max_message_bytes: u16,
		/// Exchange ratio ( AZERO / ACU )
		exchange_ratio: ExchangeRatio,
	}

	#[ink(storage)]
	pub struct Proxy {
		config: Config,
		next_outgoing_action_id: u64,
		next_job_id: u128,
		job_info: Mapping<u128, (u16, Vec<u8>)>,
	}

	impl Proxy {
		#[ink(constructor)]
		pub fn new(owner: AccountId, ibc: AccountId) -> Self {
			let mut contract = Self::default();

			contract.config.owner = owner;
			contract.config.ibc = ibc;
			contract
		}

		#[ink(constructor)]
		pub fn default() -> Self {
			Self {
				config: Config {
					owner: AccountId::from([
						24, 90, 139, 95, 146, 236, 211, 72, 237, 155, 18, 160, 71, 202, 43, 40, 72,
						139, 19, 152, 6, 90, 141, 255, 141, 207, 136, 98, 69, 249, 40, 11,
					]),
					ibc: AccountId::from([
						146, 15, 1, 125, 16, 39, 253, 47, 52, 101, 2, 241, 255, 64, 21, 83, 68,
						237, 21, 89, 222, 247, 41, 10, 166, 15, 9, 128, 31, 76, 228, 26,
					]),
					acurast_pallet_account: AccountId::from([
						109, 111, 100, 108, 97, 99, 114, 115, 116, 112, 105, 100, 0, 0, 0, 0, 0, 0,
						0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
					]),
					paused: false,
					payload_version: 1,
					max_message_bytes: 2048,
					exchange_ratio: ExchangeRatio { numerator: 1, denominator: 10 },
				},
				next_outgoing_action_id: 1,
				next_job_id: 1,
				job_info: Mapping::new(),
			}
		}

		fn fail_if_not_owner(&self) -> Result<(), Error> {
			if self.config.owner.eq(&self.env().caller()) {
				Ok(())
			} else {
				Err(Error::NotOwner)
			}
		}

		fn ensure_unpaused(&self) -> Result<(), Error> {
			if self.config.paused {
				Err(Error::ContractPaused)
			} else {
				Ok(())
			}
		}

		fn get_job(&self, job_id: u128) -> Result<(Version, Vec<u8>), Error> {
			if let Some((version, job_bytes)) = self.job_info.get(job_id) {
				match version {
					o if o == Version::V1 as u16 => Ok((Version::V1, job_bytes)),
					v => Err(Error::UnknownJobVersion(v)),
				}
			} else {
				Err(Error::UnknownJob)
			}
		}

		/// Modifies the code which is used to execute calls to this contract.
		pub fn set_code(&mut self, code_hash: Hash) {
			ink::env::set_code_hash::<DefaultEnvironment>(&code_hash).unwrap_or_else(|err| {
				panic!("Failed to `set_code_hash` to {:?} due to {:?}", code_hash, err)
			});
			ink::env::debug_println!("Switched code hash to {:?}.", code_hash);
		}

		#[ink(message)]
		pub fn configure(&mut self, actions: Vec<ConfigureArgument>) -> Result<(), Error> {
			self.fail_if_not_owner()?;

			for action in actions {
				match action {
					ConfigureArgument::Owner(address) => self.config.owner = address,
					ConfigureArgument::IBCContract(address) => self.config.ibc = address,
					ConfigureArgument::AcurastPalletAccount(address) =>
						self.config.acurast_pallet_account = address,
					ConfigureArgument::Paused(paused) => self.config.paused = paused,
					ConfigureArgument::PayloadVersion(version) =>
						self.config.payload_version = version,
					ConfigureArgument::MaxMessageBytes(max_size) =>
						self.config.max_message_bytes = max_size,

					ConfigureArgument::ExchangeRatio(ratio) => self.config.exchange_ratio = ratio,
					ConfigureArgument::Code(code_hash) => self.set_code(code_hash),
				}
			}

			Ok(())
		}

		#[ink(message)]
		pub fn config(&self) -> Config {
			self.config.clone()
		}

		/// This method is called by users to interact with the acurast protocol
		#[ink(message, payable)]
		pub fn send_actions(&mut self, actions: Vec<UserAction>) -> Result<(), Error> {
			// The contract should not be paused
			self.ensure_unpaused()?;

			let caller = self.env().caller();

			for action in actions {
				let outgoing_action = match action {
					UserAction::RegisterJob(payload) => {
						// Increment job identifier
						let job_id = self.next_job_id;
						self.next_job_id += 1;

						// Calculate the number of executions that fit the job schedule
						let start_time = payload.job_registration.schedule.start_time;
						let end_time = payload.job_registration.schedule.end_time;
						let interval = payload.job_registration.schedule.interval;
						if interval == 0 {
							return Err(Error::Verbose("INTERVAL_CANNNOT_BE_ZERO".to_string()))
						}
						let execution_count = ((end_time - start_time - 1) / interval) + 1;

						// Calculate the fee required for all job executions
						let slots = payload.job_registration.extra.slots;
						let expected_fulfillment_fee = payload.expected_fulfillment_fee;
						let expected_fee =
							((slots as u128) * execution_count as u128) * expected_fulfillment_fee;

						// Calculate the total reward required to pay all executions
						let reward_per_execution = payload.job_registration.extra.reward;
						let maximum_reward =
							(slots as u128) * (execution_count as u128) * reward_per_execution;

						// Get exchange price
						let cost: u128 = self.config.exchange_ratio.exchange_price(maximum_reward);

						// Validate job registration payment
						if self.env().transferred_value() < expected_fee + cost {
							return Err(Error::Verbose("AMOUNT_CANNOT_COVER_JOB_COSTS".to_string()))
						}

						let info = JobInformationV1 {
							status: JobStatus::Open,
							creator: self.env().caller(),
							destination: payload.destination,
							processors: Vec::new(),
							expected_fulfillment_fee,
							remaining_fee: expected_fee,
							maximum_reward,
							slots,
							schedule: payload.job_registration.schedule,
						};

						self.job_info
							.insert(self.next_job_id, &(Version::V1 as u16, info.encode()));

						OutgoingActionPayloadV1::RegisterJob(RegisterJobPayloadV1 {
							job_id,
							job_registration: payload.job_registration,
						})
					},
					UserAction::DeregisterJob(job_id) => {
						match JobInformation::decode(self, job_id)? {
							JobInformation::V1(job) => {
								// Only the job creator can deregister the job
								if job.creator != self.env().caller() {
									return Err(Error::NotJobCreator)
								}
							},
						}
						OutgoingActionPayloadV1::DeregisterJob(job_id)
					},
					UserAction::FinalizeJob(ids) => {
						for id in ids.clone() {
							match JobInformation::decode(self, id)? {
								JobInformation::V1(job) => {
									// Only the job creator can finalize the job
									if job.creator != self.env().caller() {
										return Err(Error::NotJobCreator)
									}

									// Verify if job can be finalized
									let is_expired = (job.schedule.end_time / 1000) <
										self.env().block_timestamp();
									if !is_expired {
										return Err(Error::CannotFinalizeJob)
									}
								},
							}
						}

						OutgoingActionPayloadV1::FinalizeJob(ids)
					},
					UserAction::SetJobEnvironment(payload) => {
						match JobInformation::decode(self, payload.job_id)? {
							JobInformation::V1(job) => {
								// Only the job creator can set environment variables
								if job.creator != self.env().caller() {
									return Err(Error::NotJobCreator)
								}
							},
						}
						OutgoingActionPayloadV1::SetJobEnvironment(SetJobEnvironmentPayloadV1 {
							job_id: payload.job_id,
							public_key: payload.public_key,
							processors: payload
								.processors
								.iter()
								.map(|processor| SetProcessorJobEnvironmentV1 {
									address: *processor.address.as_ref(),
									variables: processor.variables.clone(),
								})
								.collect(),
						})
					},
					UserAction::Noop => OutgoingActionPayloadV1::Noop,
				};

				let action = RawOutgoingAction {
					id: self.next_outgoing_action_id,
					origin: caller,
					payload_version: self.config.payload_version,
					payload: outgoing_action.encode(),
				};
				let encoded_action = action.encode();

				// Verify that the encoded action size is less than `max_message_bytes`
				if !encoded_action.len().lt(&(self.config.max_message_bytes as usize)) {
					return Err(Error::OutgoingActionTooBig)
				}

				let call_result: OuterError<acurast_ibc_ink::SendMessageResult> =
					build_call::<DefaultEnvironment>()
						.call(self.config.ibc)
						.call_v1()
						.exec_input(
							ExecutionInput::new(acurast_ibc_ink::SEND_MESSAGE_SELECTOR)
								// nonce
								.push_arg(self.env().hash_encoded::<Blake2x256, _>(
									&action.id.to_ne_bytes().to_vec(),
								))
								// recipient
								.push_arg(&Subject::Acurast(Layer::Extrinsic(
									self.config.acurast_pallet_account,
								)))
								// payload
								.push_arg(&encoded_action)
								//ttl
								.push_arg(100),
						)
						.transferred_value(0)
						.returns()
						.try_invoke();

				match call_result {
					// Errors from the underlying execution environment (e.g the Contracts pallet)
					Err(error) => Err(Error::Verbose(format!("{:?}", error))),
					// Errors from the programming language
					Ok(Err(error)) => Err(Error::LangError(format!("{:?}", error))),
					// Errors emitted by the contract being called
					Ok(Ok(Err(error))) => Err(Error::IBCError(error)),
					// Successful call result
					Ok(Ok(Ok(()))) => {
						// Increment action id
						self.next_outgoing_action_id += 1;

						Ok(())
					},
				}?;
			}

			Ok(())
		}

		/// This method purpose is to receive messages from the acurast protocol.
		#[ink(message)]
		pub fn receive_action(&mut self, payload: Vec<u8>) -> Result<(), Error> {
			// The contract cannot be paused
			self.ensure_unpaused()?;

			let action: IncomingAction = decode_incoming_action(&payload)?;

			// Process action
			match action.payload {
				VersionedIncomingActionPayload::V1(
					IncomingActionPayloadV1::AssignJobProcessor(payload),
				) => {
					match JobInformation::decode(self, payload.job_id)? {
						JobInformation::V1(mut job) => {
							let processor_address = AccountId::from(payload.processor);
							// Update the processor list for the given job
							job.processors.push(processor_address);

							// Send initial fees to the processor (the processor may need a reveal)
							let initial_fee = job.expected_fulfillment_fee;
							job.remaining_fee -= initial_fee;
							// Transfer
							self.env()
								.transfer(processor_address, initial_fee)
								.expect("COULD_NOT_TRANSFER");

							job.status = JobStatus::Assigned;

							// Save changes
							self.job_info
								.insert(payload.job_id, &(Version::V1 as u16, job.encode()));

							Ok(())
						},
					}
				},
				VersionedIncomingActionPayload::V1(IncomingActionPayloadV1::FinalizeJob(
					payload,
				)) => {
					match JobInformation::decode(self, payload.job_id)? {
						JobInformation::V1(mut job) => {
							// Update job status
							job.status = JobStatus::FinalizedOrCancelled;

							assert!(
								payload.unused_reward <= job.maximum_reward,
								"ABOVE_MAXIMUM_REWARD"
							);

							let refund = job.remaining_fee + payload.unused_reward;
							if refund > 0 {
								self.env()
									.transfer(job.creator, refund)
									.expect("COULD_NOT_TRANSFER");
							}

							// Save changes
							self.job_info
								.insert(payload.job_id, &(Version::V1 as u16, job.encode()));

							Ok(())
						},
					}
				},
				VersionedIncomingActionPayload::V1(IncomingActionPayloadV1::Noop) => {
					// Intentionally do nothing
					Ok(())
				},
			}?;

			// Emit event informing that a given incoming message has been processed
			Self::env().emit_event(IncomingActionProcessed { action_id: action.id });

			Ok(())
		}

		#[ink(message)]
		pub fn fulfill(&mut self, job_id: u128, payload: Vec<u8>) -> Result<(), Error> {
			self.ensure_unpaused()?;

			match JobInformation::decode(self, job_id)? {
				JobInformation::V1(mut job) => {
					// Verify if sender is assigned to the job
					if !job.processors.contains(&self.env().caller()) {
						return Err(Error::NotJobProcessor)
					}

					// Verify that the job has not been finalized
					if job.status != JobStatus::Assigned {
						return Err(Error::JobAlreadyFinished)
					}

					// Re-fill processor fees
					// Forbidden to credit 0ꜩ to a contract without code.
					let has_funds = job.remaining_fee >= job.expected_fulfillment_fee;
					let next_execution_fee = if has_funds && job.expected_fulfillment_fee > 0 {
						job.remaining_fee -= job.expected_fulfillment_fee;

						job.expected_fulfillment_fee
					} else {
						0
					};

					// Pass the fulfillment to the destination contract
					let call_result: OuterError<acurast_consumer_ink::FulfillReturn> =
						build_call::<DefaultEnvironment>()
							.call(job.destination)
							.call_v1()
							.exec_input(
								ExecutionInput::new(acurast_consumer_ink::FULFILL_SELECTOR)
									.push_arg(job_id)
									.push_arg(payload),
							)
							.transferred_value(next_execution_fee)
							.returns()
							.try_invoke();

					match call_result {
						// Errors from the underlying execution environment (e.g the Contracts pallet)
						Err(error) => Err(Error::Verbose(format!("{:?}", error))),
						// Errors from the programming language
						Ok(Err(error)) => Err(Error::LangError(format!("{:?}", error))),
						// Errors emitted by the contract being called
						Ok(Ok(Err(error))) => Err(Error::ConsumerError(error)),
						// Successful call result
						Ok(Ok(Ok(()))) => {
							// Save changes
							self.job_info.insert(job_id, &(Version::V1 as u16, job.encode()));

							Ok(())
						},
					}
				},
			}
		}

        #[ink(message)]
        pub fn job(&self, job_id: u128) -> Result<JobInformation, Error> {
            JobInformation::decode(self, job_id)
        }

        #[ink(message)]
		pub fn next_job_id(&self) -> u128 {
			self.next_job_id
		}
	}

	#[cfg(test)]
	mod tests {
		use hex_literal::hex;

		/// Imports all the definitions from the outer scope so we can use them here.
		use super::*;

		#[ink::test]
		fn test_action_encoding() {
			let encoded_incoming_action = hex!("00000000000000000002");

			let decoded_incoming_action =
				IncomingAction::decode(&mut encoded_incoming_action.as_slice());

			assert_eq!(
				decoded_incoming_action.unwrap(),
				IncomingAction {
					id: 0,
					payload: VersionedIncomingActionPayload::V1(IncomingActionPayloadV1::Noop)
				}
			);
		}
	}
}
