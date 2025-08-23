use frame_support::pallet_prelude::{DispatchResult, Weight};

use crate::Config;

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
}
