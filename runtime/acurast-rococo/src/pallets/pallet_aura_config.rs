use acurast_runtime_common::AuraId;
use pallet_aura::MinimumPeriodTimesTwo;
use sp_core::ConstBool;

use crate::{MaxAuthorities, Runtime};

/// Runtime configuration for pallet_aura.
impl pallet_aura::Config for Runtime {
	type AuthorityId = AuraId;
	type MaxAuthorities = MaxAuthorities;
	type DisabledValidators = ();
	type AllowMultipleBlocksPerSlot = ConstBool<false>;

	#[doc = " The slot duration Aura should run with, expressed in milliseconds."]
	#[doc = " The effective value of this type should not change while the chain is running."]
	#[doc = ""]
	#[doc = " For backwards compatibility either use [`MinimumPeriodTimesTwo`] or a const."]
	type SlotDuration = MinimumPeriodTimesTwo<Self>;
}
