// Copyright 2019-2022 PureStake Inc.
// Copyright 2023 Papers AG

//! Helper methods for computing issuance based on inflation
use frame_support::traits::Currency;
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use serde::{Deserialize, Serialize};
use sp_runtime::PerThing;
use sp_runtime::{Perbill, RuntimeDebug};
use substrate_fixed::transcendental::pow as floatpow;
use substrate_fixed::types::I64F64;

use crate::pallet::{BalanceOf, Config, Pallet};

const SECONDS_PER_YEAR: u32 = 31557600;
const SECONDS_PER_BLOCK: u32 = 12; // TODO: Important (This should be in pallet config)
pub const BLOCKS_PER_YEAR: u32 = SECONDS_PER_YEAR / SECONDS_PER_BLOCK;

fn rounds_per_year<T: Config>() -> u32 {
    let blocks_per_round = <Pallet<T>>::round().length;
    BLOCKS_PER_YEAR / blocks_per_round
}

#[derive(
    Eq,
    PartialEq,
    Clone,
    Copy,
    Encode,
    Decode,
    Default,
    RuntimeDebug,
    MaxEncodedLen,
    TypeInfo,
    Serialize,
    Deserialize,
)]
pub struct Range<T> {
    pub min: T,
    pub ideal: T,
}

impl<T: Ord> Range<T> {
    pub fn is_valid(&self) -> bool {
        self.ideal >= self.min
    }
}

impl<T: Ord + Copy> From<T> for Range<T> {
    fn from(other: T) -> Range<T> {
        Range {
            min: other,
            ideal: other,
        }
    }
}
/// Convert an annual inflation to a round inflation
/// round = (1+annual)^(1/rounds_per_year) - 1
pub fn perbill_annual_to_perbill_round(
    annual: Range<Perbill>,
    rounds_per_year: u32,
) -> Range<Perbill> {
    let exponent = I64F64::from_num(1) / I64F64::from_num(rounds_per_year);
    let annual_to_round = |annual: Perbill| -> Perbill {
        let x = I64F64::from_num(annual.deconstruct()) / I64F64::from_num(Perbill::ACCURACY);
        let y: I64F64 = floatpow(I64F64::from_num(1) + x, exponent)
            .expect("Cannot overflow since rounds_per_year is u32 so worst case 0; QED");
        Perbill::from_parts(
            ((y - I64F64::from_num(1)) * I64F64::from_num(Perbill::ACCURACY))
                .ceil()
                .to_num::<u32>(),
        )
    };
    Range {
        min: annual_to_round(annual.min),
        ideal: annual_to_round(annual.ideal),
    }
}
/// Convert annual inflation rate range to round inflation range
pub fn annual_to_round<T: Config>(annual: Range<Perbill>) -> Range<Perbill> {
    let periods = rounds_per_year::<T>();
    perbill_annual_to_perbill_round(annual, periods)
}

/// Compute round issuance from inflation info and current circulating supply
pub fn round_issuance<T: Config>(
    inflation_info: InflationInfo,
    staked: BalanceOf<T>,
) -> BalanceOf<T> {
    let circulating = T::Currency::total_issuance();

    let staked_percentage = Perbill::from_rational(staked, circulating);
    let round = inflation_info.round;

    let inflation_factor = pallet_staking_reward_fn::compute_inflation(
        staked_percentage,
        inflation_info.ideal_staked,
        inflation_info.decay_rate,
    );
    let round_inflation = round.min + (round.ideal - round.min) * inflation_factor;
    round_inflation * circulating
}

#[derive(
    Eq, PartialEq, Clone, Encode, Decode, Default, RuntimeDebug, TypeInfo, Serialize, Deserialize,
)]
pub struct InflationInfo {
    /// Staking expectations
    pub ideal_staked: Perbill,
    /// Annual inflation range
    pub annual: Range<Perbill>,
    /// Round inflation range
    pub round: Range<Perbill>,
    /// A decay rate used so that the inflation rate decreases by at most `decay_rate` when staked
    /// balance is bigger than `ideal_staked`.
    /// This is an disincentive to make sure there is always enough liquidity.
    pub decay_rate: Perbill,
}

