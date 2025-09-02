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
use pallet_acurast::OnboardingCounterProvider;
use pallet_acurast_processor_manager::ProcessorPairingFor;
use parity_scale_codec::{Decode, DecodeWithMemTracking, Encode};
use scale_info::TypeInfo;
use sp_runtime::traits::CheckedAdd;
use sp_std::vec;

use crate::utils::{FeePayerProvider, PairingProvider};

#[derive(Encode, Decode, DecodeWithMemTracking, Clone, Eq, PartialEq, TypeInfo)]
#[scale_info(skip_type_params(T, P, C))]
pub struct Onboarding<
	T: pallet_acurast_processor_manager::Config,
	P: PairingProvider<T>,
	C: OnboardingCounterProvider<T::AccountId, T::Counter>,
> {
	#[codec(skip)]
	_phantom_data: PhantomData<(T, P, C)>,
}

impl<
		T: pallet_acurast_processor_manager::Config,
		P: PairingProvider<T>,
		C: OnboardingCounterProvider<T::AccountId, T::Counter>,
	> core::fmt::Debug for Onboarding<T, P, C>
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

#[derive(RuntimeDebugNoBound)]
pub enum Val<T: frame_system::Config> {
	Fund(T::AccountId),
	NoFund,
}

impl<
		T: pallet_acurast_processor_manager::Config + Eq + Clone + Send + Sync + 'static,
		P: PairingProvider<T> + Eq + Clone + Send + Sync + 'static,
		C: OnboardingCounterProvider<T::AccountId, T::Counter> + Eq + Clone + Send + Sync + 'static,
	> TransactionExtension<T::RuntimeCall> for Onboarding<T, P, C>
where
	T::RuntimeCall: Dispatchable<Info = DispatchInfo>,
	T::Counter: Default,
	<T::RuntimeCall as Dispatchable>::RuntimeOrigin: AsSystemOriginSigner<T::AccountId> + Clone,
{
	const IDENTIFIER: &'static str = "Onboarding";

	type Implicit = ();

	type Val = Val<T>;

	type Pre = ();

	fn weight(&self, call: &T::RuntimeCall) -> Weight {
		todo!()
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
		let validity = ValidTransaction {
			priority: 0,
			requires: vec![],
			provides: vec![],
			longevity: TransactionLongevity::max_value(),
			propagate: true,
		};
		let Some((pairing, is_multi)) = P::pairing_for_call(call) else {
			return Ok((validity, Val::NoFund, origin));
		};

		if !is_multi {
			let Some(counter) =
				C::counter(&pairing.account).unwrap_or_default().checked_add(&1u8.into())
			else {
				return Ok((validity, Val::NoFund, origin));
			};
		}

		todo!()
	}

	fn prepare(
		self,
		val: Self::Val,
		origin: &sp_runtime::traits::DispatchOriginOf<T::RuntimeCall>,
		call: &T::RuntimeCall,
		info: &DispatchInfoOf<T::RuntimeCall>,
		len: usize,
	) -> Result<Self::Pre, TransactionValidityError> {
		todo!()
	}
}
