use core::marker::PhantomData;
use frame_support::{
	dispatch::DispatchInfo,
	pallet_prelude::TransactionSource,
	sp_runtime::{
		traits::{
			AsSystemOriginSigner, DispatchInfoOf, Dispatchable, One, PostDispatchInfoOf,
			TransactionExtension, ValidateResult, Zero,
		},
		transaction_validity::{
			InvalidTransaction, TransactionLongevity, TransactionValidityError, ValidTransaction,
		},
		DispatchResult, Saturating,
	},
	weights::Weight,
	RuntimeDebugNoBound,
};
use parity_scale_codec::{Decode, DecodeWithMemTracking, Encode};
use scale_info::TypeInfo;
use sp_std::vec;

use pallet_acurast_processor_manager::{Config as ProcessorManagerConfig, OnboardingProvider};

#[derive(Encode, Decode, DecodeWithMemTracking, Clone, Eq, PartialEq, TypeInfo)]
#[scale_info(skip_type_params(T, OP))]
pub struct CheckNonce<
	T: ProcessorManagerConfig,
	OP: OnboardingProvider<T> + Eq + Clone + Send + Sync + 'static,
> {
	#[codec(compact)]
	pub nonce: T::Nonce,
	#[codec(skip)]
	_phantom_data: PhantomData<OP>,
}

impl<T: ProcessorManagerConfig, OP: OnboardingProvider<T> + Eq + Clone + Send + Sync + 'static>
	CheckNonce<T, OP>
{
	/// utility constructor. Used only in client/factory code.
	pub fn from(nonce: T::Nonce) -> Self {
		Self { nonce, _phantom_data: Default::default() }
	}
}

impl<T: ProcessorManagerConfig, OP: OnboardingProvider<T> + Eq + Clone + Send + Sync + 'static>
	core::fmt::Debug for CheckNonce<T, OP>
{
	#[cfg(feature = "std")]
	fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
		write!(f, "CheckNonce({})", self.nonce)
	}

	#[cfg(not(feature = "std"))]
	fn fmt(&self, _: &mut core::fmt::Formatter) -> core::fmt::Result {
		Ok(())
	}
}

/// Operation to perform from `validate` to `prepare` in [`CheckNonce`] transaction extension.
#[derive(RuntimeDebugNoBound)]
pub enum Val<T: frame_system::Config> {
	/// Account and its nonce to check for.
	CheckNonce((T::AccountId, T::Nonce)),
	/// Weight to refund.
	Refund(Weight),
}

/// Operation to perform from `prepare` to `post_dispatch_details` in [`CheckNonce`] transaction
/// extension.
#[derive(RuntimeDebugNoBound)]
pub enum Pre {
	/// The transaction extension weight should not be refunded.
	NonceChecked,
	/// The transaction extension weight should be refunded.
	Refund(Weight),
}

impl<T: ProcessorManagerConfig, OP: OnboardingProvider<T> + Eq + Clone + Send + Sync + 'static>
	TransactionExtension<T::RuntimeCall> for CheckNonce<T, OP>
where
	T::RuntimeCall: Dispatchable<Info = DispatchInfo>,
	<T::RuntimeCall as Dispatchable>::RuntimeOrigin: AsSystemOriginSigner<T::AccountId> + Clone,
{
	const IDENTIFIER: &'static str = "CheckNonce";
	type Implicit = ();
	type Val = Val<T>;
	type Pre = Pre;

	fn weight(&self, _: &T::RuntimeCall) -> Weight {
		<T::ExtensionsWeightInfo as frame_system::ExtensionsWeightInfo>::check_nonce()
	}

	fn validate(
		&self,
		origin: <T as frame_system::Config>::RuntimeOrigin,
		call: &T::RuntimeCall,
		_info: &DispatchInfoOf<T::RuntimeCall>,
		_len: usize,
		_self_implicit: Self::Implicit,
		_inherited_implication: &impl Encode,
		_source: TransactionSource,
	) -> ValidateResult<Self::Val, T::RuntimeCall> {
		let Some(who) = origin.as_system_origin_signer() else {
			return Ok((Default::default(), Val::Refund(self.weight(call)), origin));
		};
		let fee_payer = OP::fee_payer(who, call);
		let fee_payer_account = frame_system::Account::<T>::get(&fee_payer);

		if (!OP::is_funding_call(call) || OP::can_fund_processor_onboarding(who).is_none())
			&& fee_payer_account.providers.is_zero()
			&& fee_payer_account.sufficients.is_zero()
		{
			// Nonce storage not paid for
			return Err(InvalidTransaction::Payment.into());
		}

		let account = if &fee_payer != who {
			frame_system::Account::<T>::get(who)
		} else {
			fee_payer_account
		};
		if self.nonce < account.nonce {
			return Err(InvalidTransaction::Stale.into());
		}

		let provides = vec![Encode::encode(&(&who, self.nonce))];
		let requires = if account.nonce < self.nonce {
			vec![Encode::encode(&(&who, self.nonce.saturating_sub(One::one())))]
		} else {
			vec![]
		};

		let validity = ValidTransaction {
			priority: 0,
			requires,
			provides,
			longevity: TransactionLongevity::MAX,
			propagate: true,
		};

		Ok((validity, Val::CheckNonce((who.clone(), account.nonce)), origin))
	}

	fn prepare(
		self,
		val: Self::Val,
		_origin: &T::RuntimeOrigin,
		_call: &T::RuntimeCall,
		_info: &DispatchInfoOf<T::RuntimeCall>,
		_len: usize,
	) -> Result<Self::Pre, TransactionValidityError> {
		let (who, mut nonce) = match val {
			Val::CheckNonce((who, nonce)) => (who, nonce),
			Val::Refund(weight) => return Ok(Pre::Refund(weight)),
		};

		// `self.nonce < nonce` already checked in `validate`.
		if self.nonce > nonce {
			return Err(InvalidTransaction::Future.into());
		}
		nonce += T::Nonce::one();
		frame_system::Account::<T>::mutate(who, |account| account.nonce = nonce);
		Ok(Pre::NonceChecked)
	}

	fn post_dispatch_details(
		pre: Self::Pre,
		_info: &DispatchInfo,
		_post_info: &PostDispatchInfoOf<T::RuntimeCall>,
		_len: usize,
		_result: &DispatchResult,
	) -> Result<Weight, TransactionValidityError> {
		match pre {
			Pre::NonceChecked => Ok(Weight::zero()),
			Pre::Refund(weight) => Ok(weight),
		}
	}
}
