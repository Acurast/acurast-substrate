use acurast_common::AttestationChain;
use frame_support::pallet_prelude::{DispatchResult, Weight};

use crate::{BalanceFor, Config, ProcessorPairingFor};

pub trait ProcessorAssetRecovery<T: Config> {
	fn recover_assets(
		processor: &T::AccountId,
		destination_account: &T::AccountId,
	) -> DispatchResult;
}

pub trait AdvertisementHandler<T: Config> {
	fn advertise_for(processor: &T::AccountId, advertisement: &T::Advertisement) -> DispatchResult;
}

impl<T: Config> AdvertisementHandler<T> for () {
	fn advertise_for(
		_processor: &T::AccountId,
		_advertisement: &T::Advertisement,
	) -> DispatchResult {
		Ok(())
	}
}

pub trait OnboardingProvider<T: Config> {
	fn validate_pairing(pairing: &ProcessorPairingFor<T>, is_multi: bool) -> DispatchResult;
	fn validate_attestation(
		attestation_chain: &AttestationChain,
		account: &T::AccountId,
	) -> DispatchResult;
	fn can_fund_processor_onboarding(
		processor: &T::AccountId,
		manager: &T::AccountId,
	) -> Option<(T::AccountId, BalanceFor<T>)>;
	fn fund(
		from_account: &T::AccountId,
		to_account: &T::AccountId,
		amount: BalanceFor<T>,
	) -> DispatchResult;
	fn can_cover_fee(account: &T::AccountId, fee: BalanceFor<T>) -> (bool, BalanceFor<T>);
	fn release_fee_funds(account: &T::AccountId, fee: BalanceFor<T>);
	fn pairing_for_call(
		call: &T::RuntimeCall,
	) -> Option<(&ProcessorPairingFor<T>, bool, Option<&AttestationChain>)>;
	fn is_funding_call(call: &T::RuntimeCall) -> bool;
	fn fee_payer(account: &T::AccountId, call: &T::RuntimeCall) -> T::AccountId;
}

/// Weight functions needed for pallet_acurast_processor_manager.
pub trait WeightInfo {
	fn update_processor_pairings(x: u32) -> Weight;
	fn pair_with_manager() -> Weight;
	fn multi_pair_with_manager() -> Weight;
	fn recover_funds() -> Weight;
	fn heartbeat() -> Weight;
	fn heartbeat_with_version() -> Weight;
	fn heartbeat_with_metrics(x: u32) -> Weight;
	fn advertise_for() -> Weight;
	fn update_binary_hash() -> Weight;
	fn update_api_version() -> Weight;
	fn set_processor_update_info(x: u32) -> Weight;
	fn update_reward_distribution_settings() -> Weight;
	fn update_min_processor_version_for_reward() -> Weight;
	fn set_management_endpoint() -> Weight;
	fn onboard() -> Weight;
	fn update_onboarding_settings() -> Weight;
	fn set_migration_data() -> Weight;
}

pub trait ExtensionWeightInfo {
	fn pairing() -> Weight;
	fn onboarding() -> Weight;
}
