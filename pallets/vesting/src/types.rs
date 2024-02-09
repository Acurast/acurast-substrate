use frame_support::pallet_prelude::*;
use serde::{Deserialize, Serialize};

use crate::Config;

pub type VestingFor<T, I> = Vesting<<T as Config<I>>::Balance, <T as Config<I>>::BlockNumber>;
pub type VesterStateFor<T, I> =
    VesterState<<T as Config<I>>::Balance, <T as Config<I>>::BlockNumber>;
pub type PoolStateFor<T, I> = PoolState<<T as Config<I>>::Balance>;

#[derive(
    RuntimeDebug,
    Encode,
    Decode,
    MaxEncodedLen,
    TypeInfo,
    Copy,
    Clone,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
)]
pub struct Vesting<Balance, BlockNumber> {
    pub stake: Balance,
    pub locking_period: BlockNumber,
}

#[derive(RuntimeDebug, Encode, Decode, MaxEncodedLen, TypeInfo, Copy, Clone, PartialEq, Eq)]
pub struct VesterState<Balance, BlockNumber> {
    pub locking_period: BlockNumber,
    pub power: Balance,
    pub stake: Balance,
    pub accrued: Balance,
    pub s: Balance,
    pub cooldown_started: Option<BlockNumber>,
}

#[derive(RuntimeDebug, Encode, Decode, MaxEncodedLen, TypeInfo, Clone, PartialEq, Eq, Default)]
pub struct PoolState<Balance> {
    pub total_power: Balance,
    pub total_stake: Balance,
    /// Sum `s = sum_k=0^t [reward_t / power_t]` as a tuple `(upper, lower)` tracking range of possible value of s
    /// that we don't know exactly due to rounding of fixed point numbers.
    pub s: (Balance, Balance),
}

impl<Balance, BlockNumber> From<VesterState<Balance, BlockNumber>>
    for Vesting<Balance, BlockNumber>
{
    fn from(state: VesterState<Balance, BlockNumber>) -> Self {
        Vesting {
            stake: state.stake,
            locking_period: state.locking_period,
        }
    }
}
