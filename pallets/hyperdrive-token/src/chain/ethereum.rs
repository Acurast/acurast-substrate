use core::{marker::PhantomData, ops};
use parity_scale_codec::{Decode, Encode};
use scale_info::TypeInfo;

use sp_core::RuntimeDebug;
use sp_std::prelude::*;

use pallet_acurast::MultiOrigin;

use super::*;
use crate::{Action, ActionDecoder, ActionEncoder, RawAction};

const fn action_id_range() -> ops::Range<usize> {
	0..4
}

const fn amount_range() -> ops::Range<usize> {
	4..20
}

const fn enabled_index() -> usize {
	4
}

const fn asset_id_range() -> ops::Range<usize> {
	20..24
}

const fn transfer_nonce_range() -> ops::Range<usize> {
	24..32
}

const fn ethereum_dest_range() -> ops::Range<usize> {
	32..52
}

const fn acurast_dest_range() -> ops::Range<usize> {
	32..64
}

#[derive(RuntimeDebug, Encode, Decode, TypeInfo, Clone, Eq, PartialEq)]
#[scale_info(skip_type_params(AccountConverter))]
pub struct EthereumActionDecoder<I, AccountConverter, AccountId> {
	#[cfg(any(test, feature = "runtime-benchmarks"))]
	pub marker: PhantomData<(I, AccountConverter, AccountId)>,
	#[cfg(not(any(test, feature = "runtime-benchmarks")))]
	marker: PhantomData<(I, AccountConverter, AccountId)>,
}

impl<I: 'static, AccountConverter, AccountId> ActionDecoder<AccountId>
	for EthereumActionDecoder<I, AccountConverter, AccountId>
where
	AccountConverter: TryFrom<Vec<u8>> + Into<AccountId>,
{
	type Error = ActionDecoderError;

	fn decode(encoded: &[u8]) -> Result<Action<AccountId>, Self::Error> {
		if encoded.len() < 4 {
			Err(Self::Error::InvalidAction)?;
		}
		let raw_action: RawAction = u32::from_be_bytes(
			encoded[action_id_range()]
				.try_into()
				.map_err(|_| ActionDecoderError::UnsupportedAction)?,
		)
		.try_into()
		.map_err(|_err| Self::Error::UnsupportedAction)?;

		fn convert_account_id<Account, AccountConverter: TryFrom<Vec<u8>> + Into<Account>>(
			bytes: &[u8; 32],
		) -> Result<Account, ActionDecoderError> {
			let parsed: AccountConverter = bytes
				.to_vec()
				.try_into()
				.map_err(|_| ActionDecoderError::CouldNotConvertAccountId)?;
			Ok(parsed.into())
		}

		match raw_action {
			RawAction::TransferToken => {
				if encoded.len() != 64 {
					Err(Self::Error::InvalidActionPayload)?;
				}

				let amount = u128::from_be_bytes(
					encoded[amount_range()]
						.try_into()
						.map_err(|_| ActionDecoderError::InvalidActionPayload)?,
				);
				let asset_id: u32 = u32::from_be_bytes(
					encoded[asset_id_range()]
						.try_into()
						.map_err(|_| ActionDecoderError::InvalidActionPayload)?,
				);
				let transfer_nonce = u64::from_be_bytes(
					encoded[transfer_nonce_range()]
						.try_into()
						.map_err(|_| ActionDecoderError::InvalidActionPayload)?,
				);
				let dest = convert_account_id::<AccountId, AccountConverter>(
					encoded[acurast_dest_range()]
						.try_into()
						.map_err(|_| ActionDecoderError::InvalidActionPayload)?,
				)?;
				Ok(Action::TransferToken(
					amount,
					(asset_id != 0u32).then_some(asset_id),
					transfer_nonce,
					MultiOrigin::Acurast(dest),
				))
			},
			RawAction::Noop => Ok(Action::Noop),
			RawAction::SetEnabled => {
				if encoded.len() != 5 {
					return Err(Self::Error::InvalidActionPayload);
				}
				let enabled = encoded[enabled_index()];
				Ok(Action::SetEnabled(enabled != 0))
			},
		}
	}
}

pub struct EthereumActionEncoder;

impl<AccountId> ActionEncoder<AccountId> for EthereumActionEncoder {
	type Error = ActionEncoderError;

	fn encode(action: &Action<AccountId>) -> Result<Vec<u8>, Self::Error> {
		Ok(match action {
			Action::TransferToken(amount, asset_id, transfer_nonce, dest) => match dest {
				MultiOrigin::Ethereum20(account_id) => {
					let mut buffer = [0u8; 52];

					let raw_action: RawAction = action.into();
					let raw_action_encoded: u32 = raw_action.into();
					buffer[action_id_range()].copy_from_slice(&raw_action_encoded.to_be_bytes());
					buffer[amount_range()].copy_from_slice(&(*amount).to_be_bytes());
					buffer[asset_id_range()]
						.copy_from_slice(&(asset_id.unwrap_or_default()).to_be_bytes());
					buffer[transfer_nonce_range()].copy_from_slice(&transfer_nonce.to_be_bytes());
					buffer[ethereum_dest_range()].copy_from_slice(&account_id.0);

					buffer.to_vec()
				},
				_ => Err(ActionEncoderError::UnsupportedProxy)?,
			},
			Action::Noop => vec![],
			Action::SetEnabled(enabled) => {
				let mut buffer = [0u8; 5];

				let raw_action: RawAction = action.into();
				let raw_action_encoded: u32 = raw_action.into();
				buffer[action_id_range()].copy_from_slice(&raw_action_encoded.to_be_bytes());
				buffer[enabled_index()] = if *enabled { 1u8 } else { 0u8 };

				buffer.to_vec()
			},
		})
	}
}

#[cfg(test)]
mod tests {
	use super::ActionDecoder;
	use super::*;
	use derive_more::{From, Into};
	use frame_support::assert_ok;
	use hex_literal::hex;
	use sp_runtime::AccountId32;

	/// Wrapper around [`AccountId32`] to allow the implementation of [`TryFrom<Vec<u8>>`].
	#[derive(Debug, From, Into, Clone, Eq, PartialEq)]
	pub struct MockAccountId(AccountId32);
	impl TryFrom<Vec<u8>> for MockAccountId {
		type Error = ();

		fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
			let a: [u8; 32] = value.try_into().map_err(|_| ())?;
			Ok(MockAccountId(AccountId32::new(a)))
		}
	}

	#[test]
	/// Tests decoding into solidity tuple
	///
	/// uint32(0), // action_id (0 for transfer)
	/// uint128 // amount
	/// uint32(0), // assetId (0 for native token)
	/// uint32 // transferNonce
	/// dest // bytes32
	///
	/// Example: This payload is split like
	///
	/// ```
	/// 0x00000000000000000000000000000000000003e8000000000000000000000000185a8b5f92ecd348ed9b12a047ca2b28488b1398065a8dff8dcf886245f9280b
	///   |       |                               |       |               |
	///   action  amount                          assetId transferNonce   dest
	/// ```
	fn decode() {
		assert_ok!(<EthereumActionDecoder::<(), MockAccountId, AccountId32> as ActionDecoder<AccountId32>>::decode(&hex!("00000000000000000000000000000000000003e8000000000000000000000000185a8b5f92ecd348ed9b12a047ca2b28488b1398065a8dff8dcf886245f9280b")));
	}
}
