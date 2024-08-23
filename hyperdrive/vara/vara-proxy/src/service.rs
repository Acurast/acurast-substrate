use gstd::{exec, msg, BlockNumber};
use sails_rs::prelude::*;

use crate::storage::*;
use crate::types::*;
use crate::utils::*;

#[derive(Default)]
pub struct VaraProxyService();

impl VaraProxyService {
	pub fn init(owner: ActorId, ibc: ActorId) -> Self {
		unsafe {
			STORAGE = Some(Storage::default());
		}
		Storage::config().owner = owner;
		Storage::config().ibc = ibc;
		Self()
	}

	fn ensure_owner() -> Result<(), ProxyError> {
		let config = Storage::config();
		if config.owner.eq(&msg::source()) {
			Ok(())
		} else {
			Err(ProxyError::NotOwner)
		}
	}

	fn ensure_unpaused() -> Result<(), ProxyError> {
		let config = Storage::config();
		if config.paused {
			Err(ProxyError::ContractPaused)
		} else {
			Ok(())
		}
	}

	pub fn do_send_actions(actions: Vec<UserAction>) -> Result<(), ProxyError> {
		let caller = msg::source();

		for action in actions {
			let outgoing_action = match action {
				UserAction::RegisterJob(payload) => {
					// Increment job identifier
					let job_id = Storage::get_and_increase_next_job_id();

					// Calculate the number of executions that fit the job schedule
					let start_time = payload.job_registration.schedule.start_time;
					let end_time = payload.job_registration.schedule.end_time;
					let interval = payload.job_registration.schedule.interval;
					if interval == 0 {
						return Err(ProxyError::Verbose("INTERVAL_CANNNOT_BE_ZERO".to_string()));
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
					let cost: u128 =
						Storage::config().exchange_ratio.exchange_price(maximum_reward);

					// Validate job registration payment
					let amount = msg::value();
					if amount < expected_fee + cost {
						return Err(Error::Verbose("AMOUNT_CANNOT_COVER_JOB_COSTS".to_string()));
					}

					let info = JobInformationV1 {
						status: JobStatus::Open,
						creator: caller,
						destination: payload.destination,
						processors: Vec::new(),
						expected_fulfillment_fee,
						remaining_fee: expected_fee,
						maximum_reward,
						slots,
						schedule: payload.job_registration.schedule,
					};

					Storage::job_info().insert(job_id, &(Version::V1 as u16, info.encode()));

					OutgoingActionPayloadV1::RegisterJob(RegisterJobPayloadV1 {
						job_id,
						job_registration: payload.job_registration,
					})
				},
				UserAction::DeregisterJob(job_id) => {
					match JobInformation::from(job_id)? {
						JobInformation::V1(job) => {
							// Only the job creator can deregister the job
							if job.creator != caller {
								return Err(ProxyError::NotJobCreator);
							}
						},
					}
					OutgoingActionPayloadV1::DeregisterJob(job_id)
				},
				UserAction::FinalizeJob(ids) => {
					for id in ids.clone() {
						match JobInformation::from(id)? {
							JobInformation::V1(job) => {
								// Only the job creator can finalize the job
								if job.creator != caller {
									return Err(ProxyError::NotJobCreator);
								}

								// Verify if job can be finalized
								let is_expired =
									(job.schedule.end_time / 1000) < exec::block_timestamp();
								if !is_expired {
									return Err(ProxyError::CannotFinalizeJob);
								}
							},
						}
					}

					OutgoingActionPayloadV1::FinalizeJob(ids)
				},
				UserAction::SetJobEnvironment(payload) => {
					match JobInformation::from(payload.job_id)? {
						JobInformation::V1(job) => {
							// Only the job creator can set environment variables
							if job.creator != caller {
								return Err(ProxyError::NotJobCreator);
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
				id: Storage::next_outgoing_action_id(),
				origin: caller,
				payload_version: Storage::config().payload_version,
				payload: outgoing_action.encode(),
			};
			let encoded_action = action.encode();

			// Verify that the encoded action size is less than `max_message_bytes`
			if !encoded_action.len().lt(&(self.config.max_message_bytes as usize)) {
				return Err(Error::OutgoingActionTooBig);
			}

			let call_result: OuterError<acurast_ibc_ink::SendMessageResult> = build_call::<
				DefaultEnvironment,
			>()
			.call(self.config.ibc)
			.call_v1()
			.exec_input(
				ExecutionInput::new(acurast_ibc_ink::SEND_MESSAGE_SELECTOR)
					// nonce
					.push_arg(
						self.env().hash_encoded::<Blake2x256, _>(&action.id.to_ne_bytes().to_vec()),
					)
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
}

#[sails_rs::service]
impl VaraProxyService {
	pub fn new() -> Self {
		Self()
	}

	pub fn configure(&mut self, actions: Vec<ConfigureArgument>) {
		panicking(Self::ensure_owner);

		let config = Storage::config();

		for action in actions {
			match action {
				ConfigureArgument::Owner(address) => config.owner = address,
				ConfigureArgument::IBCContract(address) => config.ibc = address,
				ConfigureArgument::AcurastPalletAccount(address) => {
					config.acurast_pallet_account = address
				},
				ConfigureArgument::Paused(paused) => config.paused = paused,
				ConfigureArgument::PayloadVersion(version) => config.payload_version = version,
				ConfigureArgument::MaxMessageBytes(max_size) => config.max_message_bytes = max_size,
				ConfigureArgument::ExchangeRatio(ratio) => config.exchange_ratio = ratio,
			}
		}
	}

	pub fn config(&self) -> &'static Config {
		Storage::config()
	}

	pub fn send_actions() {
		panicking(Self::ensure_unpaused);
	}
}
