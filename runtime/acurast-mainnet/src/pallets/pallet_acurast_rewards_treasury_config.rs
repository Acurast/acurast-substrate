use crate::{Epoch, FeeManagerPalletId, Runtime, RuntimeEvent};

impl pallet_acurast_rewards_treasury::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Epoch = Epoch;
	type PalletId = FeeManagerPalletId;
}
