use gstd::{exec, msg};
use sails_rs::calls::Call;
use sails_rs::gstd::calls::GStdRemoting;
use sails_rs::prelude::*;
use vara_ibc_client::traits::VaraIbc;
use vara_ibc_client::{Subject, Layer};

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

	async fn do_send_actions(actions: Vec<UserAction>) -> Result<(), ProxyError> {
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
						return Err(ProxyError::Verbose(
							"AMOUNT_CANNOT_COVER_JOB_COSTS".to_string(),
						));
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

					Storage::job_info().insert(job_id, (Version::V1 as u16, info.encode()));

					OutgoingActionPayloadV1::RegisterJob(RegisterJobPayloadV1 {
						job_id,
						job_registration: payload.job_registration,
					})
				},
				UserAction::DeregisterJob(job_id) => {
					match JobInformation::from_id(job_id)? {
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
						match JobInformation::from_id(id)? {
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
					match JobInformation::from_id(payload.job_id)? {
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
								address: processor.address.into(),
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
			if !encoded_action.len().lt(&(Storage::config().max_message_bytes as usize)) {
				return Err(ProxyError::OutgoingActionTooBig);
			}

			let mut ibc = vara_ibc_client::VaraIbc::new(GStdRemoting);
			ibc.send_message(
				blake2_256(action.id.to_ne_bytes().as_slice()),
				Subject::Acurast(Layer::Extrinsic(Storage::config().acurast_pallet_account)),
				encoded_action,
				100,
			)
			.send(Storage::config().ibc)
			.await
			.map_err(|_| ProxyError::IbcFailed)?;

			let _ = Storage::get_and_increase_next_outgoing_action_id();
		}

		Ok(())
	}

	fn decode_incoming_action(payload: &Vec<u8>) -> Result<IncomingAction, ProxyError> {
		match IncomingAction::decode(&mut payload.as_slice()) {
			Err(err) => Err(ProxyError::InvalidIncomingAction(format!("{:?}", err))),
			Ok(action) => Ok(action),
		}
	}

	fn do_receive_action(payload: Vec<u8>) -> Result<(), ProxyError> {
		let action: IncomingAction = Self::decode_incoming_action(&payload)?;

		// Process action
		match action.payload {
			VersionedIncomingActionPayload::V1(IncomingActionPayloadV1::AssignJobProcessor(
				payload,
			)) => {
				match JobInformation::from_id(payload.job_id)? {
					JobInformation::V1(mut job) => {
						let processor_address = AccountId::from(payload.processor);
						// Update the processor list for the given job
						job.processors.push(processor_address);

						// Send initial fees to the processor (the processor may need a reveal)
						let initial_fee = job.expected_fulfillment_fee;
						job.remaining_fee -= initial_fee;
						// Transfer
						msg::send(processor_address, (), initial_fee).expect("COULD_NOT_TRANSFER");

						job.status = JobStatus::Assigned;

						// Save changes
						Storage::job_info()
							.insert(payload.job_id, (Version::V1 as u16, job.encode()));

						Ok(())
					},
				}
			},
			VersionedIncomingActionPayload::V1(IncomingActionPayloadV1::FinalizeJob(payload)) => {
				match JobInformation::from_id(payload.job_id)? {
					JobInformation::V1(mut job) => {
						// Update job status
						job.status = JobStatus::FinalizedOrCancelled;

						assert!(
							payload.unused_reward <= job.maximum_reward,
							"ABOVE_MAXIMUM_REWARD"
						);

						let refund = job.remaining_fee + payload.unused_reward;
						if refund > 0 {
							msg::send(job.creator, (), refund).expect("COULD_NOT_TRANSFER");
						}

						// Save changes
						Storage::job_info()
							.insert(payload.job_id, (Version::V1 as u16, job.encode()));

						Ok(())
					},
				}
			},
			VersionedIncomingActionPayload::V1(IncomingActionPayloadV1::Noop) => {
				// Intentionally do nothing
				Ok(())
			},
		}?;

		Ok(())
	}

	fn do_fulfill(job_id: u128, payload: Vec<u8>) -> Result<(), ProxyError> {
		match JobInformation::from_id(job_id)? {
			JobInformation::V1(mut job) => {
				let processor_address = msg::source();

				// Verify if sender is assigned to the job
				if !job.processors.contains(&processor_address) {
					return Err(ProxyError::NotJobProcessor);
				}

				// Verify that the job has not been finalized
				if job.status != JobStatus::Assigned {
					return Err(ProxyError::JobAlreadyFinished);
				}

				// Re-fill processor fees
				// Forbidden to credit 0 to a contract without code.
				let has_funds = job.remaining_fee >= job.expected_fulfillment_fee;
				if has_funds && job.expected_fulfillment_fee > 0 {
					job.remaining_fee -= job.expected_fulfillment_fee;
					// Transfer
					msg::send(processor_address, (), job.expected_fulfillment_fee)
						.expect("COULD_NOT_TRANSFER");
					// Save changes
					Storage::job_info().insert(job_id, (Version::V1 as u16, job.encode()));
				}

				msg::send(job.destination, payload, 0)
					.map_err(|error| ProxyError::ConsumerError(format!("{:?}", error)))?;
			},
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

	pub async fn send_actions(&mut self, actions: Vec<UserAction>) {
		panicking(Self::ensure_unpaused);

		let result = Self::do_send_actions(actions).await;

		panicking(|| result);
	}

	pub fn receive_action(&mut self, payload: Vec<u8>) {
		panicking(Self::ensure_unpaused);

		panicking(|| Self::do_receive_action(payload));
	}

	pub fn fulfill(&mut self, job_id: u128, payload: Vec<u8>) {
		panicking(Self::ensure_unpaused);

		panicking(|| Self::do_fulfill(job_id, payload));
	}

	pub fn job(&self, job_id: u128) -> JobInformation {
		panicking(|| JobInformation::from_id(job_id))
	}

	pub fn next_job_id(&self) -> u128 {
		Storage::next_job_id()
	}
}
