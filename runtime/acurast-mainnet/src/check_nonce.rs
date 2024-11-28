use acurast_runtime_common::utils::get_fee_payer;
use frame_support::dispatch::DispatchInfo;
use parity_scale_codec::{Decode, Encode};
use scale_info::TypeInfo;
use sp_runtime::{
	traits::{DispatchInfoOf, Dispatchable, One, SignedExtension, Zero},
	transaction_validity::{
		InvalidTransaction, TransactionLongevity, TransactionValidity, TransactionValidityError,
		ValidTransaction,
	},
};
use sp_std::prelude::*;

use crate::{ProcessorPairingProvider, Runtime};

#[derive(Encode, Decode, Clone, Eq, PartialEq, TypeInfo)]
#[scale_info(skip_type_params(T))]
pub struct CheckNonce(#[codec(compact)] pub <Runtime as frame_system::Config>::Nonce);

impl CheckNonce {
	/// utility constructor. Used only in client/factory code.
	pub fn from(nonce: <Runtime as frame_system::Config>::Nonce) -> Self {
		Self(nonce)
	}
}

impl sp_std::fmt::Debug for CheckNonce {
	#[cfg(feature = "std")]
	fn fmt(&self, f: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
		write!(f, "CheckNonce({})", self.0)
	}

	#[cfg(not(feature = "std"))]
	fn fmt(&self, _: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
		Ok(())
	}
}

impl SignedExtension for CheckNonce
where
	<Runtime as frame_system::Config>::RuntimeCall: Dispatchable<Info = DispatchInfo>,
{
	type AccountId = <Runtime as frame_system::Config>::AccountId;
	type Call = <Runtime as frame_system::Config>::RuntimeCall;
	type AdditionalSigned = ();
	type Pre = ();
	const IDENTIFIER: &'static str = "CheckNonce";

	fn additional_signed(&self) -> sp_std::result::Result<(), TransactionValidityError> {
		Ok(())
	}

	fn pre_dispatch(
		self,
		who: &Self::AccountId,
		call: &Self::Call,
		_info: &DispatchInfoOf<Self::Call>,
		_len: usize,
	) -> Result<(), TransactionValidityError> {
		let fee_payer = get_fee_payer::<Runtime, ProcessorPairingProvider>(who, call);
		let fee_payer_account = frame_system::Account::<Runtime>::get(&fee_payer);
		if fee_payer_account.providers.is_zero() && fee_payer_account.sufficients.is_zero() {
			// Nonce storage not paid for
			return Err(InvalidTransaction::Payment.into());
		}
		let mut account = if &fee_payer != who {
			frame_system::Account::<Runtime>::get(who)
		} else {
			fee_payer_account
		};
		if self.0 != account.nonce {
			return Err(if self.0 < account.nonce {
				InvalidTransaction::Stale
			} else {
				InvalidTransaction::Future
			}
			.into());
		}
		account.nonce += <Runtime as frame_system::Config>::Nonce::one();
		frame_system::Account::<Runtime>::insert(who, account);
		Ok(())
	}

	fn validate(
		&self,
		who: &Self::AccountId,
		call: &Self::Call,
		_info: &DispatchInfoOf<Self::Call>,
		_len: usize,
	) -> TransactionValidity {
		let fee_payer = get_fee_payer::<Runtime, ProcessorPairingProvider>(who, call);
		let fee_payer_account = frame_system::Account::<Runtime>::get(&fee_payer);
		if fee_payer_account.providers.is_zero() && fee_payer_account.sufficients.is_zero() {
			// Nonce storage not paid for
			return InvalidTransaction::Payment.into();
		}
		let account = if &fee_payer != who {
			frame_system::Account::<Runtime>::get(who)
		} else {
			fee_payer_account
		};
		if self.0 < account.nonce {
			return InvalidTransaction::Stale.into();
		}

		let provides = vec![Encode::encode(&(who, self.0))];
		let requires = if account.nonce < self.0 {
			vec![Encode::encode(&(who, self.0 - <Runtime as frame_system::Config>::Nonce::one()))]
		} else {
			vec![]
		};

		Ok(ValidTransaction {
			priority: 0,
			requires,
			provides,
			longevity: TransactionLongevity::max_value(),
			propagate: true,
		})
	}
}
