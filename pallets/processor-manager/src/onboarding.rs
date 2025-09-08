use core::{fmt::Debug, marker::PhantomData};
use frame_support::{
	dispatch::DispatchInfo,
	pallet_prelude::TransactionSource,
	sp_runtime::{
		traits::{
			AsSystemOriginSigner, DispatchInfoOf, DispatchOriginOf, Dispatchable, IdentifyAccount,
			Implication, TransactionExtension, ValidateResult, Verify,
		},
		transaction_validity::{InvalidTransaction, TransactionValidityError, ValidTransaction},
	},
	traits::IsType,
	weights::Weight,
	RuntimeDebugNoBound,
};

use parity_scale_codec::{Decode, DecodeWithMemTracking, Encode};
use scale_info::TypeInfo;

use crate::{Config, ExtensionWeightInfo, OnboardingProvider};

#[derive(Encode, Decode, DecodeWithMemTracking, Clone, Eq, PartialEq, TypeInfo, Default)]
#[scale_info(skip_type_params(T, OP))]
pub struct Onboarding<T: Config, OP: OnboardingProvider<T>> {
	#[codec(skip)]
	_phantom_data: PhantomData<(T, OP)>,
}

impl<T: Config, OP: OnboardingProvider<T>> Onboarding<T, OP> {
	pub fn new() -> Self {
		Self { _phantom_data: Default::default() }
	}
}

impl<T: Config, OP: OnboardingProvider<T>> Debug for Onboarding<T, OP> {
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
const FUNDING_ERROR: u8 = 3;

#[derive(RuntimeDebugNoBound)]
pub enum Val<T: frame_system::Config> {
	Fund(T::AccountId),
	NoFund,
}

impl<
		T: Config + Eq + Clone + Send + Sync + 'static,
		OP: OnboardingProvider<T> + Eq + Clone + Send + Sync + 'static,
	> TransactionExtension<T::RuntimeCall> for Onboarding<T, OP>
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

	fn weight(&self, call: &T::RuntimeCall) -> Weight {
		match OP::pairing_for_call(call) {
			Some((_, _, Some(_))) => T::ExtensionWeightInfo::onboarding(),
			Some((_, _, None)) => T::ExtensionWeightInfo::pairing(),
			_ => Weight::zero(),
		}
	}

	fn validate(
		&self,
		origin: DispatchOriginOf<T::RuntimeCall>,
		call: &T::RuntimeCall,
		_info: &DispatchInfoOf<T::RuntimeCall>,
		_len: usize,
		_self_implicit: Self::Implicit,
		_inherited_implication: &impl Implication,
		_source: TransactionSource,
	) -> ValidateResult<Self::Val, T::RuntimeCall> {
		let Some(who) = origin.as_system_origin_signer() else {
			return Ok((ValidTransaction::default(), Val::NoFund, origin));
		};

		let Some((pairing, is_multi, attestation_chain)) = OP::pairing_for_call(call) else {
			return Ok((ValidTransaction::default(), Val::NoFund, origin));
		};

		if OP::validate_pairing(pairing, is_multi).is_err() {
			#[cfg(not(feature = "runtime-benchmarks"))]
			return Err(InvalidTransaction::Custom(PAIRING_VALIDATION_ERROR).into());
		}

		let Some(attestation_chain) = attestation_chain else {
			return Ok((ValidTransaction::default(), Val::NoFund, origin));
		};

		if OP::validate_attestation(attestation_chain, who).is_err() {
			#[cfg(not(feature = "runtime-benchmarks"))]
			return Err(InvalidTransaction::Custom(ATTESTATION_VALIDATION_ERROR).into());
		}

		if !OP::can_fund_processor_onboarding(who) {
			return Ok((ValidTransaction::default(), Val::NoFund, origin));
		}

		Ok((ValidTransaction::default(), Val::Fund(pairing.account.clone()), origin))
	}

	fn prepare(
		self,
		val: Self::Val,
		_origin: &DispatchOriginOf<T::RuntimeCall>,
		_call: &T::RuntimeCall,
		_info: &DispatchInfoOf<T::RuntimeCall>,
		_len: usize,
	) -> Result<Self::Pre, TransactionValidityError> {
		let Val::Fund(account) = val else {
			return Ok(());
		};
		if OP::fund(&account).is_err() {
			return Err(TransactionValidityError::Invalid(InvalidTransaction::Custom(
				FUNDING_ERROR,
			)));
		}
		Ok(())
	}
}

pub mod extension {
	pub use crate::ExtensionWeightInfo as WeightInfo;

