#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;
pub use traits::*;
pub use types::*;

mod traits;
mod types;

#[cfg(test)]
pub mod mock;
#[cfg(any(test, feature = "runtime-benchmarks"))]
mod stub;
#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

#[frame_support::pallet]
pub mod pallet {
    use core::ops::Div;

    use codec::MaxEncodedLen;
    use frame_support::sp_runtime::Saturating;
    use frame_support::traits::tokens::Balance;
    use frame_support::{
        dispatch::DispatchResultWithPostInfo, pallet_prelude::*, traits::Get, Parameter,
    };
    use frame_system::{
        ensure_signed,
        pallet_prelude::{BlockNumberFor, OriginFor},
    };
    use sp_arithmetic::traits::EnsureAddAssign;
    use sp_arithmetic::Perbill;
    use sp_runtime::traits::{CheckedAdd, CheckedMul, CheckedSub};
    use sp_std::prelude::*;

    use crate::*;

    /// Configure the pallet by specifying the parameters and types on which it depends.
    #[pallet::config]
    pub trait Config<I: 'static = ()>: frame_system::Config {
        /// Because this pallet emits events, it depends on the runtime's definition of an event.
        type RuntimeEvent: From<Event<Self, I>>
            + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        /// The the tolerance before a vester can be kicked out after his cooldown ended, as a time delta in milliseconds.
        ///
        /// A valid exit call that claims the full reward has to occur within `[cooldown end, now + DivestTolerance]`.
        /// Since the `now` timestmap is behind the current time up to the block time, the actual tolerance is sometimes higher than the configured.
        type DivestTolerance: Get<<Self as Config<I>>::BlockNumber>;
        /// The maximum locking period in number of blocks. Vesting powers are linearly raised with [`Vesting`]`::locking_period / MaximumLockingPeriod`.
        #[pallet::constant]
        type MaximumLockingPeriod: Get<<Self as Config<I>>::BlockNumber>;
        type Balance: Parameter + IsType<u128> + Div + Balance + MaybeSerializeDeserialize;
        #[pallet::constant]
        type BalanceUnit: Get<<Self as Config<I>>::Balance>;
        type BlockNumber: Parameter
            + codec::Codec
            + MaxEncodedLen
            + Ord
            + CheckedAdd
            + Copy
            + Into<u128>
            + IsType<BlockNumberFor<Self>>
            + MaybeSerializeDeserialize;
        type VestingBalance: VestingBalance<Self::AccountId, Self::Balance>;
        /// Weight Info for extrinsics.
        type WeightInfo: WeightInfo;
    }

    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config<I>, I: 'static = ()> {
        pub vesters: Vec<(T::AccountId, VestingFor<T, I>)>,
    }

    impl<T: Config<I>, I: 'static> Default for GenesisConfig<T, I> {
        fn default() -> Self {
            Self {
                vesters: Default::default(),
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config<I>, I: 'static> BuildGenesisConfig for GenesisConfig<T, I> {
        fn build(&self) {
            for (who, vesting) in &self.vesters {
                if let Err(e) = Pallet::<T, I>::vest_for(&who, vesting.to_owned()) {
                    log::error!(
                        target: "runtime::acurast_vesting",
                        "Vesting Genesis error: {:?}",
                        e,
                    );
                }
            }
        }
    }

    #[pallet::storage]
    #[pallet::getter(fn pool)]
    pub(super) type Pool<T: Config<I>, I: 'static = ()> =
        StorageValue<_, PoolStateFor<T, I>, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn vester_states)]
    pub(super) type VesterStates<T: Config<I>, I: 'static = ()> =
        StorageMap<_, Blake2_128Concat, T::AccountId, VesterStateFor<T, I>>;

    #[pallet::pallet]
    pub struct Pallet<T, I = ()>(PhantomData<(T, I)>);

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config<I>, I: 'static = ()> {
        /// A vester started vesting. [vester_state, vesting]
        Vested(T::AccountId, VesterStateFor<T, I>),
        /// A vester revested. [vester, new_vester_state, during_cooldown]
        Revested(T::AccountId, VesterStateFor<T, I>, bool),
        /// A vester started cooldown. [vester, vester_state]
        CooldownStarted(T::AccountId, VesterStateFor<T, I>),
        /// A vester divests after his cooldown ended, claiming accrued rewards. [vester, vester_state_at_divest]
        Divested(T::AccountId, VesterStateFor<T, I>),
        /// A vester that exceeded his divest tolerance got kicked out. [vester, kicker, vester_state_before_kicked_out, reward_cut]
        KickedOut(T::AccountId, T::AccountId, VesterStateFor<T, I>),
        /// A reward got distributed. [amount]
        RewardDistributed(T::Balance),
    }

    // Errors inform users that something went wrong.
    #[pallet::error]
    pub enum Error<T, I = ()> {
        AlreadyVesting,
        MaximumLockingPeriodExceeded,
        NotVesting,
        CannotCooldownDuringCooldown,
        CannotRevestLess,
        CannotRevestWithShorterLockingPeriod,
        CannotDivestBeforeCooldownStarted,
        CannotDivestBeforeCooldownEnds,
        CannotDivestWhenToleranceEnded,
        CannotKickoutBeforeCooldown,
        CannotKickoutBeforeCooldownToleranceEnded,
        CalculationOverflow,
    }

    #[pallet::call]
    impl<T: Config<I>, I: 'static> Pallet<T, I> {
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::vest())]
        pub fn vest(origin: OriginFor<T>, vesting: VestingFor<T, I>) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;

            let vester_state = Self::vest_for(&who, vesting)?;

            Self::deposit_event(Event::<T, I>::Vested(who, vester_state));

            Ok(().into())
        }

        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::revest())]
        pub fn revest(
            origin: OriginFor<T>,
            vesting: VestingFor<T, I>,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;

            let (state_before, state, cooldown_started_before) = Self::revest_for(&who, vesting)?;

            T::VestingBalance::power_increased(
                &who,
                Perbill::from_rational(state_before.power, state.power),
            )?;

            Self::deposit_event(Event::<T, I>::Revested(who, state, cooldown_started_before));

            Ok(().into())
        }

        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::cooldown())]
        pub fn cooldown(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;

            let (_state_before, state) = <VesterStates<T, I>>::try_mutate(
                &who,
                |state| -> Result<(VesterStateFor<T, I>, VesterStateFor<T, I>), DispatchError> {
                    let state = state.as_mut().ok_or(Error::<T, I>::NotVesting)?;
                    let state_before = state.clone();

                    if let Some(_) = state.cooldown_started {
                        Err(Error::<T, I>::CannotCooldownDuringCooldown)?;
                    }

                    Self::accrue(state)?;

                    state.cooldown_started = Some(<frame_system::Pallet<T>>::block_number().into());

                    // punish divest with half the power during cooldown
                    state.power /= 2u128.into();

                    <Pool<T, I>>::try_mutate(|pool| -> Result<(), Error<T, I>> {
                        // due to rounding we need to substract the difference and not the new power!
                        pool.total_power
                            .checked_sub(
                                &state_before
                                    .power
                                    .checked_sub(&state.power)
                                    .ok_or(Error::<T, I>::CalculationOverflow)?,
                            )
                            .ok_or(Error::<T, I>::CalculationOverflow)?;
                        Ok(())
                    })?;

                    Ok((state_before, state.clone()))
                },
            )?;

            // It's more price to define the factor explicitly and not deriving form the state change
            T::VestingBalance::power_decreased(&who, Perbill::from_percent(50))?;

            Self::deposit_event(Event::<T, I>::CooldownStarted(who, state));

            Ok(().into())
        }

        #[pallet::call_index(3)]
        #[pallet::weight(T::WeightInfo::divest())]
        pub fn divest(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;

            let vester_state = <VesterStates<T, I>>::try_mutate(
                &who,
                |state_| -> Result<VesterStateFor<T, I>, DispatchError> {
                    let state = state_.as_mut().ok_or(Error::<T, I>::NotVesting)?;

                    let cooldown_started = state
                        .cooldown_started
                        .ok_or(Error::<T, I>::CannotDivestBeforeCooldownStarted)?;

                    let current_block = <frame_system::Pallet<T>>::block_number();
                    if cooldown_started
                        .checked_add(&state.locking_period)
                        .ok_or(Error::<T, I>::CalculationOverflow)?
                        > current_block.into()
                    {
                        Err(Error::<T, I>::CannotDivestBeforeCooldownEnds)?
                    }

                    if cooldown_started
                        .checked_add(&state.locking_period)
                        .ok_or(Error::<T, I>::CalculationOverflow)?
                        .checked_add(&<T as Config<I>>::DivestTolerance::get().into())
                        .ok_or(Error::<T, I>::CalculationOverflow)?
                        < current_block.into()
                    {
                        Err(Error::<T, I>::CannotDivestWhenToleranceEnded)?
                    }

                    Self::accrue(state)?;
                    let divest_state = *state;

                    *state_ = None;
                    Ok(divest_state)
                },
            )?;

            T::VestingBalance::pay_accrued(&who, vester_state.accrued)?;
            T::VestingBalance::unlock_stake(&who, vester_state.stake)?;

            Self::deposit_event(Event::<T, I>::Divested(who, vester_state));

            Ok(().into())
        }

        #[pallet::call_index(4)]
        #[pallet::weight(T::WeightInfo::kick_out())]
        pub fn kick_out(origin: OriginFor<T>, vester: T::AccountId) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;

            let vester_state = <VesterStates<T, I>>::try_mutate(
                &vester,
                |state_| -> Result<VesterStateFor<T, I>, DispatchError> {
                    let state = state_.as_mut().ok_or(Error::<T, I>::NotVesting)?;

                    let cooldown_started = state
                        .cooldown_started
                        .ok_or(Error::<T, I>::CannotKickoutBeforeCooldown)?;

                    let current_block = <frame_system::Pallet<T>>::block_number();
                    if cooldown_started
                        .checked_add(&state.locking_period)
                        .ok_or(Error::<T, I>::CalculationOverflow)?
                        .checked_add(&<T as Config<I>>::DivestTolerance::get().into())
                        .ok_or(Error::<T, I>::CalculationOverflow)?
                        >= current_block.into()
                    {
                        Err(Error::<T, I>::CannotKickoutBeforeCooldownToleranceEnded)?
                    }

                    Self::accrue(state)?;
                    let before_kicked_out_state = *state;

                    *state_ = None;

                    Ok(before_kicked_out_state)
                },
            )?;

            // give accrued to kicker (or part of it)
            T::VestingBalance::pay_kicker(&who, vester_state.accrued)?;
            T::VestingBalance::unlock_stake(&vester, vester_state.stake)?;

            Self::deposit_event(Event::<T, I>::KickedOut(vester, who, vester_state));

            Ok(().into())
        }
    }

    impl<T: Config<I>, I: 'static> Pallet<T, I> {
        fn vest_for(
            who: &T::AccountId,
            vesting: VestingFor<T, I>,
        ) -> Result<VesterStateFor<T, I>, DispatchError> {
            // update vester state
            let vester_state = <VesterStates<T, I>>::try_mutate(
                &who,
                |state| -> Result<VesterStateFor<T, I>, DispatchError> {
                    if let Some(_) = state {
                        Err(Error::<T, I>::AlreadyVesting)?
                    }

                    if vesting.locking_period > <T as Config<I>>::MaximumLockingPeriod::get() {
                        Err(Error::<T, I>::MaximumLockingPeriodExceeded)?
                    }

                    let power = Self::calculate_power(&vesting)?;

                    let s = VesterStateFor::<T, I> {
                        locking_period: vesting.locking_period,
                        power,
                        stake: vesting.stake,
                        accrued: 0u128.into(),
                        // record global s upper bound at time of vest
                        s: <Pool<T, I>>::get().s.1,
                        cooldown_started: None,
                    };
                    *state = Some(s);

                    // update global state
                    <Pool<T, I>>::try_mutate(|state| -> Result<(), DispatchError> {
                        // total_stake += stake
                        state
                            .total_stake
                            .ensure_add_assign(vesting.stake)
                            .map_err(|_| Error::<T, I>::CalculationOverflow)?;
                        // total_power += power
                        state
                            .total_power
                            .ensure_add_assign(power)
                            .map_err(|_| Error::<T, I>::CalculationOverflow)?;

                        Ok(())
                    })?;

                    Ok(s)
                },
            )?;

            T::VestingBalance::lock_stake(&who, vester_state.stake)?;
            Ok(vester_state.into())
        }

        /// The core logic for revesting, aka increasing the stake. This function is either user-initiaited or
        /// compounding external to this pallet might increase it. Therefore this function **does not** call into hooks
        /// like [`T::VestingBalance::power_increased`] for adapting power but the caller is expected to do so.
        fn revest_for(
            who: &T::AccountId,
            vesting: VestingFor<T, I>,
        ) -> Result<(VesterStateFor<T, I>, VesterStateFor<T, I>, bool), Error<T, I>> {
            T::VestingBalance::adjust_lock(who, vesting.stake);

            <VesterStates<T, I>>::try_mutate(
                &who,
                |state| -> Result<(VesterStateFor<T, I>, VesterStateFor<T, I>, bool), Error<T, I>> {
                    let state = state.as_mut().ok_or(Error::<T, I>::NotVesting)?;
                    let state_before = state.clone();

                    if vesting.stake < state.stake {
                        Err(Error::<T, I>::CannotRevestLess)?
                    }
                    if vesting.locking_period < state.locking_period {
                        Err(Error::<T, I>::CannotRevestWithShorterLockingPeriod)?
                    }
                    if vesting.locking_period > <T as Config<I>>::MaximumLockingPeriod::get() {
                        Err(Error::<T, I>::MaximumLockingPeriodExceeded)?
                    }

                    Self::accrue(state)?;

                    let cooldown_started_before = state.cooldown_started.is_some();

                    // recalculate the power
                    let power_before = state.power;
                    let power = Self::calculate_power(&vesting)?;

                    state.locking_period = vesting.locking_period;
                    state.power = power;
                    state.stake = vesting.stake;
                    // record global s upper bound at time of revest
                    state.s = <Pool<T, I>>::get().s.1;
                    state.cooldown_started = None;

                    <Pool<T, I>>::try_mutate(|pool| -> Result<(), Error<T, I>> {
                        // due to rounding we need to substract the difference and not the new power!
                        pool.total_power.saturating_add(
                            // the new power is always greater than the old power, so check_sub should never fail
                            state
                                .power
                                .checked_sub(&power_before)
                                .ok_or(Error::<T, I>::CalculationOverflow)?,
                        );
                        Ok(())
                    })?;

                    Ok((state_before, state.clone(), cooldown_started_before))
                },
            )
        }

        /// Distributes a reward to the entire pool according to current power distribution.
        ///
        /// Assumes that the reward was already minted and users of this pallet ensure only minted rewards are payed out in [`VestingBalance::pay_accrued`] and [`VestingBalance::pay_kicker`].
        pub fn distribute_reward(reward: T::Balance) -> DispatchResult {
            // s = s + reward / total_power = s + reward * MaximumLockingPeriod / total_power_numerator

            <Pool<T, I>>::try_mutate(|state| -> Result<(), DispatchError> {
                if state.total_power > 0u128.into() {
                    state.s = (
                        state
                            .s
                            .0
                            .checked_add(
                                &(reward * <T as Config<I>>::BalanceUnit::get()
                                    / state.total_power),
                            )
                            .ok_or(Error::<T, I>::CalculationOverflow)?,
                        state
                            .s
                            .1
                            .checked_add(
                                &(reward
                                    // integer division, rounded up
                                    // (we already checked for state.total_power > 0 to avoid DivisionByZero)
                                    .checked_add(&(state.total_power - 1u128.into()))
                                    .ok_or(Error::<T, I>::CalculationOverflow)?
                                    * <T as Config<I>>::BalanceUnit::get()
                                    / state.total_power),
                            )
                            .ok_or(Error::<T, I>::CalculationOverflow)?,
                    );
                }

                Ok(())
            })?;

            Self::deposit_event(Event::<T, I>::RewardDistributed(reward));

            Ok(().into())
        }

        fn accrue(state: &mut VesterStateFor<T, I>) -> Result<(), Error<T, I>> {
            let pool = Self::pool();
            // reward = self.data.power * (self.model.data.s - self.data.s)
            let reward = state
                .power
                .checked_mul(
                    &pool
                        .s
                        // use minimal possible pool.s
                        .0
                        .checked_sub(&state.s)
                        .unwrap_or(0u128.into()),
                )
                .ok_or(Error::<T, I>::CalculationOverflow)?
                / <T as Config<I>>::BalanceUnit::get();
            // accrued += reward
            state
                .accrued
                .ensure_add_assign(reward)
                .map_err(|_| Error::<T, I>::CalculationOverflow)?;
            // memorize maximum possible s
            state.s = pool.s.1;

            Ok(())
        }

        pub fn calculate_power(vesting: &VestingFor<T, I>) -> Result<T::Balance, Error<T, I>> {
            let locking_period: u128 = vesting.locking_period.into();
            let max_locking_period: u128 = <T as Config<I>>::MaximumLockingPeriod::get().into();
            // power = locking_period / MaximumLockingPeriod * stake = locking_period * stake / MaximumLockingPeriod
            Ok((locking_period
                .checked_mul(vesting.stake.into())
                .ok_or(Error::<T, I>::CalculationOverflow)?
                / max_locking_period)
                .into())
        }

        pub fn compound(acc: &T::AccountId, more: T::Balance) -> Result<(), Error<T, I>> {
            let vester_state = Self::vester_states(acc).ok_or(Error::<T, I>::NotVesting)?;
            let new_total_stake = vester_state.stake.saturating_add(more);
            let _ = Self::revest_for(
                acc,
                Vesting {
                    stake: new_total_stake,
                    locking_period: vester_state.locking_period,
                },
            )?;
            Ok(())
        }
    }
}
