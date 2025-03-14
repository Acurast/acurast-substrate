use core::{marker::PhantomData, ops};
use parity_scale_codec::{Decode, Encode};
use scale_info::TypeInfo;

use sp_core::RuntimeDebug;
use sp_std::prelude::*;

use pallet_acurast::MultiOrigin;

use super::*;
use crate::{Action, ActionDecoder, ActionEncoder, RawAction};

const fn action_id_range() -> ops::Range<usize> {
	3 * 8..4 * 8
}

const fn amount_range() -> ops::Range<usize> {
	32 + 3 * 8..32 + 4 * 8
}

const fn asset_id_range() -> ops::Range<usize> {
	2 * 32 + 3 * 8..2 * 32 + 4 * 8
}

const fn transfer_nonce_range() -> ops::Range<usize> {
	3 * 32 + 3 * 8..3 * 32 + 4 * 8
}

const fn dest_range() -> ops::Range<usize> {
	4 * 32 + 12..4 * 32 + 32
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
		if encoded.len() != 160 {
			Err(Self::Error::CouldNotDecodeAction)?;
		}
		let raw_action: RawAction = u32::from_be_bytes(
			encoded[3 * 8..4 * 8]
				.try_into()
				.map_err(|_| ActionDecoderError::CouldNotDecodeAction)?,
		)
		.try_into()
		.map_err(|_err| Self::Error::CouldNotDecodeAction)?;

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
				let amount: u32 = u32::from_be_bytes(
					encoded[amount_range()]
						.try_into()
						.map_err(|_| ActionDecoderError::CouldNotDecodeAction)?,
				);
				let asset_id: u32 = u32::from_be_bytes(
					encoded[asset_id_range()]
						.try_into()
						.map_err(|_| ActionDecoderError::CouldNotDecodeAction)?,
				);
				let transfer_nonce = u32::from_be_bytes(
					encoded[transfer_nonce_range()]
						.try_into()
						.map_err(|_| ActionDecoderError::CouldNotDecodeAction)?,
				);
				let dest = convert_account_id::<AccountId, AccountConverter>(
					encoded[dest_range()]
						.try_into()
						.map_err(|_| ActionDecoderError::CouldNotDecodeAction)?,
				)?;
				Ok(Action::TransferToken(
					amount as u128,
					(asset_id != 0u32).then_some(asset_id as u128),
					transfer_nonce,
					MultiOrigin::Acurast(dest),
				))
			},
			RawAction::Noop => Ok(Action::Noop),
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
					let mut buffer = vec![0u8; 32];

					let raw_action: RawAction = action.into();
					let raw_action_encoded: u32 = raw_action.into();
					buffer[action_id_range()].copy_from_slice(&raw_action_encoded.to_be_bytes());
					buffer[amount_range()].copy_from_slice(&(*amount as u32).to_be_bytes());
					buffer[asset_id_range()]
						.copy_from_slice(&(asset_id.unwrap_or_default()).to_be_bytes());
					buffer[transfer_nonce_range()].copy_from_slice(&transfer_nonce.to_be_bytes());
					buffer[dest_range()].copy_from_slice(&account_id.0);

					buffer.to_vec()
				},
				_ => Err(ActionEncoderError::UnsupportedProxy)?,
			},
			Action::Noop => vec![],
		})
	}
}
