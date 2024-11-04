use acurast_runtime_common::{weights, Balance, BlockNumber};

use crate::{
	implementations::StakingOverVesting, BalanceUnit, DivestTolerance, MaximumLockingPeriod,
	Runtime, RuntimeEvent,
};

impl pallet_acurast_vesting::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type DivestTolerance = DivestTolerance;
	type MaximumLockingPeriod = MaximumLockingPeriod;
	type Balance = Balance;
	type BalanceUnit = BalanceUnit;
	type BlockNumber = BlockNumber;
	type VestingBalance = StakingOverVesting;
	type WeightInfo = weights::pallet_acurast_vesting::WeightInfo<Runtime>;
}
