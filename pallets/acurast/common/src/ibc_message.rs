use core::fmt::Debug;

use frame_support::{
	dispatch::DispatchResultWithPostInfo, pallet_prelude::*, sp_runtime::DispatchError, BoundedVec,
};
use parity_scale_codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_core::{crypto::AccountId32, ConstU32, RuntimeDebug, H256};
use sp_std::prelude::*;

use crate::AccountId20;

#[derive(
	RuntimeDebug,
	Encode,
	Decode,
	DecodeWithMemTracking,
	MaxEncodedLen,
	TypeInfo,
	Clone,
	Eq,
	PartialEq,
)]
pub enum Subject<AccountId, Contract> {
	Acurast(Layer<AccountId, Contract>),
	AlephZero(Layer<AccountId32, Contract>),
	Vara(Layer<AccountId32, Contract>),
	Ethereum(Layer<AccountId20, AccountId20>),
	Solana(Layer<AccountId32, AccountId32>),
	AcurastCanary(Layer<AccountId, Contract>),
}

#[derive(
	RuntimeDebug,
	Encode,
	Decode,
	DecodeWithMemTracking,
	MaxEncodedLen,
	TypeInfo,
	Clone,
	Eq,
	PartialEq,
)]
pub enum Layer<AccountId, C> {
	/// A sender/recipient extrinsic. In case of a sender, it should hold the pallet_account of either this pallet
	/// if `hyperdrive_ibc::send_message`-extrinsic sent the message or the (internal) caller of `hyperdrive_ibc::do_send_message`.
	Extrinsic(AccountId),
	Contract(ContractCall<C>),
}

/// A contract call acting as a sender/recipient of a message.
///
/// See how to invoke another contract: https://use.ink/4.x/basics/cross-contract-calling#callbuilder
#[derive(
	RuntimeDebug,
	Encode,
	Decode,
	DecodeWithMemTracking,
	MaxEncodedLen,
	TypeInfo,
	Clone,
	Eq,
	PartialEq,
)]
pub struct ContractCall<C> {
	pub contract: C,
	/// Selector for the message of `contract` to send payload to,
	/// as the only argument.
	pub selector: Option<[u8; 4]>,
}

pub trait MessageSender<AccountId, Contract, Balance, BlockNumber> {
	type MessageNonce: Member + Debug;
	type OutgoingMessage: MessageFeeProvider<Balance>;

	fn send_message(
		sender_extrinsic: &AccountId,
		payer: &AccountId,
		nonce: Self::MessageNonce,
		recipient: Subject<AccountId, Contract>,
		payload: Vec<u8>,
		ttl: BlockNumber,
		fee: Balance,
	) -> Result<(Self::OutgoingMessage, Option<Self::OutgoingMessage>), DispatchError>;
}

pub trait MessageFeeProvider<Balance> {
	fn get_fee(&self) -> Balance;
}

impl<Balance> MessageFeeProvider<Balance> for ()
where
	Balance: Zero,
{
	fn get_fee(&self) -> Balance {
		Balance::zero()
	}
}

impl<AccountId, Contract, Balance, BlockNumber>
	MessageSender<AccountId, Contract, Balance, BlockNumber> for ()
where
	AccountId: From<[u8; 32]>,
	Balance: Zero,
{
	type MessageNonce = H256;
	type OutgoingMessage = ();

	fn send_message(
		_sender: &AccountId,
		_payer: &AccountId,
		_nonce: Self::MessageNonce,
		_recipient: Subject<AccountId, Contract>,
		_payload: Vec<u8>,
		_ttl: BlockNumber,
		_fee: Balance,
	) -> Result<(Self::OutgoingMessage, Option<Self::OutgoingMessage>), DispatchError> {
		Ok(((), None))
	}
}

pub trait MessageProcessor<AccountId, Contract> {
	fn process(message: impl MessageBody<AccountId, Contract>) -> DispatchResultWithPostInfo;
}

pub const PAYLOAD_MAX_LENGTH: u32 = 1024;
pub type Payload = BoundedVec<u8, ConstU32<PAYLOAD_MAX_LENGTH>>;

pub trait MessageBody<AccountId, Contract> {
	fn sender(&self) -> &Subject<AccountId, Contract>;
	fn recipient(&self) -> &Subject<AccountId, Contract>;
	fn payload(self) -> Payload;
}
