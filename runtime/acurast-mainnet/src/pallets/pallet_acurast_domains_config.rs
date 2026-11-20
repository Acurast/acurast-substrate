use frame_support::parameter_types;

use crate::{Balances, EnsureCouncilOrRoot, Runtime, RuntimeHoldReason};

parameter_types! {
	pub const MaxDomainsPerAccount: u32 = 16;
}

impl pallet_acurast_domains::Config for Runtime {
	type RuntimeHoldReason = RuntimeHoldReason;
	type Currency = Balances;
	type UpdateOrigin = EnsureCouncilOrRoot;
	type MaxDomainsPerAccount = MaxDomainsPerAccount;
	type WeightInfo = acurast_runtime_common::weight::pallet_acurast_domains::WeightInfo<Self>;
}
