use frame_support::weights::ConstantMultiplier;
use polkadot_runtime_common::SlowAdjustingFeeUpdate;

use acurast_runtime_common::types::{Balance, IsFundable, TransactionCharger};

use crate::{
	AcurastMarketplace, AcurastProcessorManager, Balances, OperationalFeeMultiplier, Runtime,
	RuntimeEvent, TransactionByteFee, WeightToFee,
};

/// Runtime configuration for pallet_transaction_payment.
impl pallet_transaction_payment::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type OnChargeTransaction = TransactionCharger<
		Balances,
		(),
		IsFundable<Self, AcurastProcessorManager, AcurastMarketplace>,
		AcurastProcessorManager,
	>;
	type WeightToFee = WeightToFee;
	type LengthToFee = ConstantMultiplier<Balance, TransactionByteFee>;
	type FeeMultiplierUpdate = SlowAdjustingFeeUpdate<Self>;
	type OperationalFeeMultiplier = OperationalFeeMultiplier;
	type WeightInfo =
		acurast_runtime_common::weight::pallet_transaction_payment::WeightInfo<Runtime>;
}

#[cfg(test)]
mod tests {
	use super::*;
	use acurast_runtime_common::constants::UNIT;
	use frame_support::{
		assert_ok,
		dispatch::{DispatchInfo, PostDispatchInfo},
		pallet_prelude::DispatchResult,
		traits::fungible::{Inspect, InspectHold, Mutate, MutateHold},
	};
	use pallet_acurast::{AttestationChain, IsFundableCall};
	use pallet_acurast_processor_manager::{
		BalanceFor, HoldReason, OnboardingProvider, ProcessorPairingFor,
	};
	use pallet_transaction_payment::OnChargeTransaction;
	use sp_runtime::{
		transaction_validity::{InvalidTransaction, TransactionValidityError},
		AccountId32, BuildStorage,
	};

	use crate::{RuntimeCall, RuntimeHoldReason};

	const MANAGER: AccountId32 = AccountId32::new([1; 32]);
	const PROCESSOR: AccountId32 = AccountId32::new([2; 32]);

	struct Fundable;
	impl IsFundableCall<RuntimeCall> for Fundable {
		fn is_fundable_call(_: &RuntimeCall) -> bool {
			true
		}
		fn is_manager_fundable_call(_: &RuntimeCall) -> bool {
			true
		}
	}

	/// Pays every fee from `MANAGER`; hold handling is the real pallet's.
	struct Onboarding;
	impl OnboardingProvider<Runtime> for Onboarding {
		fn validate_pairing(_: &ProcessorPairingFor<Runtime>, _: bool) -> DispatchResult {
			Ok(())
		}
		fn validate_attestation(
			_: &AttestationChain,
			_: &AccountId32,
			_: &AccountId32,
		) -> DispatchResult {
			Ok(())
		}
		fn can_fund_processor_onboarding(
			_: &AccountId32,
			_: &AccountId32,
		) -> Option<(AccountId32, BalanceFor<Runtime>)> {
			None
		}
		fn fund(_: &AccountId32, _: &AccountId32, _: BalanceFor<Runtime>) -> DispatchResult {
			Ok(())
		}
		fn can_cover_fee(a: &AccountId32, fee: BalanceFor<Runtime>) -> (bool, BalanceFor<Runtime>) {
			AcurastProcessorManager::can_cover_fee(a, fee)
		}
		fn release_fee_funds(a: &AccountId32, fee: BalanceFor<Runtime>) -> BalanceFor<Runtime> {
			<AcurastProcessorManager as OnboardingProvider<Runtime>>::release_fee_funds(a, fee)
		}
		fn hold_fee_funds(a: &AccountId32, amount: BalanceFor<Runtime>) {
			<AcurastProcessorManager as OnboardingProvider<Runtime>>::hold_fee_funds(a, amount)
		}
		fn pairing_for_call(
			_: &RuntimeCall,
		) -> Option<(&ProcessorPairingFor<Runtime>, bool, Option<&AttestationChain>)> {
			None
		}
		fn is_funding_call(_: &RuntimeCall) -> bool {
			false
		}
		fn fee_payer(_: &AccountId32, _: &RuntimeCall) -> AccountId32 {
			MANAGER
		}
	}

	type Charger = TransactionCharger<Balances, (), Fundable, Onboarding>;

	fn reason() -> RuntimeHoldReason {
		HoldReason::Onboarding.into()
	}

	/// Charges `fee`, corrects it to `actual` and returns the manager's (free, held) balance.
	fn charge(held: Balance, fee: Balance, actual: Balance) -> (Balance, Balance) {
		let storage = frame_system::GenesisConfig::<Runtime>::default().build_storage().unwrap();
		sp_io::TestExternalities::new(storage).execute_with(|| {
			Balances::set_balance(&MANAGER, 100 * UNIT);
			Balances::hold(&reason(), &MANAGER, held).unwrap();
			let call: RuntimeCall = frame_system::Call::remark { remark: vec![] }.into();
			let info = DispatchInfo::default();
			let liquidity = <Charger as OnChargeTransaction<Runtime>>::withdraw_fee(
				&PROCESSOR, &call, &info, fee, 0,
			)
			.unwrap();
			<Charger as OnChargeTransaction<Runtime>>::correct_and_deposit_fee(
				&PROCESSOR,
				&info,
				&PostDispatchInfo::default(),
				actual,
				0,
				liquidity,
			)
			.unwrap();
			(Balances::balance(&MANAGER), Balances::balance_on_hold(&reason(), &MANAGER))
		})
	}

	#[test]
	fn refund_of_held_fee_goes_back_on_hold() {
		assert_eq!(charge(10 * UNIT, 4 * UNIT, UNIT), (90 * UNIT, 9 * UNIT));
	}

	#[test]
	fn hold_pays_actual_fee_first() {
		assert_eq!(charge(2 * UNIT, 4 * UNIT, UNIT), (98 * UNIT, UNIT));
	}

	#[test]
	fn refund_of_free_paid_fee_stays_free() {
		assert_eq!(charge(0, 4 * UNIT, UNIT), (99 * UNIT, 0));
	}

	#[test]
	fn sponsored_tip_is_rejected() {
		let storage = frame_system::GenesisConfig::<Runtime>::default().build_storage().unwrap();
		sp_io::TestExternalities::new(storage).execute_with(|| {
			Balances::set_balance(&MANAGER, 100 * UNIT);
			let call: RuntimeCall = frame_system::Call::remark { remark: vec![] }.into();
			let info = DispatchInfo::default();
			let payment = Err(TransactionValidityError::Invalid(InvalidTransaction::Payment));
			assert_eq!(
				<Charger as OnChargeTransaction<Runtime>>::can_withdraw_fee(
					&PROCESSOR,
					&call,
					&info,
					5 * UNIT,
					4 * UNIT
				),
				payment
			);
			assert!(<Charger as OnChargeTransaction<Runtime>>::withdraw_fee(
				&PROCESSOR,
				&call,
				&info,
				5 * UNIT,
				4 * UNIT
			)
			.is_err());
			assert_eq!(Balances::balance(&MANAGER), 100 * UNIT);
			assert_ok!(<Charger as OnChargeTransaction<Runtime>>::can_withdraw_fee(
				&MANAGER,
				&call,
				&info,
				5 * UNIT,
				4 * UNIT
			));
		});
	}
}
