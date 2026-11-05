use crate::Config;
/// A compressed secp256r1 (P-256) public key, i.e. the key type of
/// [`acurast_p256_crypto::MultiSigner::P256`].
pub use acurast_p256_crypto::application_crypto::p256::Public as P256Public;
/// A recoverable secp256r1 (P-256) signature, i.e. the signature type of
/// [`acurast_p256_crypto::MultiSignature::P256`].
pub use acurast_p256_crypto::application_crypto::p256::Signature as P256Signature;
use frame_support::{
	pallet_prelude::*, storage::bounded_vec::BoundedVec, traits::fungible::Inspect,
};
use frame_system::pallet_prelude::BlockNumberFor;
use pallet_acurast::{Layer, MessageFeeProvider, MultiOrigin, Subject};
use parity_scale_codec::{Decode, Encode};
use scale_info::TypeInfo;
pub use sp_core::ecdsa::{
	Public, Signature, PUBLIC_KEY_SERIALIZED_SIZE, SIGNATURE_SERIALIZED_SIZE,
};
use sp_core::{ConstU32, H256};
use sp_std::prelude::*;

pub const SIGNATURES_MAX_LENGTH: u32 = 32;

pub type Signatures = BoundedVec<(Signature, Public), ConstU32<SIGNATURES_MAX_LENGTH>>;

/// secp256r1 (P-256) oracle signatures, checked against [`crate::OraclePublicKeysV2`].
pub type SignaturesV2 = BoundedVec<(P256Signature, P256Public), ConstU32<SIGNATURES_MAX_LENGTH>>;

/// A well-formed P-256 signature for the benchmarks: `b"acurast"` signed by the key with seed
/// `[1u8; 32]`, checked by the `p256_signature_vector` test.
#[cfg(any(test, feature = "runtime-benchmarks"))]
pub(crate) const BENCHMARK_P256_SIGNATURE: [u8; 65] = hex_literal::hex!("c2f38f1efce593920b17f0befda6a7590fc181f6d30b8ff5cdc4af1b503f4c55bf0146087d9f5f1026247a36b0c8b7fcab408e39a22a2956ae078b8de78770ec01");

pub const ORACLE_UPDATES_MAX_LENGTH: u32 = 50;
pub type OracleUpdates<T> = BoundedVec<OracleUpdateFor<T>, ConstU32<ORACLE_UPDATES_MAX_LENGTH>>;
pub type OracleUpdatesV2<T> = BoundedVec<OracleUpdateV2For<T>, ConstU32<ORACLE_UPDATES_MAX_LENGTH>>;

pub const MESSAGES_CLEANUP_MAX_LENGTH: u32 = 50;
pub type MessagesCleanup = BoundedVec<MessageId, ConstU32<MESSAGES_CLEANUP_MAX_LENGTH>>;

pub type OracleUpdateFor<T> = OracleUpdate<BlockNumberFor<T>>;
pub type OracleUpdateV2For<T> = OracleUpdateV2<BlockNumberFor<T>>;

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
	Debug, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo, Clone, PartialEq,
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

#[derive(Debug, Encode, Decode, DecodeWithMemTracking, TypeInfo, Clone, PartialEq)]
pub enum OracleUpdate<BlockNumber> {
	Add(Public, ActivityWindow<BlockNumber>),
	Remove(Public),
	Update(Public, ActivityWindow<BlockNumber>),
}

/// Like [`OracleUpdate`], for secp256r1 (P-256) oracles registered in
/// [`crate::OraclePublicKeysV2`].
#[derive(Debug, Encode, Decode, DecodeWithMemTracking, TypeInfo, Clone, PartialEq)]
pub enum OracleUpdateV2<BlockNumber> {
	Add(P256Public, ActivityWindow<BlockNumber>),
	Remove(P256Public),
	Update(P256Public, ActivityWindow<BlockNumber>),
}

/// The message (without metadata) that gets signed by oracle and verified by recipient.
#[derive(
	Debug, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo, Clone, PartialEq,
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
	Debug, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo, Clone, PartialEq,
)]
pub struct OutgoingMessageWithMeta<AccountId, Balance, BlockNumber, Contract> {
	pub message: Message<AccountId, Contract>,
	pub current_block: BlockNumber,
	pub ttl_block: BlockNumber,
	pub fee: Balance,
	/// The payer of the fee. Not necessarily the sender of the message.
	pub payer: AccountId,
}

impl<AccountId, Balance: Copy, BlockNumber, Contract> MessageFeeProvider<Balance>
	for OutgoingMessageWithMeta<AccountId, Balance, BlockNumber, Contract>
{
	fn get_fee(&self) -> Balance {
		self.fee
	}
}

/// A wrapper around an outgoing message containing metadata related to TTL.
#[derive(
	Debug, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo, Clone, PartialEq,
)]
pub struct IncomingMessageWithMeta<AccountId, BlockNumber, Contract> {
	pub message: Message<AccountId, Contract>,
	pub current_block: BlockNumber,
	pub relayer: MultiOrigin<AccountId>,
}

/// The part of the message that is passed on to final recipient, controlled by
/// [`MessageProcessor`].
#[derive(Debug, Encode, Decode, TypeInfo, Clone, PartialEq)]
pub struct MessageBody<AccountId, Contract> {
	pub sender: Subject<AccountId, Contract>,
	pub recipient: Subject<AccountId, Contract>,
	pub payload: Payload,
}

impl<AccountId, Contract> pallet_acurast::MessageBody<AccountId, Contract>
	for MessageBody<AccountId, Contract>
{
	fn sender(&self) -> &Subject<AccountId, Contract> {
		&self.sender
	}

	fn recipient(&self) -> &Subject<AccountId, Contract> {
		&self.recipient
	}

	fn payload(self) -> pallet_acurast::Payload {
		self.payload
	}
}

impl<AccountId, Contract> From<Message<AccountId, Contract>> for MessageBody<AccountId, Contract> {
	fn from(m: Message<AccountId, Contract>) -> Self {
		MessageBody { sender: m.sender, recipient: m.recipient, payload: m.payload }
	}
}

pub type MessageIndex = u64;
pub type MessageId = H256;
pub type MessageNonce = H256;

/// Tracks the progress during `submit_message`, intended to be included in events.
#[derive(Debug, Encode, Decode, TypeInfo, Clone, PartialEq)]
pub enum ProcessMessageResult {
	TTLExceeded,
	ProcessingFailed(DispatchError),
}

impl From<DispatchError> for ProcessMessageResult {
	fn from(value: DispatchError) -> Self {
		ProcessMessageResult::ProcessingFailed(value)
	}
}
