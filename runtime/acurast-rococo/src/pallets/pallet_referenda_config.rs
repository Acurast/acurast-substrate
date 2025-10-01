use frame_support::parameter_types;
use frame_system::{EnsureRoot, EnsureSigned};
use pallet_referenda::{Curve, Track, TrackInfo};
use sp_core::ConstU32;
use sp_runtime::str_array as s;

use acurast_runtime_common::{
	constants::{DAYS, MINUTES, UNIT},
	types::{AccountId, Balance, BlockNumber, TracksInfo},
};

use crate::{Balances, Preimage, Referenda, Runtime, RuntimeCall, RuntimeEvent, Scheduler, System};

const fn percent(x: i32) -> sp_arithmetic::FixedI64 {
	sp_arithmetic::FixedI64::from_rational(x as u128, 100)
}

const APP_ROOT: Curve = Curve::make_reciprocal(4, 10, percent(80), percent(50), percent(100));
const SUP_ROOT: Curve = Curve::make_linear(10, 10, percent(0), percent(50));

parameter_types! {
	pub const AlarmInterval: BlockNumber = 1;
	pub const SubmissionDeposit: Balance = 4 * UNIT;
	pub const UndecidingTimeout: BlockNumber = 14 * DAYS;
	pub const Tracks: [Track<u16, Balance, BlockNumber>; 1] = [Track {
		id: 0,
		info: TrackInfo {
			name: s("root"),
			max_deciding: 1,
			decision_deposit: 100 * UNIT,
			prepare_period: 2 * MINUTES,
			decision_period: 10 * MINUTES,
			confirm_period: 2 * MINUTES,
			min_enactment_period: 2 * MINUTES,
			min_approval: APP_ROOT,
			min_support: SUP_ROOT,
		},
	}];
	pub const VoteLockingPeriod: BlockNumber = 2 * MINUTES;
}

impl pallet_referenda::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type RuntimeCall = RuntimeCall;
	type Currency = Balances;
	type Scheduler = Scheduler;
	type SubmitOrigin = EnsureSigned<AccountId>;
	type CancelOrigin = EnsureRoot<AccountId>;
	type KillOrigin = EnsureRoot<AccountId>;
	type Slash = ();
	type Votes = pallet_conviction_voting::VotesOf<Self>;
	type Tally = pallet_conviction_voting::TallyOf<Self>;
	type SubmissionDeposit = SubmissionDeposit;
	type MaxQueued = ConstU32<100>;
	type UndecidingTimeout = UndecidingTimeout;
	type AlarmInterval = AlarmInterval;
	type Tracks = TracksInfo<Self, Tracks>;
	type Preimages = Preimage;
	type BlockNumberProvider = System;
	type WeightInfo = pallet_referenda::weights::SubstrateWeight<Self>;
}

impl pallet_conviction_voting::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type Polls = Referenda;
	type MaxTurnout =
		frame_support::traits::tokens::currency::ActiveIssuanceOf<Balances, Self::AccountId>;
	type MaxVotes = ConstU32<512>;
	type VoteLockingPeriod = VoteLockingPeriod;
	type BlockNumberProvider = System;
	type VotingHooks = ();
	type WeightInfo = pallet_conviction_voting::weights::SubstrateWeight<Self>;
}
