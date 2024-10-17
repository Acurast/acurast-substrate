use gstd::msg;
use sails_rs::prelude::*;

use crate::storage::*;
use crate::types::*;
use crate::utils::*;

#[derive(Default)]
pub struct VaraConsumerService();

impl VaraConsumerService {
	pub fn init(owner: Option<ActorId>, proxy: ActorId) -> Self {
		unsafe {
			STORAGE = Some(Storage::default());
		}
		Storage::config().owner = owner.unwrap_or(msg::source());
		Storage::config().proxy = proxy;
		Self()
	}

	fn ensure_owner() -> Result<(), ConsumerError> {
		let config = Storage::config();
		if config.owner.eq(&msg::source()) {
			Ok(())
		} else {
			Err(ConsumerError::NotOwner)
		}
	}
}

#[sails_rs::service(events = Event)]
impl VaraConsumerService {
	pub fn new() -> Self {
		Self()
	}

	pub fn configure(&mut self, actions: Vec<ConfigureArgument>) {
		panicking(Self::ensure_owner);

		let config = Storage::config();

		for action in actions {
			match action {
				ConfigureArgument::Owner(address) => config.owner = address,
				ConfigureArgument::Proxy(proxy) => config.proxy = proxy,
			}
		}
	}

	pub fn config(&self) -> &'static Config {
		Storage::config()
	}

	pub fn fulfillment(&self, job_id: u64) -> Option<&'static Vec<u8>> {
		Storage::fulfillments().get(&job_id)
	}

	pub fn fulfillments_index(&self) -> &'static Vec<u64> {
		Storage::fulfillments_index()
	}

	pub fn fulfill(&mut self, job_id: u64, payload: Vec<u8>) -> Result<(), ConsumerError> {
		if Storage::config().proxy != msg::source() {
			Err(ConsumerError::UnauthorizedCaller)?
		}

		Storage::fulfillments().insert(job_id, payload);
		Storage::fulfillments_index().push(job_id);

		let _ = self.notify_on(Event::FulfillmentReceived { job_id });

		Ok(())
	}
}