	#[cfg(feature = "runtime-benchmarks")]
	pub mod benchmarking {
		use frame_benchmarking::{account, v2::*, BenchmarkError};
		use frame_support::{
			assert_ok,
			dispatch::{DispatchInfo, PostDispatchInfo},
			pallet_prelude::Zero,
			sp_runtime::traits::{
				AsSystemOriginSigner, AsTransactionAuthorizedOrigin, DispatchTransaction,
				Dispatchable, IdentifyAccount, StaticLookup, Verify,
			},
			traits::{IsSubType, IsType},
			weights::Weight,
		};
		use frame_system::{pallet_prelude::*, RawOrigin};

		use crate::{
			benchmarking::{attestation_chain, processor_pairing, set_timestamp},
			onboarding::Onboarding,
			BalanceFor, BenchmarkHelper, Call, Config, OnboardingSettings,
			Pallet as ProcessorManager,
		};

		pub struct Pallet<T: Config>(ProcessorManager<T>);

		#[benchmarks(where
			T: Send + Sync + pallet_timestamp::Config<Moment = u64>,
			T::RuntimeCall: Dispatchable<Info = DispatchInfo, PostInfo = PostDispatchInfo> + From<Call<T>> + IsSubType<Call<T>>,
			BalanceFor<T>: IsType<u128>,
			T::Counter: Default,
			<T::RuntimeCall as Dispatchable>::RuntimeOrigin: AsSystemOriginSigner<T::AccountId> + AsTransactionAuthorizedOrigin + Clone,
			T::AccountId: IsType<<<T::Proof as Verify>::Signer as IdentifyAccount>::AccountId>,
			<<T as frame_system::Config>::Lookup as StaticLookup>::Source: From<<<T::Proof as Verify>::Signer as IdentifyAccount>::AccountId>,
		)]
		mod benchmarks {
			use super::*;

			#[benchmark]
			fn onboarding() -> Result<(), BenchmarkError> {
				set_timestamp::<T>(1000);
				let len = 0_usize;
				let processor = account::<T::AccountId>("processor", 0, 0);
				let manager = account::<T::AccountId>("manager", 0, 0);
				let info = DispatchInfo { call_weight: Weight::zero(), ..Default::default() };
				let call: T::RuntimeCall = Call::onboard {
					pairing: processor_pairing::<T>(manager),
					multi: false,
					attestation_chain: attestation_chain(),
				}
				.into();
				frame_benchmarking::benchmarking::add_to_whitelist(
					frame_system::BlockHash::<T>::hashed_key_for(BlockNumberFor::<T>::zero())
						.into(),
				);

				let settings = OnboardingSettings::<BalanceFor<T>, T::AccountId> {
					funds: 100_000_000_000u128.into(),
					funds_account: T::BenchmarkHelper::funded_account(0),
				};

				assert_ok!(ProcessorManager::<T>::update_onboarding_settings(
					RawOrigin::<T::AccountId>::Root.into(),
					Some(settings)
				));

				#[block]
				{
					Onboarding::<T, ProcessorManager<T>>::new()
						.test_run(RawOrigin::Signed(processor).into(), &call, &info, len, 0, |_| {
							Ok(().into())
						})
						.unwrap()
						.unwrap();
				}

				Ok(())
			}

			#[benchmark]
			fn pairing() -> Result<(), BenchmarkError> {
				set_timestamp::<T>(1000);
				let len = 0_usize;
				let processor = account::<T::AccountId>("processor", 0, 0);
				let manager = account::<T::AccountId>("manager", 0, 0);
				let info = DispatchInfo { call_weight: Weight::zero(), ..Default::default() };
				let call: T::RuntimeCall =
					Call::pair_with_manager { pairing: processor_pairing::<T>(manager) }.into();
				frame_benchmarking::benchmarking::add_to_whitelist(
					frame_system::BlockHash::<T>::hashed_key_for(BlockNumberFor::<T>::zero())
						.into(),
				);

				let settings = OnboardingSettings::<BalanceFor<T>, T::AccountId> {
					funds: 100_000_000_000u128.into(),
					funds_account: T::BenchmarkHelper::funded_account(0),
				};

				assert_ok!(ProcessorManager::<T>::update_onboarding_settings(
					RawOrigin::<T::AccountId>::Root.into(),
					Some(settings)
				));

				#[block]
				{
					Onboarding::<T, ProcessorManager<T>>::new()
						.test_run(RawOrigin::Signed(processor).into(), &call, &info, len, 0, |_| {
							Ok(().into())
						})
						.unwrap()
						.unwrap();
				}

				Ok(())
			}
		}
	}
}
