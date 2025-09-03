use core::{fmt::Debug, marker::PhantomData};
use frame_support::{
	dispatch::DispatchInfo,
	pallet_prelude::TransactionSource,
	sp_runtime::{
		traits::{
			AsSystemOriginSigner, DispatchInfoOf, Dispatchable, TransactionExtension,
			ValidateResult,
		},
		transaction_validity::{InvalidTransaction, TransactionValidityError, ValidTransaction},
	},
	traits::IsType,
	weights::Weight,
	RuntimeDebugNoBound,
};
use frame_system::ensure_signed;
use pallet_acurast_processor_manager::{Config as ProcessorManagerConfig, OnboardingProvider};
use parity_scale_codec::{Decode, DecodeWithMemTracking, Encode};
use scale_info::TypeInfo;
use sp_runtime::traits::{IdentifyAccount, Verify};

use crate::utils::PairingProvider;

#[derive(Encode, Decode, DecodeWithMemTracking, Clone, Eq, PartialEq, TypeInfo)]
#[scale_info(skip_type_params(T, P, OP))]
pub struct Onboarding<T: ProcessorManagerConfig, P: PairingProvider<T>, OP: OnboardingProvider<T>> {
	#[codec(skip)]
	_phantom_data: PhantomData<(T, P, OP)>,
}

impl<T: ProcessorManagerConfig, P: PairingProvider<T>, OP: OnboardingProvider<T>> Debug
	for Onboarding<T, P, OP>
{
	#[cfg(feature = "std")]
	fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
		write!(f, "Onboarding")
	}

	#[cfg(not(feature = "std"))]
	fn fmt(&self, _: &mut core::fmt::Formatter) -> core::fmt::Result {
		Ok(())
	}
}

const PAIRING_VALIDATION_ERROR: u8 = 1;
const ATTESTATION_VALIDATION_ERROR: u8 = 2;

#[derive(RuntimeDebugNoBound)]
pub enum Val<T: frame_system::Config> {
	Fund(T::AccountId),
	NoFund,
}

impl<
		T: ProcessorManagerConfig + Eq + Clone + Send + Sync + 'static,
		P: PairingProvider<T> + Eq + Clone + Send + Sync + 'static,
		OP: OnboardingProvider<T> + Eq + Clone + Send + Sync + 'static,
	> TransactionExtension<T::RuntimeCall> for Onboarding<T, P, OP>
where
	T::RuntimeCall: Dispatchable<Info = DispatchInfo>,
	T::Counter: Default,
	<T::RuntimeCall as Dispatchable>::RuntimeOrigin: AsSystemOriginSigner<T::AccountId> + Clone,
	T::AccountId: IsType<<<T::Proof as Verify>::Signer as IdentifyAccount>::AccountId>,
{
	const IDENTIFIER: &'static str = "Onboarding";

	type Implicit = ();

	type Val = Val<T>;

	type Pre = ();

	fn weight(&self, _call: &T::RuntimeCall) -> Weight {
		// TODO: return actual Weight
		Weight::from_parts(10_000, 0)
	}

	fn validate(
		&self,
		origin: sp_runtime::traits::DispatchOriginOf<T::RuntimeCall>,
		call: &T::RuntimeCall,
		_info: &DispatchInfoOf<T::RuntimeCall>,
		_len: usize,
		_self_implicit: Self::Implicit,
		_inherited_implication: &impl sp_runtime::traits::Implication,
		_source: TransactionSource,
	) -> ValidateResult<Self::Val, T::RuntimeCall> {
		let Some((pairing, is_multi, attestation_chain)) = P::pairing_for_call(call) else {
			return Ok((ValidTransaction::default(), Val::NoFund, origin));
		};

		if !OP::validate_pairing(pairing, is_multi).is_ok() {
			return Err(InvalidTransaction::Custom(PAIRING_VALIDATION_ERROR).into());
		}

		let Some(attestation_chain) = attestation_chain else {
			return Ok((ValidTransaction::default(), Val::NoFund, origin));
		};

		let Ok(who) = ensure_signed(origin.clone()) else {
			return Ok((ValidTransaction::default(), Val::NoFund, origin));
		};

		if !OP::validate_attestation(attestation_chain, &who).is_ok() {
			return Err(InvalidTransaction::Custom(ATTESTATION_VALIDATION_ERROR).into());
		}

		Ok((ValidTransaction::default(), Val::Fund(pairing.account.clone()), origin))
	}

	fn prepare(
		self,
		val: Self::Val,
		_origin: &sp_runtime::traits::DispatchOriginOf<T::RuntimeCall>,
		_call: &T::RuntimeCall,
		_info: &DispatchInfoOf<T::RuntimeCall>,
		_len: usize,
	) -> Result<Self::Pre, TransactionValidityError> {
		let Val::Fund(account) = val else {
			return Ok(());
		};
		OP::fund(&account);
		Ok(())
	}
}
