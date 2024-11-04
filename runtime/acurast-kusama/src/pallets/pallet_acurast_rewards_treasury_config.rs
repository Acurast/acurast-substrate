use crate::{Epoch, Runtime, RuntimeEvent, Treasury};

impl pallet_acurast_rewards_treasury::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Epoch = Epoch;
	type Treasury = Treasury;
}
