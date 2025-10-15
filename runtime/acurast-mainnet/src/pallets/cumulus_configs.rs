use cumulus_pallet_parachain_system::RelayNumberMonotonicallyIncreases;
use cumulus_primitives_core::{AggregateMessageOrigin, ParaId};
use frame_support::traits::TransformOrigin;
use parachains_common::message_queue::ParaIdToSibling;
use polkadot_runtime_common::xcm_sender::NoPriceForMessageDelivery;
use sp_core::ConstU32;

use crate::{
	xcm_config::XcmOriginToTransactDispatchOrigin, ConsensusHook, EnsureCouncilOrRoot,
	MessageQueue, ParachainSystem, RelayOrigin, ReservedDmpWeight, ReservedXcmpWeight, Runtime,
	RuntimeEvent, XcmpQueue,
};

/// Runtime configuration for cumulus_pallet_parachain_system.
impl cumulus_pallet_parachain_system::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type OnSystemEvent = ();
	type SelfParaId = parachain_info::Pallet<Runtime>;
	type OutboundXcmpMessageSource = XcmpQueue;
	type DmpQueue = frame_support::traits::EnqueueWithOrigin<MessageQueue, RelayOrigin>;
	type ReservedDmpWeight = ReservedDmpWeight;
	type XcmpMessageHandler = XcmpQueue;
	type ReservedXcmpWeight = ReservedXcmpWeight;
	type CheckAssociatedRelayNumber = RelayNumberMonotonicallyIncreases;
	type WeightInfo = ();
	type ConsensusHook = ConsensusHook;
	type SelectCore = cumulus_pallet_parachain_system::DefaultCoreSelector<Self>;
}

/// Runtime configuration for cumulus_pallet_aura_ext.
impl cumulus_pallet_aura_ext::Config for Runtime {}

/// Runtime configuration for cumulus_pallet_xcmp_queue.
impl cumulus_pallet_xcmp_queue::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type ChannelInfo = ParachainSystem;
	type VersionWrapper = ();
	type ControllerOrigin = EnsureCouncilOrRoot;
	type ControllerOriginConverter = XcmOriginToTransactDispatchOrigin;
	type PriceForSiblingDelivery = NoPriceForMessageDelivery<ParaId>;
	type WeightInfo = ();
	type XcmpQueue = TransformOrigin<MessageQueue, AggregateMessageOrigin, ParaId, ParaIdToSibling>;
	type MaxInboundSuspended = ConstU32<1_000>;
	type MaxActiveOutboundChannels = ConstU32<128>;
	type MaxPageSize = ConstU32<{ 103 * 1024 }>;
}

impl cumulus_pallet_weight_reclaim::Config for Runtime {
	type WeightInfo = ();
}
