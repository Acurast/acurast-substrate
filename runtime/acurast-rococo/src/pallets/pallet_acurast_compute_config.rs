use acurast_runtime_common::{
	types::{Balance, BlockNumber, ComputeRewardDistributor},
	weight,
};

use frame_support::parameter_types;

use crate::{Balances, Runtime, RuntimeEvent};

use super::pallet_acurast_processor_manager_config::AcurastManagerIdProvider;

parameter_types! {
	pub const EpochBase: BlockNumber = 0;
	pub const Epoch: BlockNumber = 900; // 1.5 hours
	pub const WarmupPeriod: BlockNumber = 1800; // 3 hours, only for testing, we should use something like 2 weeks = 219027
}

impl pallet_acurast_compute::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type ManagerId = u128;
	type ManagerIdProvider = AcurastManagerIdProvider;
	type EpochBase = EpochBase;
	type Epoch = Epoch;
	type WarmupPeriod = WarmupPeriod;
	type Balance = Balance;
	type BlockNumber = BlockNumber;
	type Currency = Balances;
	type ComputeRewardDistributor = ComputeRewardDistributor<Runtime, (), Balances>;
	type WeightInfo = weight::pallet_acurast_compute::WeightInfo<Runtime>;
}
