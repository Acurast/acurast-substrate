use core::marker::PhantomData;

use frame_support::{dispatch::DispatchInfo, traits::IsType};
use parity_scale_codec::{Decode, DecodeWithMemTracking, Encode};
use scale_info::TypeInfo;
use sp_runtime::{
	traits::{DispatchInfoOf, Dispatchable, IdentifyAccount, One, SignedExtension, Verify, Zero},
	transaction_validity::{
		InvalidTransaction, TransactionLongevity, TransactionValidity, TransactionValidityError,
		ValidTransaction,
	},
};
use sp_std::prelude::*;

use crate::utils::{get_fee_payer, PairingProvider};

#[derive(Encode, Decode, DecodeWithMemTracking, Clone, Eq, PartialEq, TypeInfo)]
#[scale_info(skip_type_params(Runtime, P))]
pub struct CheckNonce<
	Runtime: frame_system::Config + pallet_acurast_processor_manager::Config,
	P: PairingProvider<Runtime> + Eq + Clone + Send + Sync + 'static,
> {
	#[codec(compact)]
	pub nonce: Runtime::Nonce,
	#[codec(skip)]
	_phantom_data: PhantomData<P>,
}

impl<
		Runtime: frame_system::Config + pallet_acurast_processor_manager::Config,
		P: PairingProvider<Runtime> + Eq + Clone + Send + Sync + 'static,
	> CheckNonce<Runtime, P>
{
	/// utility constructor. Used only in client/factory code.
	pub fn from(nonce: Runtime::Nonce) -> Self {
		Self { nonce, _phantom_data: Default::default() }
	}
}

impl<
		Runtime: frame_system::Config + pallet_acurast_processor_manager::Config,
		P: PairingProvider<Runtime> + Eq + Clone + Send + Sync + 'static,
	> sp_std::fmt::Debug for CheckNonce<Runtime, P>
{
	#[cfg(feature = "std")]
	fn fmt(&self, f: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
		write!(f, "CheckNonce({})", self.nonce)
	}

	#[cfg(not(feature = "std"))]
	fn fmt(&self, _: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
		Ok(())
	}
}

impl<Runtime, P> SignedExtension for CheckNonce<Runtime, P>
where
	Runtime: frame_system::Config + pallet_acurast_processor_manager::Config,
	P: PairingProvider<Runtime> + Eq + Clone + Send + Sync + 'static,
	Runtime::RuntimeCall: Dispatchable<Info = DispatchInfo>,
	Runtime::AccountId: IsType<<<<Runtime as pallet_acurast_processor_manager::Config>::Proof as Verify>::Signer as IdentifyAccount>::AccountId>
{
	type AccountId = Runtime::AccountId;
	type Call = Runtime::RuntimeCall;
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
		let fee_payer = get_fee_payer::<Runtime, P>(who, call);
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
		if self.nonce != account.nonce {
			return Err(if self.nonce < account.nonce {
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
		let fee_payer = get_fee_payer::<Runtime, P>(who, call);
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
		if self.nonce < account.nonce {
			return InvalidTransaction::Stale.into();
		}

		let provides = vec![Encode::encode(&(who, self.nonce))];
		let requires = if account.nonce < self.nonce {
			vec![Encode::encode(&(who, self.nonce - <Runtime as frame_system::Config>::Nonce::one()))]
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
