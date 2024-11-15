use core::marker::PhantomData;

use acurast_p256_crypto::MultiSignature;
use frame_support::traits::{fungible, tokens::Preservation};
use pallet_acurast::{
	utils::ensure_source_verified_and_security_level, AttestationSecurityLevel, CU32,
};
use pallet_acurast_marketplace::RegistrationExtra;
use sp_core::H256;
use sp_runtime::{
	traits::{IdentifyAccount, Verify},
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
	Currency: fungible::Mutate<Runtime::AccountId>,
	<Currency as fungible::Inspect<Runtime::AccountId>>::Balance: From<Runtime::Balance>,
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