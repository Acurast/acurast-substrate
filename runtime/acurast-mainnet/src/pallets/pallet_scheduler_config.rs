use crate::{
	EnsureCouncilOrRoot, MaxScheduledPerBlock, MaximumSchedulerWeight, OriginCaller, Preimage,
	Runtime, RuntimeCall, RuntimeEvent, RuntimeOrigin,
};

/// Runtime configuration for pallet_scheduler.
impl pallet_scheduler::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type RuntimeOrigin = RuntimeOrigin;
	type PalletsOrigin = OriginCaller;
	type RuntimeCall = RuntimeCall;
	type MaximumWeight = MaximumSchedulerWeight;
	type ScheduleOrigin = EnsureCouncilOrRoot;
	type MaxScheduledPerBlock = MaxScheduledPerBlock;
	type WeightInfo = acurast_runtime_common::weight::pallet_scheduler::WeightInfo<Runtime>;
	type OriginPrivilegeCmp = frame_support::traits::EqualPrivilegeOnly;
	type Preimages = Preimage;
	type BlockNumberProvider = frame_system::Pallet<Self>;
}
