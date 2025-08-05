use acurast_runtime_common::{
	constants::UNIT,
	types::{Balance, BlockNumber, ComputeRewardDistributor},
	weight,
};

use frame_support::{
	parameter_types,
	traits::{ConstU32, LockIdentifier},
};

use sp_runtime::Perquintill;

use crate::{Balances, Runtime, RuntimeEvent};

use super::pallet_acurast_marketplace_config::ManagerProvider;
use super::pallet_acurast_processor_manager_config::AcurastManagerIdProvider;

parameter_types! {
	pub const Epoch: BlockNumber = 900; // 1.5 hours
	pub const EpochBase: BlockNumber = 0;
	pub const Era: BlockNumber = 16200; // 24 hours
	pub const WarmupPeriod: BlockNumber = 1800; // 3 hours, only for testing, we should use something like 2 weeks = 219027
	pub const MaxDelegations: u8 = 20;
	pub const MinCooldownPeriod: BlockNumber = 432000; // ~1 month
	pub const MaxCooldownPeriod: BlockNumber = 20736000; // ~4 years
	pub const MinDelegation: Balance = 1 * UNIT;
	pub const MinStake: Balance = 10 * UNIT;
	pub const MaxDelegationRatio: Perquintill = Perquintill::from_percent(10);
 pub const ComputeStakingLockId: LockIdentifier = *b"compstak";
}

impl pallet_acurast_compute::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type ManagerId = u128;
	type ManagerProvider = ManagerProvider;
	type ManagerIdProvider = AcurastManagerIdProvider;
	type Epoch = Epoch;
	type EpochBase = EpochBase;
	type Era = Era;
	type MaxPools = ConstU32<30>;
	type MaxDelegations = MaxDelegations;
	type MinCooldownPeriod = MinCooldownPeriod;
	type MaxCooldownPeriod = MaxCooldownPeriod;
	type MinDelegation = MinDelegation;
	type MaxDelegationRatio = MaxDelegationRatio;
	type MinStake = MinStake;
	type WarmupPeriod = WarmupPeriod;
	type Balance = Balance;
	type BlockNumber = BlockNumber;
	type Currency = Balances;
	type LockIdentifier = ComputeStakingLockId;
	type ComputeRewardDistributor = ComputeRewardDistributor<Runtime, (), Balances>;
	type WeightInfo = weight::pallet_acurast_compute::WeightInfo<Runtime>;
}
