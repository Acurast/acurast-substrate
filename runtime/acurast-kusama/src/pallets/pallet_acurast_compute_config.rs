use frame_support::parameter_types;

use acurast_runtime_common::{types::BlockNumber, weight};
use pallet_acurast::ElegibleRewardAccountLookup;

use crate::{Acurast, AcurastProcessorManager, Balances, Runtime, RuntimeEvent};

parameter_types! {
	pub const EpochBase: BlockNumber = 0;
	pub const Epoch: BlockNumber = 900; // 1.5 hours
	pub const MetricEpochValidity: BlockNumber = 240;
	pub const WarmupPeriod: BlockNumber = 1800; // 3 hours, only for testing, we should use something like 2 weeks = 219027
}

impl pallet_acurast_compute::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type EpochBase = EpochBase;
	type Epoch = Epoch;
	type MetricValidity = MetricEpochValidity;
	type WarmupPeriod = WarmupPeriod;
	type Currency = Balances;
	type EligibleRewardAccountLookup = ElegibleRewardAccountLookup<
		Self::AccountId,
		Acurast,
		AcurastProcessorManager,
		AcurastProcessorManager,
	>;
	type WeightInfo = weight::pallet_acurast_compute::WeightInfo<Runtime>;
}
