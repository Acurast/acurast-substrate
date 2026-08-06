use derive_more::Display;
use frame_support::pallet_prelude::*;
use parity_scale_codec::{Decode, Encode};
use scale_info::TypeInfo;

pub mod ethereum;

/// Errors returned by decoders.
#[derive(Debug, Encode, Decode, MaxEncodedLen, TypeInfo, Clone, Eq, PartialEq, Display)]
pub enum ActionDecoderError {
	InvalidAction,
	UnsupportedAction,
	InvalidActionPayload,
	CouldNotConvertAccountId,
}

#[derive(Debug, Encode, Decode, MaxEncodedLen, TypeInfo, Clone, Eq, PartialEq, Display)]
pub enum ActionEncoderError {
	UnsupportedProxy,
}
