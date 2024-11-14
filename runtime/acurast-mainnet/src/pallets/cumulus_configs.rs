use cumulus_pallet_parachain_system::RelayNumberMonotonicallyIncreases;
use cumulus_primitives_core::{AggregateMessageOrigin, ParaId};
use frame_support::traits::TransformOrigin;
use parachains_common::message_queue::ParaIdToSibling;
use polkadot_runtime_common::xcm_sender::NoPriceForMessageDelivery;
use sp_core::ConstU32;

use crate::{
	xcm_config::XcmOriginToTransactDispatchOrigin, ConsensusHook, EnsureAdminOrRoot, MessageQueue,
	ParachainSystem, RelayOrigin, ReservedDmpWeight, ReservedXcmpWeight, Runtime, RuntimeEvent,
	XcmpQueue,
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

	#[doc = " An entry-point for higher-level logic to manage the backlog of unincluded parachain"]
	#[doc = " blocks and authorship rights for those blocks."]
	#[doc = ""]
	#[doc = " Typically, this should be a hook tailored to the collator-selection/consensus mechanism"]
	#[doc = " that is used for this chain."]
	#[doc = ""]
	#[doc = " However, to maintain the same behavior as prior to asynchronous backing, provide the"]
	#[doc = " [`consensus_hook::ExpectParentIncluded`] here. This is only necessary in the case"]
	#[doc = " that collators aren\'t expected to have node versions that supply the included block"]
	#[doc = " in the relay-chain state proof."]
	type ConsensusHook = ConsensusHook;
}

/// Runtime configuration for cumulus_pallet_aura_ext.
impl cumulus_pallet_aura_ext::Config for Runtime {}

/// Runtime configuration for cumulus_pallet_xcmp_queue.
impl cumulus_pallet_xcmp_queue::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type ChannelInfo = ParachainSystem;
	type VersionWrapper = ();
	type ControllerOrigin = EnsureAdminOrRoot;
	type ControllerOriginConverter = XcmOriginToTransactDispatchOrigin;
	type PriceForSiblingDelivery = NoPriceForMessageDelivery<ParaId>;
	type WeightInfo = ();
	type XcmpQueue = TransformOrigin<MessageQueue, AggregateMessageOrigin, ParaId, ParaIdToSibling>;
	type MaxInboundSuspended = ConstU32<1_000>;

	#[doc = " Maximal number of outbound XCMP channels that can have messages queued at the same time."]
	#[doc = ""]
	#[doc = " If this is reached, then no further messages can be sent to channels that do not yet"]
	#[doc = " have a message queued. This should be set to the expected maximum of outbound channels"]
	#[doc = " which is determined by [`Self::ChannelInfo`]. It is important to set this large enough,"]
	#[doc = " since otherwise the congestion control protocol will not work as intended and messages"]
	#[doc = " may be dropped. This value increases the PoV and should therefore not be picked too"]
	#[doc = " high. Governance needs to pay attention to not open more channels than this value."]
	type MaxActiveOutboundChannels = ConstU32<128>;

	#[doc = " The maximal page size for HRMP message pages."]
	#[doc = ""]
	#[doc = " A lower limit can be set dynamically, but this is the hard-limit for the PoV worst case"]
	#[doc = " benchmarking. The limit for the size of a message is slightly below this, since some"]
	#[doc = " overhead is incurred for encoding the format."]
	type MaxPageSize = ConstU32<{ 103 * 1024 }>;
}
