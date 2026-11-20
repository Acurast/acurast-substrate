use frame_support::{pallet_prelude::*, traits::fungible::Inspect};

use crate::Config;

pub const MIN_LABEL_LENGTH: u32 = 3;
pub const MAX_LABEL_LENGTH: u32 = 63;
pub const MAX_SUFFIX_LENGTH: u32 = 100;
pub const MAX_HOST_LENGTH: u32 = 253;

pub type ZoneId = u16;
pub type Label = BoundedVec<u8, ConstU32<MAX_LABEL_LENGTH>>;
pub type Suffix = BoundedVec<u8, ConstU32<MAX_SUFFIX_LENGTH>>;
pub type Host = BoundedVec<u8, ConstU32<MAX_HOST_LENGTH>>;
pub type DomainKey = (ZoneId, Label);

pub type BalanceFor<T> =
	<<T as Config>::Currency as Inspect<<T as frame_system::Config>::AccountId>>::Balance;

pub type ReservationFor<T> = Reservation<
	<T as frame_system::Config>::AccountId,
	BalanceFor<T>,
	frame_system::pallet_prelude::BlockNumberFor<T>,
>;

#[derive(Debug, Encode, Decode, TypeInfo, MaxEncodedLen, Clone, PartialEq, Eq)]
pub struct Zone<Balance> {
	pub suffix: Suffix,
	/// Deposit held for each new claim in this zone.
	pub deposit: Balance,
	/// Whether new claims are accepted.
	pub active: bool,
}

#[derive(Debug, Encode, Decode, TypeInfo, MaxEncodedLen, Clone, PartialEq, Eq)]
pub struct Reservation<AccountId, Balance, BlockNumber> {
	/// Must stay the first field: off-chain readers decode only this prefix.
	pub owner: AccountId,
	/// Amount held from `owner`, set from the zone deposit at claim or transfer.
	pub deposit: Balance,
	pub registered_at: BlockNumber,
	/// Account proposed as the next owner, until it accepts or the proposal is dropped.
	pub pending_owner: Option<AccountId>,
}
