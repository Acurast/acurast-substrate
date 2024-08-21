use gstd::BlockNumber;
use sails_rs::prelude::{scale_codec::*, *};

pub type PubKey = [u8; 32];
pub type MessageIndex = u64;
pub type MsgId = [u8; 32];
pub type MessageNonce = [u8; 32];
pub type FunctionName = String;
pub type Payload = Vec<u8>;
pub type Contract = ActorId;
pub type Signature = [u8; 65];
pub type Public = Vec<u8>;
pub type Signatures = Vec<Signature>;
pub type AccountId = ActorId;
pub type Balance = u128;

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo)]
pub enum Event {
	OraclesUpdated,
	MessageReadyToSend { message: OutgoingMessageWithMeta },
	MessageDelivered { id: MsgId },
	MessageRemoved { id: MsgId },
	MessageStored { id: MsgId },
	MessageProcessed { id: MsgId },
	MessageProcessedWithErrors { id: MsgId },
}

/// Contract configurations are contained in this structure
#[derive(Debug, Clone, Eq, PartialEq, Encode, Decode, TypeInfo, Default)]
pub struct Config {
	/// Address allowed to manage the contract
	pub owner: ActorId,
	/// Flag that states if the contract is paused or not
	pub paused: bool,
	pub min_delivery_signatures: u8,
	pub min_receipt_signatures: u8,
	pub min_ttl: BlockNumber,
	/// ttl for incoming message before removed from ids index (to limit length of vector when reading `incoming_index`)
	pub incoming_ttl: BlockNumber,
}

#[derive(Debug, Clone, Eq, PartialEq, Encode, Decode, TypeInfo)]
pub enum ConfigureArgument {
	Owner(AccountId),
	Paused(bool),
	OraclePublicKeys(Vec<OracleUpdate>),
	MinDeliverySignatures(u8),
	MinReceiptSignatures(u8),
	MinTTL(BlockNumber),
	IncomingTTL(BlockNumber),
}

#[derive(Debug, Clone, Eq, PartialEq, Encode, Decode, TypeInfo)]
pub enum OracleUpdate {
	Add(Public),
	Remove(Public),
}

#[derive(Debug, Clone, Eq, PartialEq, Encode, Decode, TypeInfo)]
pub enum Subject {
	Acurast(AccountId),
	Vara(ActorId),
}

#[derive(Debug, Clone, Eq, PartialEq, Encode, Decode, TypeInfo)]
pub struct Message {
	pub id: MsgId,
	pub sender: Subject,
	pub nonce: MessageNonce,
	pub recipient: Subject,
	pub payload: Payload,
	// pub amount: u128,
}

#[derive(Debug, Clone, Eq, PartialEq, Encode, Decode, TypeInfo)]
pub struct OutgoingMessageWithMeta {
	pub message: Message,
	pub current_block: BlockNumber,
	pub ttl_block: BlockNumber,
	pub fee: Balance,
	/// The payer of the fee. Not necessarily the sender of the message.
	pub payer: AccountId,
}

#[derive(Debug, Clone, Eq, PartialEq, Encode, Decode, TypeInfo)]
pub struct IncomingMessageWithMeta {
	pub message: Message,
	pub current_block: BlockNumber,
	pub relayer: AccountId,
}

