use derive_more::Display;
use frame_support::pallet_prelude::*;
use parity_scale_codec::{Decode, Encode};
use scale_info::TypeInfo;

pub mod ethereum;

/// Errors returned by decoders.
#[derive(RuntimeDebug, Encode, Decode, MaxEncodedLen, TypeInfo, Clone, Eq, PartialEq, Display)]
pub enum ActionDecoderError {
	CouldNotDecodeAction,
	CouldNotConvertAccountId,
}

#[derive(RuntimeDebug, Encode, Decode, MaxEncodedLen, TypeInfo, Clone, Eq, PartialEq, Display)]
pub enum ActionEncoderError {
	UnsupportedProxy,
}
