use frame_support::{
	pallet_prelude::*, storage::bounded_vec::BoundedVec, traits::fungible::Inspect,
};
use pallet_acurast::{MultiOrigin, Subject};
use parity_scale_codec::{Decode, Encode};
use scale_info::TypeInfo;
use sp_core::ConstU32;
use sp_runtime::RuntimeDebug;
use sp_std::prelude::*;
use strum_macros::{EnumString, IntoStaticStr};

use crate::Config;

pub type TransferNonce = u64;
pub type EnableNonce = u32;

pub const TRANSFER_RECIPIENT_MAX_LENGTH: u32 = 64;
/// The recipient of a transfer, on Acurast or proxy chain. The length depends on the chain the transfer is received on.
pub type TransferRecipient = BoundedVec<u8, ConstU32<TRANSFER_RECIPIENT_MAX_LENGTH>>;

/// The action is triggered in target chain (the _Hyperdrive Token_ contract on proxy chain) upon a [`hyperdrive_ibc::Message`].
#[derive(RuntimeDebug, Encode, Decode, TypeInfo, Eq, PartialEq, Clone, MaxEncodedLen)]
pub enum Action<AccountId> {
	/// Transfers (bridges) a token to the target chain.
	///
	/// Message consists of `amount, asset_id, transfer_nonce, dest`.
	/// * Using None for asset_id stands for the native token.
	/// * The `transfer_nonce` is used to identify and order transfers **per proxy**. Note that the [`pallet_acurast_hyperdrive_ibc::Message.id`] does change on each send retry, therefore this nonce is required for deduplication of transfers.
	///   Even though hyperdrive_ibc allows to resend messages with same nonce after ttl expired, _Exactly-once delivery_ is and cannot be guaranteed.
	TransferToken(u128, Option<u32>, TransferNonce, MultiOrigin<AccountId>),
	/// A noop action that solely suits the purpose of testing that messages get sent.
	Noop,
	SetEnabled(bool),
}

impl<AccountId> From<&Action<AccountId>> for RawAction {
	fn from(action: &Action<AccountId>) -> Self {
		match action {
			Action::TransferToken(_, _, _, _) => RawAction::TransferToken,
			Action::Noop => RawAction::Noop,
			Action::SetEnabled(_) => RawAction::SetEnabled,
		}
	}
}

/// The possible actions found in messages to and from proxy chain (the _Hyperdrive Token_ contract on proxy chain).
#[derive(
	RuntimeDebug, Encode, Decode, TypeInfo, Clone, Eq, PartialEq, EnumString, IntoStaticStr,
)]
pub enum RawAction {
	#[strum(serialize = "TRANS")]
	TransferToken,
	#[strum(serialize = "ENABLED")]
	SetEnabled,
	#[strum(serialize = "NOOP")]
	Noop = 255,
}

/// Convert an index to a RawAction
impl TryFrom<u32> for RawAction {
	type Error = ();

	fn try_from(value: u32) -> Result<Self, Self::Error> {
		match value {
			o if o == RawAction::TransferToken as u32 => Ok(RawAction::TransferToken),

			_ => Err(()),
		}
	}
}

/// Convert [RawOutgoingAction] to an index
impl From<RawAction> for u32 {
	fn from(val: RawAction) -> Self {
		val as u32
	}
}

pub trait ActionDecoder<AccountId> {
	type Error;

	fn decode(encoded: &[u8]) -> Result<Action<AccountId>, Self::Error>;
}

pub trait ActionEncoder<AccountId> {
	type Error;

	/// Encodes the given action for the proxy chain.
	fn encode(action: &Action<AccountId>) -> Result<Vec<u8>, Self::Error>;
}

/// Tracks the progress during `submit_message`, intended to be included in events.
#[derive(RuntimeDebug, Encode, Decode, TypeInfo, Clone, PartialEq)]
pub enum ProcessMessageResult {
	ParsingValueFailed,
	ActionFailed(RawAction),
	ActionSuccess,
	ProcessingFailed(DispatchError),
}

impl From<DispatchError> for ProcessMessageResult {
	fn from(value: DispatchError) -> Self {
		ProcessMessageResult::ProcessingFailed(value)
	}
}

pub type BalanceFor<T, I> =
	<<T as Config<I>>::Currency as Inspect<<T as frame_system::Config>::AccountId>>::Balance;

pub type SubjectFor<T> =
	Subject<<T as frame_system::Config>::AccountId, <T as frame_system::Config>::AccountId>;
