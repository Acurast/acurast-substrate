use crate::{Epoch, FeeManagerPalletId, Runtime};

impl pallet_acurast_rewards_treasury::Config for Runtime {
	type Epoch = Epoch;
	type PalletId = FeeManagerPalletId;
}
