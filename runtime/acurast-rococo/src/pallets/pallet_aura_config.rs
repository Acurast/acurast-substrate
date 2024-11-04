use acurast_runtime_common::AuraId;
use sp_core::ConstBool;

use crate::{MaxAuthorities, Runtime};

/// Runtime configuration for pallet_aura.
impl pallet_aura::Config for Runtime {
	type AuthorityId = AuraId;
	type MaxAuthorities = MaxAuthorities;
	type DisabledValidators = ();
	type AllowMultipleBlocksPerSlot = ConstBool<false>;
}
