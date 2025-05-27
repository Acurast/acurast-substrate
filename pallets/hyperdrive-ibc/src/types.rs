use crate::Config;
use frame_support::{
	pallet_prelude::*, storage::bounded_vec::BoundedVec, traits::fungible::Inspect,
};
use frame_system::pallet_prelude::BlockNumberFor;
use pallet_acurast::AccountId20;
use pallet_acurast::MultiOrigin;
use parity_scale_codec::{Decode, Encode};
use scale_info::TypeInfo;
pub use sp_core::ecdsa::{
	Public, Signature, PUBLIC_KEY_SERIALIZED_SIZE, SIGNATURE_SERIALIZED_SIZE,
};
use sp_core::{crypto::AccountId32, ConstU32, H256};
use sp_runtime::RuntimeDebug;
use sp_std::prelude::*;

pub const SIGNATURES_MAX_LENGTH: u32 = 32;

pub type Signatures = BoundedVec<(Signature, Public), ConstU32<SIGNATURES_MAX_LENGTH>>;

pub const ORACLE_UPDATES_MAX_LENGTH: u32 = 50;
pub type OracleUpdates<T> = BoundedVec<OracleUpdateFor<T>, ConstU32<ORACLE_UPDATES_MAX_LENGTH>>;

pub const MESSAGES_CLEANUP_MAX_LENGTH: u32 = 50;
pub type MessagesCleanup = BoundedVec<MessageId, ConstU32<MESSAGES_CLEANUP_MAX_LENGTH>>;

pub type OracleUpdateFor<T> = OracleUpdate<BlockNumberFor<T>>;

pub type BalanceOf<T, I> =
	<<T as Config<I>>::Currency as Inspect<<T as frame_system::Config>::AccountId>>::Balance;

pub type MessageFor<T> =
	Message<<T as frame_system::Config>::AccountId, <T as frame_system::Config>::AccountId>;

pub type OutgoingMessageWithMetaFor<T, I> = OutgoingMessageWithMeta<
	<T as frame_system::Config>::AccountId,
	BalanceOf<T, I>,
	BlockNumberFor<T>,
	<T as frame_system::Config>::AccountId,
>;

pub type IncomingMessageWithMetaFor<T> = IncomingMessageWithMeta<
	<T as frame_system::Config>::AccountId,
	BlockNumberFor<T>,
	<T as frame_system::Config>::AccountId,
>;

pub type SubjectFor<T> =
	Subject<<T as frame_system::Config>::AccountId, <T as frame_system::Config>::AccountId>;

pub type LayerFor<T> =
	Layer<<T as frame_system::Config>::AccountId, <T as frame_system::Config>::AccountId>;

pub const PAYLOAD_MAX_LENGTH: u32 = 1024;

pub type Payload = BoundedVec<u8, ConstU32<PAYLOAD_MAX_LENGTH>>;

/// Defines the transmitter activity window.
#[derive(
	RuntimeDebug, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo, Clone, PartialEq,
)]
pub struct ActivityWindow<BlockNumber> {
	/// From this block on, the transmitter is permitted to submit Merkle roots.
	pub start_block: BlockNumber,
	/// From this block on, the transmitter is not permitted to submit any Merkle root.
	pub end_block: Option<BlockNumber>,
}

impl<BlockNumber: From<u8>> Default for ActivityWindow<BlockNumber> {
	fn default() -> Self {
		Self { start_block: BlockNumber::from(0), end_block: None }
	}
}

#[derive(RuntimeDebug, Encode, Decode, DecodeWithMemTracking, TypeInfo, Clone, PartialEq)]
pub enum OracleUpdate<BlockNumber> {
	Add(Public, ActivityWindow<BlockNumber>),
	Remove(Public),
	Update(Public, ActivityWindow<BlockNumber>),
}

/// The message (without metadata) that gets signed by oracle and verified by recipient.
#[derive(
	RuntimeDebug, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo, Clone, PartialEq,
)]
pub struct Message<AccountId, Contract> {
	pub id: MessageId,
	pub sender: Subject<AccountId, Contract>,
	pub nonce: MessageNonce,
	pub recipient: Subject<AccountId, Contract>,
	/// The payload contains also the "endpoint" depending on the destination chain,
	/// e.g. it can be the encoded extrinsic for layer 0 recipients.
	pub payload: Payload,
}

/// A wrapper around an outgoing message containing metadata related to fee handling and TTL.
#[derive(
	RuntimeDebug, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo, Clone, PartialEq,
)]
pub struct OutgoingMessageWithMeta<AccountId, Balance, BlockNumber, Contract> {
	pub message: Message<AccountId, Contract>,
	pub current_block: BlockNumber,
	pub ttl_block: BlockNumber,
	pub fee: Balance,
	/// The payer of the fee. Not necessarily the sender of the message.
	pub payer: AccountId,
}

/// A wrapper around an outgoing message containing metadata related to TTL.
#[derive(
	RuntimeDebug, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo, Clone, PartialEq,
)]
pub struct IncomingMessageWithMeta<AccountId, BlockNumber, Contract> {
	pub message: Message<AccountId, Contract>,
	pub current_block: BlockNumber,
	pub relayer: MultiOrigin<AccountId>,
}

/// The part of the message that is passed on to final recipient, controlled by
/// [`MessageProcessor`].
#[derive(RuntimeDebug, Encode, Decode, TypeInfo, Clone, PartialEq)]
pub struct MessageBody<AccountId, Contract> {
	pub sender: Subject<AccountId, Contract>,
	pub recipient: Subject<AccountId, Contract>,
	pub payload: Payload,
}

impl<AccountId, Contract> From<Message<AccountId, Contract>> for MessageBody<AccountId, Contract> {
	fn from(m: Message<AccountId, Contract>) -> Self {
		MessageBody { sender: m.sender, recipient: m.recipient, payload: m.payload }
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
	Eq,
	PartialEq,
)]
pub enum Subject<AccountId, Contract> {
	Acurast(Layer<AccountId, Contract>),
	AlephZero(Layer<AccountId32, Contract>),
	Vara(Layer<AccountId32, Contract>),
	Ethereum(Layer<AccountId20, AccountId20>),
	Solana(Layer<AccountId32, AccountId32>),
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

pub type MessageIndex = u64;
pub type MessageId = H256;
pub type MessageNonce = H256;

pub trait MessageProcessor<AccountId, Contract> {
	fn process(message: MessageBody<AccountId, Contract>) -> DispatchResultWithPostInfo;
}

/// Tracks the progress during `submit_message`, intended to be included in events.
#[derive(RuntimeDebug, Encode, Decode, TypeInfo, Clone, PartialEq)]
pub enum ProcessMessageResult {
	TTLExceeded,
	ProcessingFailed(DispatchError),
}

impl From<DispatchError> for ProcessMessageResult {
	fn from(value: DispatchError) -> Self {
		ProcessMessageResult::ProcessingFailed(value)
	}
}
