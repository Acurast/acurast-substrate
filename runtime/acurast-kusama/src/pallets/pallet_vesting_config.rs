use frame_support::{parameter_types, traits::WithdrawReasons};
use sp_core::ConstU128;
use sp_runtime::traits::ConvertInto;

use crate::{constants::EXISTENTIAL_DEPOSIT, Balances, Runtime, RuntimeEvent, System};

parameter_types! {
	pub const VestingWithdrawReasons: WithdrawReasons = WithdrawReasons::FEE;
}

impl pallet_vesting::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type BlockNumberToBalance = ConvertInto;
	type MinVestedTransfer = ConstU128<{ EXISTENTIAL_DEPOSIT }>;
	type UnvestedFundsAllowedWithdrawReasons = VestingWithdrawReasons;
	type BlockNumberProvider = System;
	const MAX_VESTING_SCHEDULES: u32 = 10;
	type WeightInfo = pallet_vesting::weights::SubstrateWeight<Self>;
}
