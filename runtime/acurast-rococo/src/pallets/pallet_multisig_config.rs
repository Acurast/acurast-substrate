use crate::{
	Balances, DepositBase, DepositFactor, MaxSignatories, Runtime, RuntimeCall, RuntimeEvent,
};

impl pallet_multisig::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type RuntimeCall = RuntimeCall;
	type Currency = Balances;
	type DepositBase = DepositBase;
	type DepositFactor = DepositFactor;
	type MaxSignatories = MaxSignatories;
	type BlockNumberProvider = frame_system::Pallet<Self>;
	type WeightInfo = acurast_runtime_common::weight::pallet_multisig::WeightInfo<Runtime>;
}
