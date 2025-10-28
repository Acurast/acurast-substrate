use core::marker::PhantomData;

use frame_support::traits::{
	fungible::{Balanced, Credit, Debt, Inspect},
	tokens::{imbalance::OnUnbalanced, Fortitude, Precision, Preservation, WithdrawConsequence},
	Currency, Imbalance, IsType,
};
use frame_system::Config as FrameSystemConfig;
use sp_runtime::{
	traits::{DispatchInfoOf, IdentifyAccount, PostDispatchInfoOf, Verify, Zero},
	transaction_validity::{InvalidTransaction, TransactionValidityError},
	Saturating,
};

use pallet_acurast::IsFundableCall;
use pallet_acurast_processor_manager::{Config as ProcessorManagerConfig, OnboardingProvider};
use pallet_transaction_payment::{Config as TransactionPaymentConfig, OnChargeTransaction};

pub struct LiquidityInfo<Runtime: TransactionPaymentConfig, F: Balanced<Runtime::AccountId>> {
	pub imbalance: Option<Credit<Runtime::AccountId, F>>,
	pub fee_payer: Option<Runtime::AccountId>,
}

pub struct TransactionCharger<F, OU, P, OP>(PhantomData<(F, OU, P, OP)>);
impl<Runtime, F, OU, P, OP> OnChargeTransaction<Runtime>
	for TransactionCharger<F, OU, P, OP>
where
	Runtime: TransactionPaymentConfig + ProcessorManagerConfig,
	F: Balanced<Runtime::AccountId>,
	OU: OnUnbalanced<Credit<Runtime::AccountId, F>>,
	P: IsFundableCall<Runtime::RuntimeCall>,
	OP: OnboardingProvider<Runtime>,
	<Runtime as FrameSystemConfig>::AccountId: IsType<<<<Runtime as ProcessorManagerConfig>::Proof as Verify>::Signer as IdentifyAccount>::AccountId>,
	<F as Inspect<<Runtime as FrameSystemConfig>::AccountId>>::Balance: IsType<<<Runtime as ProcessorManagerConfig>::Currency as Currency<<Runtime as FrameSystemConfig>::AccountId>>::Balance>,
{
	type Balance = <F as Inspect<<Runtime as FrameSystemConfig>::AccountId>>::Balance;
	type LiquidityInfo = Option<LiquidityInfo<Runtime, F>>;

	fn withdraw_fee(
		who: &<Runtime>::AccountId,
		call: &<Runtime>::RuntimeCall,
		_dispatch_info: &DispatchInfoOf<<Runtime>::RuntimeCall>,
		fee: Self::Balance,
		_tip: Self::Balance,
	) -> Result<Self::LiquidityInfo, TransactionValidityError> {
		if fee.is_zero() {
			return Ok(None);
		}

		let fee_payer = OP::fee_payer(who, call);
		if &fee_payer != who && P::is_fundable_call(call) {
			OP::release_fee_funds(&fee_payer, fee.into());
		}

		match F::withdraw(
			&fee_payer,
			fee,
			Precision::Exact,
			Preservation::Preserve,
			Fortitude::Polite,
		) {
			Ok(imbalance) => {
				Ok(Some(LiquidityInfo { imbalance: Some(imbalance), fee_payer: Some(fee_payer) }))
			},
			Err(_) => Err(InvalidTransaction::Payment.into()),
		}
	}

	fn correct_and_deposit_fee(
		who: &<Runtime>::AccountId,
		_dispatch_info: &DispatchInfoOf<<Runtime>::RuntimeCall>,
		_post_info: &PostDispatchInfoOf<<Runtime>::RuntimeCall>,
		corrected_fee: Self::Balance,
		tip: Self::Balance,
		info: Self::LiquidityInfo,
	) -> Result<(), TransactionValidityError> {
		let Some(LiquidityInfo { imbalance, fee_payer }) = info else {
			return Ok(());
		};
		let Some(paid) = imbalance else {
			return Ok(());
		};
		let fee_payer = fee_payer.as_ref().unwrap_or(who);
		// Calculate how much refund we should return
		let refund_amount = paid.peek().saturating_sub(corrected_fee);
		// refund to the the account that paid the fees. If this fails, the
		// account might have dropped below the existential balance. In
		// that case we don't refund anything.
		let refund_imbalance = if F::total_balance(fee_payer) > F::Balance::zero() {
			F::deposit(fee_payer, refund_amount, Precision::BestEffort)
				.unwrap_or_else(|_| Debt::<Runtime::AccountId, F>::zero())
		} else {
			Debt::<Runtime::AccountId, F>::zero()
		};
		// merge the imbalance caused by paying the fees and refunding parts of it again.
		let adjusted_paid = paid
			.offset(refund_imbalance)
			.same()
			.map_err(|_| TransactionValidityError::Invalid(InvalidTransaction::Payment))?;
		// Call someone else to handle the imbalance (fee and tip separately)
		let (tip, fee) = adjusted_paid.split(tip);
		OU::on_unbalanceds(Some(fee).into_iter().chain(Some(tip)));
		Ok(())
	}

	fn can_withdraw_fee(
		who: &<Runtime>::AccountId,
		call: &<Runtime>::RuntimeCall,
		_dispatch_info: &DispatchInfoOf<<Runtime>::RuntimeCall>,
		fee: Self::Balance,
		_tip: Self::Balance,
	) -> Result<(), TransactionValidityError> {
		let fee_payer = OP::fee_payer(who, call);
		if fee.is_zero() || (OP::is_funding_call(call) && OP::can_fund_processor_onboarding(who, &fee_payer).is_some()) {
			return Ok(())
		}

		let mut fee = fee;
		if &fee_payer != who && P::is_fundable_call(call) {
			let (can_cover, missing) = OP::can_cover_fee(&fee_payer, fee.into());
			if can_cover {
				return Ok(());
			}
			fee = missing.into();
		}

		match F::can_withdraw(&fee_payer, fee) {
			WithdrawConsequence::Success => Ok(()),
			_ => Err(InvalidTransaction::Payment.into()),
		}
    }

	#[cfg(feature = "runtime-benchmarks")]
	fn endow_account(who: &<Runtime>::AccountId, amount: Self::Balance) {
        let _ = F::deposit(who, amount, Precision::BestEffort);
    }

	#[cfg(feature = "runtime-benchmarks")]
	fn minimum_balance() -> Self::Balance {
        F::minimum_balance()
    }
}

pub struct IsFundable<T, A, B>(PhantomData<(T, A, B)>);
impl<
		T: frame_system::Config,
		A: IsFundableCall<T::RuntimeCall>,
		B: IsFundableCall<T::RuntimeCall>,
	> IsFundableCall<T::RuntimeCall> for IsFundable<T, A, B>
{
	fn is_fundable_call(call: &T::RuntimeCall) -> bool {
		A::is_fundable_call(call) || B::is_fundable_call(call)
	}
}
