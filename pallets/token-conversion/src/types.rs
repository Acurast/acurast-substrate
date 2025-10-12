use acurast_common::Subject;
use frame_support::{pallet_prelude::*, traits::fungible::Inspect};
use frame_system::pallet_prelude::BlockNumberFor;
use parity_scale_codec::{Decode, Encode};
use sp_runtime::RuntimeDebug;
use sp_std::prelude::*;

use crate::Config;

pub type BalanceFor<T> =
	<<T as Config>::Currency as Inspect<<T as frame_system::Config>::AccountId>>::Balance;

#[derive(RuntimeDebug, Encode, Decode, TypeInfo, Eq, PartialEq, Clone, MaxEncodedLen)]
pub struct InitiatedConversionMessage<AccountId, Balance, BlockNumber> {
	pub burned: Balance,
	pub fee_payer: AccountId,
	pub started_at: BlockNumber,
}

#[derive(RuntimeDebug, Encode, Decode, TypeInfo, Eq, PartialEq, Clone, MaxEncodedLen)]
pub struct Conversion<Balance, BlockNumber> {
	pub amount: Balance,
	pub lock_start: BlockNumber,
}

#[derive(RuntimeDebug, Encode, Decode, TypeInfo, Eq, PartialEq, Clone, MaxEncodedLen)]
pub struct ConversionMessage<AccountId, Balance> {
	pub account: AccountId,
	pub amount: Balance,
}

pub type InitiatedConversionMessageFor<T> = InitiatedConversionMessage<
	<T as frame_system::Config>::AccountId,
	BalanceFor<T>,
	BlockNumberFor<T>,
>;

pub type ConversionMessageFor<T> =
	ConversionMessage<<T as frame_system::Config>::AccountId, BalanceFor<T>>;

pub type SubjectFor<T> =
	Subject<<T as frame_system::Config>::AccountId, <T as frame_system::Config>::AccountId>;
