use sails_rs::prelude::{scale_codec::*, *};

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo)]
pub enum Event {
	FulfillmentReceived { job_id: u64 },
}

/// Contract configurations are contained in this structure
#[derive(Debug, Clone, Eq, PartialEq, Encode, Decode, TypeInfo, Default)]
pub struct Config {
	/// Address allowed to manage the program
	pub owner: ActorId,
	/// Program allowed to call fulfill of this consumer program
	pub proxy: ActorId,
}

#[derive(Debug, Clone, Eq, PartialEq, Encode, Decode, TypeInfo)]
pub enum ConfigureArgument {
	Owner(ActorId),
	Proxy(ActorId),
}
