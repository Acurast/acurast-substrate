use core::marker::PhantomData;

use pallet_acurast_compute::RewardContributionProvider;
use pallet_acurast_marketplace::PriceProvider;

use crate::constants::SLOT_DURATION;

use super::Balance;

pub struct ProcessorPriceProvider<Runtime, RCP>(PhantomData<(Runtime, RCP)>);
impl<
		Runtime: frame_system::Config,
		RCP: RewardContributionProvider<<Runtime as frame_system::Config>::AccountId, Balance>,
	> PriceProvider<<Runtime as frame_system::Config>::AccountId, Balance>
	for ProcessorPriceProvider<Runtime, RCP>
{
	fn price_per_millisecond_for(
		processor: &<Runtime as frame_system::Config>::AccountId,
	) -> Option<Balance> {
		let slot_duration = SLOT_DURATION as u128;
		let price_per_block = RCP::reward_contribution_per_block_for(processor)?;
		Some(price_per_block.saturating_div(slot_duration))
	}
}
