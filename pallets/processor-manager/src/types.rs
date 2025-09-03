use acurast_common::{ListUpdate, Version};
use frame_support::{
	pallet_prelude::*,
	sp_runtime::traits::{IdentifyAccount, MaybeDisplay, Verify},
	traits::{Currency, IsType, UnixTime},
};
use sp_core::H256;

use crate::Config;

#[derive(
	RuntimeDebug,
	Encode,
	Decode,
	DecodeWithMemTracking,
	MaxEncodedLen,
	TypeInfo,
	Clone,
	PartialEq,
	Eq,
)]
pub struct ProcessorPairing<AccountId, Signature>
where
	AccountId: Parameter + Member + MaybeSerializeDeserialize + MaybeDisplay,
	Signature: Parameter + Member + Verify,
{
	pub account: AccountId,
	pub proof: Option<Proof<Signature>>,
}

impl<AccountId, Signature> ProcessorPairing<AccountId, Signature>
where
	AccountId: Parameter + Member + MaybeSerializeDeserialize + MaybeDisplay,
	Signature: Parameter + Member + Verify,
{
	pub fn new_with_proof(account: AccountId, timestamp: u128, signature: Signature) -> Self {
		Self { account, proof: Some(Proof { timestamp, signature }) }
	}

	pub fn new(account: AccountId) -> Self {
		Self { account, proof: None }
	}
}

impl<AccountId, Signature> ProcessorPairing<AccountId, Signature>
where
	AccountId: IsType<<<Signature as Verify>::Signer as IdentifyAccount>::AccountId>,
	AccountId: Parameter + Member + MaybeSerializeDeserialize + MaybeDisplay + Ord,
	Signature: Parameter + Member + Verify,
{
	pub fn validate_timestamp<T: Config>(&self) -> bool {
		if let Some(proof) = &self.proof {
			let now = T::UnixTime::now().as_millis();
			if let Some(diff) = now.checked_sub(proof.timestamp) {
				return proof.timestamp <= now && diff < T::PairingProofExpirationTime::get();
			}
		}
		false
	}

	pub fn validate_signature<T: Config>(
		&self,
		account_id: &AccountId,
		counter: T::Counter,
	) -> bool {
		if let Some(proof) = &self.proof {
			let message = [
				b"<Bytes>".to_vec(),
				account_id.encode(),
				proof.timestamp.encode(),
				counter.encode(),
				b"</Bytes>".to_vec(),
			]
			.concat();
			return proof.signature.verify(message.as_ref(), &self.account.clone().into());
		}

		false
	}

	pub fn multi_validate_signature<T: Config>(&self, account_id: &AccountId) -> bool {
		if let Some(proof) = &self.proof {
			let message = [
				b"<Bytes>".to_vec(),
				account_id.encode(),
				proof.timestamp.encode(),
				b"</Bytes>".to_vec(),
			]
			.concat();
			return proof.signature.verify(message.as_ref(), &self.account.clone().into());
		}

		false
	}
}

#[derive(
	RuntimeDebug,
	Encode,
	Decode,
	DecodeWithMemTracking,
	MaxEncodedLen,
	TypeInfo,
	Clone,
	PartialEq,
	Eq,
)]
pub struct Proof<Signature>
where
	Signature: Parameter + Member + Verify,
{
	pub timestamp: u128,
	pub signature: Signature,
}

pub type ProcessorPairingUpdate<AccountId, Signature> =
	ListUpdate<ProcessorPairing<AccountId, Signature>>;

pub type BinaryHash = H256;
pub(crate) const MAX_LOCATION_LENGTH: u32 = 200;
pub type BinaryLocation = BoundedVec<u8, ConstU32<MAX_LOCATION_LENGTH>>;

#[derive(
	RuntimeDebug,
	Encode,
	Decode,
	DecodeWithMemTracking,
	MaxEncodedLen,
	TypeInfo,
	Clone,
	PartialEq,
	Eq,
)]
pub struct UpdateInfo {
	pub version: Version,
	pub binary_location: BinaryLocation,
}

#[derive(RuntimeDebug, Encode, Decode, MaxEncodedLen, TypeInfo, Clone, PartialEq)]
pub struct UpdateInfos {
	pub udpate_info: UpdateInfo,
	pub binary_hash: BinaryHash,
}

#[derive(RuntimeDebug, Encode, Decode, MaxEncodedLen, TypeInfo, Clone, Copy, PartialEq)]
pub struct RewardDistributionWindow {
	pub start: u32,
	pub heartbeats: u32,
	pub window_length: u32,
	pub tollerance: u32,
	pub min_heartbeats: u32,
}

impl RewardDistributionWindow {
	pub fn new<Balance, AccountId>(
		start: u32,
		settings: &RewardDistributionSettings<Balance, AccountId>,
	) -> Self {
		Self {
			start,
			heartbeats: 0,
			window_length: settings.window_length,
			tollerance: settings.tollerance,
			min_heartbeats: settings.min_heartbeats,
		}
	}

	pub fn next(&self) -> Self {
		Self {
			start: self.start,
			heartbeats: self.heartbeats + 1,
			window_length: self.window_length,
			tollerance: self.tollerance,
			min_heartbeats: self.min_heartbeats,
		}
	}
}

#[derive(
	RuntimeDebug, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo, Clone, PartialEq,
)]
pub struct RewardDistributionSettings<Balance, AccountId> {
	pub window_length: u32,
	pub tollerance: u32,
	pub min_heartbeats: u32,
	pub reward_per_distribution: Balance,
	pub distributor_account: AccountId,
}

pub const MAX_ENDPOINT_LENGTH: u32 = 200;
pub type Endpoint = BoundedVec<u8, ConstU32<MAX_ENDPOINT_LENGTH>>;

#[derive(
	RuntimeDebug, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo, Clone, PartialEq,
)]
pub struct OnboardingSettings<Balance, AccountId> {
	pub funds: Balance,
	pub funds_account: AccountId,
}

pub type ProcessorPairingFor<T> =
	ProcessorPairing<<T as frame_system::Config>::AccountId, <T as Config>::Proof>;
pub type ProcessorPairingUpdateFor<T> =
	ProcessorPairingUpdate<<T as frame_system::Config>::AccountId, <T as Config>::Proof>;

pub type ProcessorUpdatesFor<T> =
	BoundedVec<ProcessorPairingUpdateFor<T>, <T as Config>::MaxPairingUpdates>;
pub type ProcessorList<T> =
	BoundedVec<<T as frame_system::Config>::AccountId, <T as Config>::MaxProcessorsInSetUpdateInfo>;

pub type BalanceFor<T> =
	<<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;
