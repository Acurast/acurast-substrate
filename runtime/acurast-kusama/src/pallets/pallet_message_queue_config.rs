use cumulus_primitives_core::AggregateMessageOrigin;
use parachains_common::message_queue::NarrowOriginToSibling;

#[cfg(not(feature = "runtime-benchmarks"))]
use crate::RuntimeCall;
use crate::{MessageQueueServiceWeight, Runtime, RuntimeEvent, XcmpQueue};

impl pallet_message_queue::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = ();
	#[cfg(feature = "runtime-benchmarks")]
	type MessageProcessor =
		pallet_message_queue::mock_helpers::NoopMessageProcessor<AggregateMessageOrigin>;
	#[cfg(not(feature = "runtime-benchmarks"))]
	type MessageProcessor = xcm_builder::ProcessXcmMessage<
		AggregateMessageOrigin,
		xcm_executor::XcmExecutor<crate::xcm_config::XcmConfig>,
		RuntimeCall,
	>;
	type Size = u32;
	// The XCMP queue pallet is only ever able to handle the `Sibling(ParaId)` origin:
	type QueueChangeHandler = NarrowOriginToSibling<XcmpQueue>;
	type QueuePausedQuery = NarrowOriginToSibling<XcmpQueue>;
	type HeapSize = sp_core::ConstU32<{ 64 * 1024 }>;
	type MaxStale = sp_core::ConstU32<8>;
	type ServiceWeight = MessageQueueServiceWeight;

	#[doc = " The maximum amount of weight (if any) to be used from remaining weight `on_idle` which"]
	#[doc = " should be provided to the message queue for servicing enqueued items `on_idle`."]
	#[doc = " Useful for parachains to process messages at the same block they are received."]
	#[doc = ""]
	#[doc = " If `None`, it will not call `ServiceQueues::service_queues` in `on_idle`."]
	type IdleMaxServiceWeight = MessageQueueServiceWeight;
}
