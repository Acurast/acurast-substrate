use crate::{
	EnsureAdminOrRoot, MaxScheduledPerBlock, MaximumSchedulerWeight, OriginCaller, Preimage,
	Runtime, RuntimeCall, RuntimeEvent, RuntimeOrigin,
};

/// Runtime configuration for pallet_scheduler.
impl pallet_scheduler::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type RuntimeOrigin = RuntimeOrigin;
	type PalletsOrigin = OriginCaller;
	type RuntimeCall = RuntimeCall;
	type MaximumWeight = MaximumSchedulerWeight;
	type ScheduleOrigin = EnsureAdminOrRoot;
	type MaxScheduledPerBlock = MaxScheduledPerBlock;
	type WeightInfo = pallet_scheduler::weights::SubstrateWeight<Self>;
	type OriginPrivilegeCmp = frame_support::traits::EqualPrivilegeOnly;
	type Preimages = Preimage;
}
