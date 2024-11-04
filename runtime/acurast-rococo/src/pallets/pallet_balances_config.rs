use acurast_runtime_common::{weight, Balance};
use sp_core::ConstU32;

use crate::{
	ExistentialDeposit, MaxLocks, MaxReserves, Runtime, RuntimeEvent, RuntimeFreezeReason,
	RuntimeHoldReason, System,
};

/// Runtime configuration for pallet_balances.
impl pallet_balances::Config for Runtime {
	type MaxLocks = MaxLocks;
	/// The type for recording an account's balance.
	type Balance = Balance;
	/// The ubiquitous event type.
	type RuntimeEvent = RuntimeEvent;
	type DustRemoval = ();
	type ExistentialDeposit = ExistentialDeposit;
	type AccountStore = System;
	type WeightInfo = weight::pallet_balances::WeightInfo<Runtime>;
	type MaxReserves = MaxReserves;
	type ReserveIdentifier = [u8; 8];
	type RuntimeHoldReason = RuntimeHoldReason;
	type RuntimeFreezeReason = RuntimeFreezeReason;
	type FreezeIdentifier = ();
	type MaxHolds = ConstU32<{ u32::MAX }>;
	type MaxFreezes = ConstU32<0>;
}