#[derive(
    Eq, PartialEq, Clone, Encode, Decode, Default, RuntimeDebug, TypeInfo, Serialize, Deserialize,
)]
pub struct InflationInfoWithoutRound {
    /// Staking expectations
    pub ideal_staked: Perbill,
    /// Annual inflation range
    pub annual: Range<Perbill>,
    /// A decay rate used so that the inflation rate decreases by at most `decay_rate` when staked
    /// balance is bigger than `ideal_staked`.
    /// This is an disincentive to make sure there is always enough liquidity.
    pub decay_rate: Perbill,
}

impl InflationInfo {
    pub fn new<T: Config>(info: InflationInfoWithoutRound) -> InflationInfo {
        InflationInfo {
            ideal_staked: info.ideal_staked,
            annual: info.annual,
            round: annual_to_round::<T>(info.annual),
            decay_rate: info.decay_rate,
        }
    }
    /// Set round inflation range according to input annual inflation range
    pub fn set_round_from_annual<T: Config>(&mut self, new: Range<Perbill>) {
        self.round = annual_to_round::<T>(new);
    }
    /// Reset round inflation rate based on changes to round length
    pub fn reset_round(&mut self, new_length: u32) {
        let periods = BLOCKS_PER_YEAR / new_length;
        self.round = perbill_annual_to_perbill_round(self.annual, periods);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mock_annual_to_round(annual: Range<Perbill>, rounds_per_year: u32) -> Range<Perbill> {
        perbill_annual_to_perbill_round(annual, rounds_per_year)
    }
    fn mock_round_issuance_range(
        // Total circulating before minting
        circulating: u128,
        // Round inflation range
        round: Range<Perbill>,
    ) -> Range<u128> {
        Range {
            min: round.min * circulating,
            ideal: round.ideal * circulating,
        }
    }
    #[test]
    fn simple_issuance_conversion() {
        // 5% inflation for 10_000_0000 = 500,000 minted over the year
        // let's assume there are 10 periods in a year
        // => mint 500_000 over 10 periods => 50_000 minted per period
        let expected_round_issuance_range: Range<u128> = Range {
            min: 48_909,
            ideal: 48_909,
        };
        let schedule = Range {
            min: Perbill::from_percent(5),
            ideal: Perbill::from_percent(5),
        };
        assert_eq!(
            expected_round_issuance_range,
            mock_round_issuance_range(10_000_000, mock_annual_to_round(schedule, 10))
        );
    }
    #[test]
    fn range_issuance_conversion() {
        // 3-5% inflation for 10_000_0000 = 300_000-500,000 minted over the year
        // let's assume there are 10 periods in a year
        // => mint 300_000-500_000 over 10 periods => 30_000-50_000 minted per period
        let expected_round_issuance_range: Range<u128> = Range {
            min: 29_603,
            ideal: 39298,
        };
        let schedule = Range {
            min: Perbill::from_percent(3),
            ideal: Perbill::from_percent(4),
        };
        assert_eq!(
            expected_round_issuance_range,
            mock_round_issuance_range(10_000_000, mock_annual_to_round(schedule, 10))
        );
    }
    #[test]
    fn expected_parameterization() {
        let expected_round_schedule: Range<u128> = Range { min: 45, ideal: 56 };
        let schedule = Range {
            min: Perbill::from_percent(4),
            ideal: Perbill::from_percent(5),
        };
        assert_eq!(
            expected_round_schedule,
            mock_round_issuance_range(10_000_000, mock_annual_to_round(schedule, 8766))
        );
    }
    #[test]
    fn inflation_does_not_panic_at_round_number_limit() {
        let schedule = Range {
            min: Perbill::from_percent(100),
            ideal: Perbill::from_percent(100),
        };
        mock_round_issuance_range(u32::MAX.into(), mock_annual_to_round(schedule, u32::MAX));
        mock_round_issuance_range(u64::MAX.into(), mock_annual_to_round(schedule, u32::MAX));
        mock_round_issuance_range(u128::MAX.into(), mock_annual_to_round(schedule, u32::MAX));
        mock_round_issuance_range(u32::MAX.into(), mock_annual_to_round(schedule, 1));
        mock_round_issuance_range(u64::MAX.into(), mock_annual_to_round(schedule, 1));
        mock_round_issuance_range(u128::MAX.into(), mock_annual_to_round(schedule, 1));
    }
}
