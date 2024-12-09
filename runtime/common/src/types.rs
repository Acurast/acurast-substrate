use core::marker::PhantomData;

use crate::utils::{get_fee_payer, PairingProvider};
use acurast_p256_crypto::MultiSignature;
use frame_support::traits::{
	fungible::{Balanced, Credit, Debt, Inspect, Mutate},
	tokens::{imbalance::OnUnbalanced, Fortitude, Precision, Preservation},
	Imbalance, IsType,
};
use pallet_acurast::{
	utils::ensure_source_verified_and_security_level, AttestationSecurityLevel, CU32,
};
use pallet_acurast_marketplace::RegistrationExtra;
use sp_core::H256;
use sp_runtime::{
	traits::{DispatchInfoOf, IdentifyAccount, PostDispatchInfoOf, Saturating, Verify, Zero},
	transaction_validity::{InvalidTransaction, TransactionValidityError},
	MultiAddress, SaturatedConversion,
};

pub use parachains_common::Balance;
pub use sp_consensus_aura::sr25519::AuthorityId as AuraId;

/// Alias to 512-bit hash when used in the context of a transaction signature on the chain.
pub type Signature = MultiSignature;

/// Some way of identifying an account on the chain. We intentionally make it equivalent
/// to the public key of our transaction signing scheme.
pub type AccountId = <<Signature as Verify>::Signer as IdentifyAccount>::AccountId;

/// Index of a transaction in the chain.
pub type Nonce = u32;

/// A hash of some data used by the chain.
pub type Hash = H256;

/// An index to a block.
pub type BlockNumber = u32;

/// The address format for describing accounts.
pub type Address = MultiAddress<AccountId, ()>;

pub type MaxAllowedSources = pallet_acurast::CU32<1000>;
pub type MaxAllowedSourcesFor<T> = <T as pallet_acurast::Config>::MaxAllowedSources;
pub type MaxSlots = CU32<64>;
pub type MaxSlotsFor<T> = <T as pallet_acurast::Config>::MaxSlots;
pub type ProcessorVersionFor<T> = <T as pallet_acurast::Config>::ProcessorVersion;
pub type MaxVersions = CU32<2>;
pub type MaxVersionsFor<T> = <T as pallet_acurast::Config>::MaxVersions;
pub type MaxEnvVars = CU32<10>;
pub type EnvKeyMaxSize = CU32<32>;
pub type EnvValueMaxSize = CU32<1024>;
pub type ExtraFor<T> = RegistrationExtra<
	Balance,
	AccountId,
	MaxSlotsFor<T>,
	ProcessorVersionFor<T>,
	MaxVersionsFor<T>,
>;

pub struct RewardDistributor<Runtime, Currency>(PhantomData<(Runtime, Currency)>);
impl<Runtime, Currency> pallet_acurast_processor_manager::ProcessorRewardDistributor<Runtime>
	for RewardDistributor<Runtime, Currency>
where
	Currency: Mutate<Runtime::AccountId>,
	<Currency as Inspect<Runtime::AccountId>>::Balance: From<Runtime::Balance>,
	Runtime: pallet_acurast_processor_manager::Config + pallet_acurast::Config,
{
	fn distribute_reward(
		manager: &Runtime::AccountId,
		amount: Runtime::Balance,
		distributor_account: &Runtime::AccountId,
	) -> frame_support::dispatch::DispatchResult {
		Currency::transfer(
			distributor_account,
			&manager,
			amount.saturated_into(),
			Preservation::Preserve,
		)?;
		Ok(())
	}

	fn is_elegible_for_reward(processor: &Runtime::AccountId) -> bool {
		ensure_source_verified_and_security_level::<Runtime>(
			processor,
			&[AttestationSecurityLevel::StrongBox, AttestationSecurityLevel::TrustedEnvironemnt],
		)
		.is_ok()
	}
}

pub struct LiquidityInfo<
	Runtime: pallet_transaction_payment::Config,
	F: Balanced<Runtime::AccountId>,
> {
	pub imbalance: Option<Credit<Runtime::AccountId, F>>,
	pub fee_payer: Option<Runtime::AccountId>,
}

pub struct TransactionCharger<F, OU, P>(PhantomData<(F, OU, P)>);
impl<Runtime, F, OU, P> pallet_transaction_payment::OnChargeTransaction<Runtime>
	for TransactionCharger<F, OU, P>
where
	Runtime: pallet_transaction_payment::Config + pallet_acurast_processor_manager::Config,
	F: Balanced<Runtime::AccountId>,
	OU: OnUnbalanced<Credit<Runtime::AccountId, F>>,
	P: PairingProvider<Runtime>,
	<Runtime as frame_system::Config>::AccountId: IsType<<<<Runtime as pallet_acurast_processor_manager::Config>::Proof as Verify>::Signer as IdentifyAccount>::AccountId>,
{
	type Balance = <F as Inspect<<Runtime as frame_system::Config>::AccountId>>::Balance;
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

		let fee_payer = get_fee_payer::<Runtime, P>(who, call);

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
		if let Some(LiquidityInfo { imbalance, fee_payer }) = info {
			if let Some(paid) = imbalance {
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
			}
		}
		Ok(())
	}
}
