// Copyright 2019-2022 PureStake Inc.
// Copyright 2023 Papers AG

//! traits for parachain-staking

use frame_support::pallet_prelude::Weight;

pub trait OnNewRound {
    fn on_new_round(round_index: crate::RoundIndex) -> Weight;
}
impl OnNewRound for () {
    fn on_new_round(_round_index: crate::RoundIndex) -> Weight {
        Weight::zero()
    }
}
