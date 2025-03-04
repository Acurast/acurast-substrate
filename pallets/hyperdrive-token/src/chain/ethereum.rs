use core::marker::PhantomData;
use parity_scale_codec::{Decode, Encode};
use scale_info::TypeInfo;

use sp_core::RuntimeDebug;
use sp_std::prelude::*;

use pallet_acurast::MultiOrigin;

use super::*;
use crate::{Action, ActionDecoder, ActionEncoder, RawAction};

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
		let raw_action: RawAction = u32::from_be_bytes(encoded[0..4].try_into().unwrap())
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
				let amount: u32 = u32::from_be_bytes(encoded[4..8].try_into().unwrap());
				let asset_id: u32 = u32::from_be_bytes(encoded[8..12].try_into().unwrap());
				let transfer_nonce = u32::from_be_bytes(encoded[12..16].try_into().unwrap());
				let transfer_recipient = convert_account_id::<AccountId, AccountConverter>(
					encoded[16..36].try_into().unwrap(),
				)?;
				Ok(Action::TransferToken(
					amount as u128,
					(asset_id != 0u32).then_some(asset_id as u128),
					transfer_nonce,
					MultiOrigin::Acurast(transfer_recipient),
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
					buffer[0..4].copy_from_slice(&raw_action_encoded.to_be_bytes());
					buffer[4..8].copy_from_slice(&(*amount as u32).to_be_bytes());
					buffer[8..12].copy_from_slice(&(asset_id.unwrap_or_default()).to_be_bytes());
					buffer[12..16].copy_from_slice(&transfer_nonce.to_be_bytes());
					buffer[16..36].copy_from_slice(&account_id.0);

					buffer.to_vec()
				},
				_ => Err(ActionEncoderError::UnsupportedProxy)?,
			},
			Action::Noop => vec![],
		})
	}
}
