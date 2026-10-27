use crate::{OriginCaller, Runtime, RuntimeCall, RuntimeEvent};

impl pallet_utility::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type RuntimeCall = RuntimeCall;
	type PalletsOrigin = OriginCaller;
	type WeightInfo = acurast_runtime_common::weight::pallet_utility::WeightInfo<Runtime>;
}
