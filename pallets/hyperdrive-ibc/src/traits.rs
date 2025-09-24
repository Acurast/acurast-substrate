use frame_support::{dispatch::DispatchResult, weights::Weight};
use frame_system::pallet_prelude::BlockNumberFor;
use sp_std::prelude::*;

use crate::{MessageNonce, SubjectFor};

/// Weight functions needed for pallet_acurast_hyperdrive_ibc.
pub trait WeightInfo {
	fn update_oracles(l: u32) -> Weight;
	fn send_test_message() -> Weight;
	fn confirm_message_delivery() -> Weight;
	fn remove_message() -> Weight;
	fn receive_message() -> Weight;
	fn clean_incoming() -> Weight;
}

pub trait MessageSender<T: frame_system::Config, Balance> {
	fn send_message(
		sender: SubjectFor<T>,
		payer: &T::AccountId,
		nonce: MessageNonce,
		recipient: SubjectFor<T>,
		payload: Vec<u8>,
		ttl: BlockNumberFor<T>,
		fee: Balance,
	) -> DispatchResult;
}
