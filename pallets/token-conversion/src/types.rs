use frame_support::{pallet_prelude::*, traits::fungible::Inspect};
use parity_scale_codec::{Decode, Encode};
use sp_runtime::RuntimeDebug;
use sp_std::prelude::*;

use crate::Config;

pub type BalanceFor<T> =
	<<T as Config>::Currency as Inspect<<T as frame_system::Config>::AccountId>>::Balance;

#[derive(RuntimeDebug, Encode, Decode, TypeInfo, Eq, PartialEq, Clone, MaxEncodedLen)]
pub struct Conversion<Balance, BlockNumber> {
	pub amount: Balance,
	pub lock_start: BlockNumber,
	pub lock_duration: BlockNumber,
	pub modified: bool,
}

#[derive(RuntimeDebug, Encode, Decode, TypeInfo, Eq, PartialEq, Clone, MaxEncodedLen)]
pub struct ConversionMessage<AccountId, Balance> {
	pub account: AccountId,
	pub amount: Balance,
}

pub type ConversionMessageFor<T> =
	ConversionMessage<<T as frame_system::Config>::AccountId, BalanceFor<T>>;
