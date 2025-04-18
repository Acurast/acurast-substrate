use acurast_runtime_common::types::AuraId;
use sp_core::{ConstBool, ConstU64};

use crate::{MaxAuthorities, Runtime, SLOT_DURATION};

/// Runtime configuration for pallet_aura.
impl pallet_aura::Config for Runtime {
	type AuthorityId = AuraId;
	type MaxAuthorities = MaxAuthorities;
	type DisabledValidators = ();
	type AllowMultipleBlocksPerSlot = ConstBool<true>;

	#[doc = " The slot duration Aura should run with, expressed in milliseconds."]
	#[doc = " The effective value of this type should not change while the chain is running."]
	#[doc = ""]
	#[doc = " For backwards compatibility either use [`MinimumPeriodTimesTwo`] or a const."]
	type SlotDuration = ConstU64<SLOT_DURATION>;
}
