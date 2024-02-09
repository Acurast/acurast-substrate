// Copyright 2019-2022 PureStake Inc.
// Copyright 2023 Papers AG

//! Scheduled requests functionality for delegators

use frame_support::ensure;
use frame_support::pallet_prelude::DispatchResult;
use frame_support::sp_runtime::DispatchError;
use frame_support::traits::Get;
use frame_support::{dispatch::DispatchResultWithPostInfo, pallet_prelude::RuntimeDebug};
use num_traits::Saturating;
use parity_scale_codec::{Decode, Encode};
use scale_info::TypeInfo;
use sp_std::{vec, vec::Vec};

use crate::pallet::{
    BalanceOf, CandidateInfo, Config, DelegationScheduledRequests, DelegatorState, Error, Event,
    Pallet, Round, RoundIndex, Total,
};
use crate::{auto_compound::AutoCompoundDelegations, DelegatorOf, DelegatorStatus, Stake, StakeOf};

/// An action that can be performed upon a delegation
#[derive(Clone, Eq, PartialEq, Encode, Decode, RuntimeDebug, TypeInfo, PartialOrd, Ord)]
pub enum DelegationAction<Balance> {
    Revoke(Stake<Balance>),
    Decrease(Stake<Balance>),
}

impl<Balance: Copy> DelegationAction<Balance> {
    /// Returns the wrapped amount value.
    pub fn amount(&self) -> Stake<Balance> {
        match self {
            DelegationAction::Revoke(amount) => *amount,
            DelegationAction::Decrease(amount) => *amount,
        }
    }
}

/// Represents a scheduled request that define a [DelegationAction]. The request is executable
/// iff the provided [RoundIndex] is achieved.
#[derive(Clone, Eq, PartialEq, Encode, Decode, RuntimeDebug, TypeInfo, PartialOrd, Ord)]
pub struct ScheduledRequest<AccountId, Balance> {
    pub delegator: AccountId,
    pub when_executable: RoundIndex,
    pub action: DelegationAction<Balance>,
}

/// Represents a cancelled scheduled request for emitting an event.
#[derive(Clone, Eq, PartialEq, Encode, Decode, RuntimeDebug, TypeInfo)]
pub struct CancelledScheduledRequest<Balance> {
    pub when_executable: RoundIndex,
    pub action: DelegationAction<Balance>,
}

impl<A, B> From<ScheduledRequest<A, B>> for CancelledScheduledRequest<B> {
    fn from(request: ScheduledRequest<A, B>) -> Self {
        CancelledScheduledRequest {
            when_executable: request.when_executable,
            action: request.action,
        }
    }
}

impl<T: Config> Pallet<T> {
    /// Schedules a [DelegationAction::Revoke] for the delegator, towards a given collator.
    pub(crate) fn delegation_schedule_revoke(
        collator: T::AccountId,
        delegator: T::AccountId,
    ) -> DispatchResultWithPostInfo {
        let mut state = <DelegatorState<T>>::get(&delegator).ok_or(<Error<T>>::DelegatorDNE)?;
        let mut scheduled_requests = <DelegationScheduledRequests<T>>::get(&collator);

        ensure!(
            !scheduled_requests
                .iter()
                .any(|req| req.delegator == delegator),
            <Error<T>>::PendingDelegationRequestAlreadyExists,
        );

        let bonded_amount = state
            .get_bond_amount(&collator)
            .ok_or(<Error<T>>::DelegationDNE)?;
        let now = <Round<T>>::get().current;
        let when = now.saturating_add(T::RevokeDelegationDelay::get());
        scheduled_requests.push(ScheduledRequest {
            delegator: delegator.clone(),
            action: DelegationAction::Revoke(bonded_amount),
            when_executable: when,
        });
        state.less_total = state.less_total.saturating_add(bonded_amount);
        <DelegationScheduledRequests<T>>::insert(collator.clone(), scheduled_requests);
        <DelegatorState<T>>::insert(delegator.clone(), state);

        Self::deposit_event(Event::DelegationRevocationScheduled {
            round: now,
            delegator,
            candidate: collator,
            scheduled_exit: when,
        });
        Ok(().into())
    }

    /// Schedules a [DelegationAction::Decrease] for the delegator, towards a given collator.
    pub(crate) fn delegation_schedule_bond_decrease(
        collator: T::AccountId,
        delegator: T::AccountId,
        decrease_amount: StakeOf<T>,
    ) -> DispatchResultWithPostInfo {
        let mut state = <DelegatorState<T>>::get(&delegator).ok_or(<Error<T>>::DelegatorDNE)?;
        let mut scheduled_requests = <DelegationScheduledRequests<T>>::get(&collator);

        ensure!(
            !scheduled_requests
                .iter()
                .any(|req| req.delegator == delegator),
            <Error<T>>::PendingDelegationRequestAlreadyExists,
        );

        let bonded_amount = state
            .get_bond_amount(&collator)
            .ok_or(<Error<T>>::DelegationDNE)?;
        ensure!(
            bonded_amount > decrease_amount,
            <Error<T>>::DelegatorBondBelowMin
        );
        let new_amount: StakeOf<T> = bonded_amount - decrease_amount;
        ensure!(
            new_amount.amount >= T::MinDelegation::get(),
            <Error<T>>::DelegationBelowMin
        );

        // Net Total is total after pending orders are executed
        let total: StakeOf<T> = state.total().into();
        let less_total: StakeOf<T> = state.less_total.into();
        let net_total = total.saturating_sub(less_total);
        // Net Total is always >= MinDelegatorStk
        let max_subtracted_amount = sp_arithmetic::traits::Saturating::saturating_sub(
            net_total.amount,
            T::MinDelegatorStk::get().into(),
        );
        ensure!(
            decrease_amount.amount <= max_subtracted_amount,
            <Error<T>>::DelegatorBondBelowMin
        );

        let now = <Round<T>>::get().current;
        let when = now.saturating_add(T::RevokeDelegationDelay::get());
        scheduled_requests.push(ScheduledRequest {
            delegator: delegator.clone(),
            action: DelegationAction::Decrease(decrease_amount),
            when_executable: when,
        });
        state.less_total = state.less_total.saturating_add(decrease_amount);
        <DelegationScheduledRequests<T>>::insert(collator.clone(), scheduled_requests);
        <DelegatorState<T>>::insert(delegator.clone(), state);

        Self::deposit_event(Event::DelegationDecreaseScheduled {
            delegator,
            candidate: collator,
            amount_to_decrease: decrease_amount,
            execute_round: when,
        });
        Ok(().into())
    }

    /// Cancels the delegator's existing [ScheduledRequest] towards a given collator.
    pub(crate) fn delegation_cancel_request(
        collator: T::AccountId,
        delegator: T::AccountId,
    ) -> DispatchResultWithPostInfo {
        let mut state = <DelegatorState<T>>::get(&delegator).ok_or(<Error<T>>::DelegatorDNE)?;
        let mut scheduled_requests = <DelegationScheduledRequests<T>>::get(&collator);

        let request =
            Self::cancel_request_with_state(&delegator, &mut state, &mut scheduled_requests)
                .ok_or(<Error<T>>::PendingDelegationRequestDNE)?;

        <DelegationScheduledRequests<T>>::insert(collator.clone(), scheduled_requests);
        <DelegatorState<T>>::insert(delegator.clone(), state);

        Self::deposit_event(Event::CancelledDelegationRequest {
            delegator,
            collator,
            cancelled_request: request.into(),
        });
        Ok(().into())
    }

    fn cancel_request_with_state(
        delegator: &T::AccountId,
        state: &mut DelegatorOf<T>,
        scheduled_requests: &mut Vec<ScheduledRequest<T::AccountId, BalanceOf<T>>>,
    ) -> Option<ScheduledRequest<T::AccountId, BalanceOf<T>>> {
        let request_idx = scheduled_requests
            .iter()
            .position(|req| &req.delegator == delegator)?;

        let request = scheduled_requests.remove(request_idx);
        let amount = request.action.amount();
        state.less_total = state.less_total.saturating_sub(amount);
        Some(request)
    }

    /// Executes the delegator's existing [ScheduledRequest] towards a given collator.
    pub(crate) fn delegation_execute_scheduled_request(
        collator: T::AccountId,
        delegator: T::AccountId,
    ) -> DispatchResultWithPostInfo {
        <DelegationScheduledRequests<T>>::try_mutate(
            &collator,
            |scheduled_requests| -> DispatchResult {
                let request_idx = scheduled_requests
                    .iter()
                    .position(|req| req.delegator == delegator)
                    .ok_or(<Error<T>>::PendingDelegationRequestDNE)?;
                // remove from pending requests
                let request = scheduled_requests.remove(request_idx);

                let now = <Round<T>>::get().current;
                ensure!(
                    request.when_executable <= now,
                    <Error<T>>::PendingDelegationRequestNotDueYet
                );

                let mut state =
                    <DelegatorState<T>>::get(&delegator).ok_or(<Error<T>>::DelegatorDNE)?;
                match request.action {
                    DelegationAction::Revoke(stake) => {
                        Self::revoke_delegator_stake(
                            &mut state,
                            collator.clone(),
                            delegator.clone(),
                            stake,
                        )?;
                        Self::deposit_event(Event::DelegationRevoked {
                            delegator: delegator,
                            candidate: collator.clone(),
                            unstaked_amount: stake,
                        });
                    }
                    DelegationAction::Decrease(stake) => {
                        let in_top = Self::decrease_delegator_stake(
                            &mut state,
                            collator.clone(),
                            delegator.clone(),
                            stake,
                        )?;
                        Self::deposit_event(Event::DelegationDecreased {
                            delegator,
                            candidate: collator.clone(),
                            less: stake,
                            in_top,
                        });
                    }
                };
                Ok(())
            },
        )?;
        Ok(().into())
    }

    pub(crate) fn revoke_delegator_stake(
        state: &mut DelegatorOf<T>,
        collator: T::AccountId,
        delegator: T::AccountId,
        stake: StakeOf<T>,
    ) -> DispatchResult {
        // revoking last delegation => leaving set of delegators
        let leaving = if state.delegations.0.len() == 1usize {
            true
        } else {
            ensure!(
                sp_arithmetic::traits::Saturating::saturating_sub(
                    state.total().amount,
                    T::MinDelegatorStk::get().into()
                ) >= stake.amount,
                <Error<T>>::DelegatorBondBelowMin
            );
            false
        };

        state.less_total = state.less_total.saturating_sub(stake);

        // remove delegation from delegator state
        state.rm_delegation::<T>(&collator);

        // remove delegation from auto-compounding info
        <AutoCompoundDelegations<T>>::remove_auto_compound(&collator, &delegator);

        // remove delegation from collator state delegations
        Self::delegator_leaves_candidate(collator.clone(), delegator.clone(), stake)?;

        if leaving {
            <DelegatorState<T>>::remove(&delegator);
            Self::deposit_event(Event::DelegatorLeft {
                delegator,
                unstaked_amount: stake,
            });
        } else {
            <DelegatorState<T>>::insert(&delegator, state);
        }
        Ok(().into())
    }

    pub(crate) fn decrease_delegator_stake(
        state: &mut DelegatorOf<T>,
        collator: T::AccountId,
        delegator: T::AccountId,
        stake: StakeOf<T>,
    ) -> Result<bool, DispatchError> {
        state.less_total = state.less_total.saturating_sub(stake);

        // decrease delegation
        for bond in &mut state.delegations.0 {
            if bond.owner == collator {
                return if bond.stake > stake {
                    let amount_before: StakeOf<T> = bond.stake.into();
                    bond.stake = bond.stake.saturating_sub(stake);
                    let mut collator_info =
                        <CandidateInfo<T>>::get(&collator).ok_or(<Error<T>>::CandidateDNE)?;

                    state.total_sub_if::<T, _>(stake, |total| {
                        ensure!(
                            total.amount >= T::MinDelegation::get(),
                            <Error<T>>::DelegationBelowMin
                        );
                        ensure!(
                            total.amount >= T::MinDelegatorStk::get(),
                            <Error<T>>::DelegatorBondBelowMin
                        );

                        Ok(())
                    })?;

                    // need to go into decrease_delegation
                    let in_top = collator_info.decrease_delegation::<T>(
                        &collator,
                        delegator.clone(),
                        amount_before,
                        stake,
                    )?;
                    <CandidateInfo<T>>::insert(&collator, collator_info);
                    let new_total_staked = <Total<T>>::get().saturating_sub(stake);
                    <Total<T>>::put(new_total_staked);

                    <DelegatorState<T>>::insert(delegator.clone(), state);
                    Ok(in_top.into())
                } else {
                    // must rm entire delegation if bond.amount <= less or cancel request
                    Err(<Error<T>>::DelegationBelowMin.into())
                };
            }
        }
        Err(<Error<T>>::DelegationDNE.into())
    }

    /// Schedules [DelegationAction::Revoke] for the delegator, towards all delegated collator.
    /// The last fulfilled request causes the delegator to leave the set of delegators.
    pub(crate) fn delegator_schedule_revoke_all(
        delegator: T::AccountId,
    ) -> DispatchResultWithPostInfo {
        let mut state = <DelegatorState<T>>::get(&delegator).ok_or(<Error<T>>::DelegatorDNE)?;
        let mut updated_scheduled_requests = vec![];
        let now = <Round<T>>::get().current;
        let when = now.saturating_add(T::LeaveDelegatorsDelay::get());

        // lazy migration for DelegatorStatus::Leaving
        #[allow(deprecated)]
        if matches!(state.status, DelegatorStatus::Leaving(_)) {
            state.status = DelegatorStatus::Active;
            <DelegatorState<T>>::insert(delegator.clone(), state.clone());
        }

        // it is assumed that a multiple delegations to the same collator does not exist, else this
        // will cause a bug - the last duplicate delegation update will be the only one applied.
        let mut existing_revoke_count = 0;
        for bond in state.delegations.0.clone() {
            let collator = bond.owner;
            let bonded_amount = bond.stake;
            let mut scheduled_requests = <DelegationScheduledRequests<T>>::get(&collator);

            // cancel any existing requests
            let request =
                Self::cancel_request_with_state(&delegator, &mut state, &mut scheduled_requests);
            let request = match request {
                Some(revoke_req) if matches!(revoke_req.action, DelegationAction::Revoke(_)) => {
                    existing_revoke_count += 1;
                    revoke_req // re-insert the same Revoke request
                }
                _ => ScheduledRequest {
                    delegator: delegator.clone(),
                    action: DelegationAction::Revoke(bonded_amount.clone()),
                    when_executable: when,
                },
            };

            scheduled_requests.push(request);
            state.less_total = state.less_total.saturating_add(bonded_amount);
            updated_scheduled_requests.push((collator, scheduled_requests));
        }

        if existing_revoke_count == state.delegations.0.len() {
            return Err(<Error<T>>::DelegatorAlreadyLeaving.into());
        }

        updated_scheduled_requests
            .into_iter()
            .for_each(|(collator, scheduled_requests)| {
                <DelegationScheduledRequests<T>>::insert(collator, scheduled_requests);
            });

        <DelegatorState<T>>::insert(delegator.clone(), state);
        Self::deposit_event(Event::DelegatorExitScheduled {
            round: now,
            delegator,
            scheduled_exit: when,
        });
        Ok(().into())
    }

    /// Cancels every [DelegationAction::Revoke] request for a delegator towards a collator.
    /// Each delegation must have a [DelegationAction::Revoke] scheduled that must be allowed to be
    /// executed in the current round, for this function to succeed.
    pub(crate) fn delegator_cancel_scheduled_revoke_all(
        delegator: T::AccountId,
    ) -> DispatchResultWithPostInfo {
        let mut state = <DelegatorState<T>>::get(&delegator).ok_or(<Error<T>>::DelegatorDNE)?;
        let mut updated_scheduled_requests = vec![];

        // backwards compatible handling for DelegatorStatus::Leaving
        #[allow(deprecated)]
        if matches!(state.status, DelegatorStatus::Leaving(_)) {
            state.status = DelegatorStatus::Active;
            <DelegatorState<T>>::insert(delegator.clone(), state.clone());
            Self::deposit_event(Event::DelegatorExitCancelled { delegator });
            return Ok(().into());
        }

        // pre-validate that all delegations have a Revoke request.
        for bond in &state.delegations.0 {
            let collator = bond.owner.clone();
            let scheduled_requests = <DelegationScheduledRequests<T>>::get(&collator);
            scheduled_requests
                .iter()
                .find(|req| {
                    req.delegator == delegator && matches!(req.action, DelegationAction::Revoke(_))
                })
                .ok_or(<Error<T>>::DelegatorNotLeaving)?;
        }

        // cancel all requests
        for bond in state.delegations.0.clone() {
            let collator = bond.owner.clone();
            let mut scheduled_requests = <DelegationScheduledRequests<T>>::get(&collator);
            Self::cancel_request_with_state(&delegator, &mut state, &mut scheduled_requests);
            updated_scheduled_requests.push((collator, scheduled_requests));
        }

        updated_scheduled_requests
            .into_iter()
            .for_each(|(collator, scheduled_requests)| {
                <DelegationScheduledRequests<T>>::insert(collator, scheduled_requests);
            });

        <DelegatorState<T>>::insert(delegator.clone(), state);
        Self::deposit_event(Event::DelegatorExitCancelled { delegator });

        Ok(().into())
    }

    /// Executes every [DelegationAction::Revoke] request for a delegator towards a collator.
    /// Each delegation must have a [DelegationAction::Revoke] scheduled that must be allowed to be
    /// executed in the current round, for this function to succeed.
    pub(crate) fn delegator_execute_scheduled_revoke_all(
        delegator: T::AccountId,
        delegation_count: u32,
    ) -> DispatchResultWithPostInfo {
        let mut state = <DelegatorState<T>>::get(&delegator).ok_or(<Error<T>>::DelegatorDNE)?;
        ensure!(
            delegation_count >= (state.delegations.0.len() as u32),
            Error::<T>::TooLowDelegationCountToLeaveDelegators
        );
        let now = <Round<T>>::get().current;

        // backwards compatible handling for DelegatorStatus::Leaving
        #[allow(deprecated)]
        if let DelegatorStatus::Leaving(when) = state.status {
            ensure!(
                <Round<T>>::get().current >= when,
                Error::<T>::DelegatorCannotLeaveYet
            );

            for bond in state.delegations.0.clone() {
                if let Err(error) = Self::delegator_leaves_candidate(
                    bond.owner.clone(),
                    delegator.clone(),
                    bond.stake,
                ) {
                    log::warn!(
                        "STORAGE CORRUPTED \nDelegator leaving collator failed with error: {:?}",
                        error
                    );
                }

                Self::delegation_remove_request_with_state(&bond.owner, &delegator, &mut state);
                <AutoCompoundDelegations<T>>::remove_auto_compound(&bond.owner, &delegator);
            }
            <DelegatorState<T>>::remove(&delegator);
            Self::deposit_event(Event::DelegatorLeft {
                delegator,
                unstaked_amount: state.total,
            });
            return Ok(().into());
        }

        let mut validated_scheduled_requests = vec![];
        // pre-validate that all delegations have a Revoke request that can be executed now.
        for bond in &state.delegations.0 {
            let scheduled_requests = <DelegationScheduledRequests<T>>::get(&bond.owner);
            let request_idx = scheduled_requests
                .iter()
                .position(|req| {
                    req.delegator == delegator && matches!(req.action, DelegationAction::Revoke(_))
                })
                .ok_or(<Error<T>>::DelegatorNotLeaving)?;
            let request = &scheduled_requests[request_idx];

            ensure!(
                request.when_executable <= now,
                <Error<T>>::DelegatorCannotLeaveYet
            );

            validated_scheduled_requests.push((bond.clone(), scheduled_requests, request_idx))
        }

        let mut updated_scheduled_requests = vec![];
        // we do not update the delegator state, since the it will be completely removed
        for (bond, mut scheduled_requests, request_idx) in validated_scheduled_requests {
            let collator = bond.owner;

            if let Err(error) =
                Self::delegator_leaves_candidate(collator.clone(), delegator.clone(), bond.stake)
            {
                log::warn!(
                    "STORAGE CORRUPTED \nDelegator {:?} leaving collator failed with error: {:?}",
                    delegator,
                    error
                );
            }

            // remove the scheduled request, since it is fulfilled
            scheduled_requests.remove(request_idx).action.amount();
            updated_scheduled_requests.push((collator.clone(), scheduled_requests));

            // remove the auto-compounding entry for the delegation
            <AutoCompoundDelegations<T>>::remove_auto_compound(&collator, &delegator);
        }

        // set state.total so that state.adjust_bond_lock will remove lock
        let unstaked_amount = state.total();
        state.total_sub::<T>(unstaked_amount)?;

        updated_scheduled_requests
            .into_iter()
            .for_each(|(collator, scheduled_requests)| {
                <DelegationScheduledRequests<T>>::insert(collator, scheduled_requests);
            });

        Self::deposit_event(Event::DelegatorLeft {
            delegator: delegator.clone(),
            unstaked_amount,
        });
        <DelegatorState<T>>::remove(&delegator);

        Ok(().into())
    }

    /// Removes the delegator's existing [ScheduledRequest] towards a given collator, if exists.
    /// The state needs to be persisted by the caller of this function.
    pub(crate) fn delegation_remove_request_with_state(
        collator: &T::AccountId,
        delegator: &T::AccountId,
        state: &mut DelegatorOf<T>,
    ) {
        let mut scheduled_requests = <DelegationScheduledRequests<T>>::get(collator);

        let maybe_request_idx = scheduled_requests
            .iter()
            .position(|req| &req.delegator == delegator);

        if let Some(request_idx) = maybe_request_idx {
            let request = scheduled_requests.remove(request_idx);
            let amount = request.action.amount();
            state.less_total = state.less_total.saturating_sub(amount);
            <DelegationScheduledRequests<T>>::insert(collator, scheduled_requests);
        }
    }

    /// Returns true if a [ScheduledRequest] exists for a given delegation
    pub fn delegation_request_exists(collator: &T::AccountId, delegator: &T::AccountId) -> bool {
        <DelegationScheduledRequests<T>>::get(collator)
            .iter()
            .any(|req| &req.delegator == delegator)
    }

    /// Returns true if a [DelegationAction::Revoke] [ScheduledRequest] exists for a given delegation
    pub fn delegation_request_revoke_exists(
        collator: &T::AccountId,
        delegator: &T::AccountId,
    ) -> bool {
        <DelegationScheduledRequests<T>>::get(collator)
            .iter()
            .any(|req| {
                &req.delegator == delegator && matches!(req.action, DelegationAction::Revoke(_))
            })
    }
}

#[cfg(test)]
mod tests {
    use crate::{mock::Test, set::OrderedSet, Bond, Delegator};

    use super::*;

    #[test]
    fn test_cancel_request_with_state_removes_request_for_correct_delegator_and_updates_state() {
        let mut state = Delegator {
            id: 1,
            delegations: OrderedSet::from(vec![Bond {
                stake: Stake {
                    power: 100,
                    amount: 100,
                },
                owner: 2,
            }]),
            total: Stake {
                power: 100,
                amount: 100,
            },
            less_total: Stake {
                power: 100,
                amount: 100,
            },
            status: crate::DelegatorStatus::Active,
        };
        let mut scheduled_requests = vec![
            ScheduledRequest {
                delegator: 1,
                when_executable: 1,
                action: DelegationAction::Revoke(Stake {
                    power: 100,
                    amount: 100,
                }),
            },
            ScheduledRequest {
                delegator: 2,
                when_executable: 1,
                action: DelegationAction::Decrease(Stake {
                    power: 50,
                    amount: 50,
                }),
            },
        ];
        let removed_request =
            <Pallet<Test>>::cancel_request_with_state(&1, &mut state, &mut scheduled_requests);

        assert_eq!(
            removed_request,
            Some(ScheduledRequest {
                delegator: 1,
                when_executable: 1,
                action: DelegationAction::Revoke(Stake {
                    power: 100,
                    amount: 100,
                }),
            })
        );
        assert_eq!(
            scheduled_requests,
            vec![ScheduledRequest {
                delegator: 2,
                when_executable: 1,
                action: DelegationAction::Decrease(Stake {
                    power: 50,
                    amount: 50,
                }),
            },]
        );
        assert_eq!(
            state,
            Delegator {
                id: 1,
                delegations: OrderedSet::from(vec![Bond {
                    stake: Stake {
                        power: 100,
                        amount: 100,
                    },
                    owner: 2,
                }]),
                total: Stake {
                    power: 100,
                    amount: 100,
                },
                less_total: Stake {
                    power: 0,
                    amount: 0,
                },
                status: crate::DelegatorStatus::Active,
            }
        );
    }

    #[test]
    fn test_cancel_request_with_state_does_nothing_when_request_does_not_exist() {
        let mut state = Delegator {
            id: 1,
            delegations: OrderedSet::from(vec![Bond {
                stake: Stake {
                    power: 100,
                    amount: 200,
                },
                owner: 2,
            }]),
            total: Stake {
                power: 100,
                amount: 200,
            },
            less_total: Stake {
                power: 100,
                amount: 200,
            },
            status: crate::DelegatorStatus::Active,
        };
        let mut scheduled_requests = vec![ScheduledRequest {
            delegator: 2,
            when_executable: 1,
            action: DelegationAction::Decrease(Stake {
                power: 50,
                amount: 100,
            }),
        }];
        let removed_request =
            <Pallet<Test>>::cancel_request_with_state(&1, &mut state, &mut scheduled_requests);

        assert_eq!(removed_request, None,);
        assert_eq!(
            scheduled_requests,
            vec![ScheduledRequest {
                delegator: 2,
                when_executable: 1,
                action: DelegationAction::Decrease(Stake {
                    power: 50,
                    amount: 100,
                }),
            },]
        );
        assert_eq!(
            state,
            Delegator {
                id: 1,
                delegations: OrderedSet::from(vec![Bond {
                    stake: Stake {
                        power: 100,
                        amount: 200,
                    },
                    owner: 2,
                }]),
                total: Stake {
                    power: 100,
                    amount: 200,
                },
                less_total: Stake {
                    power: 100,
                    amount: 200,
                },
                status: crate::DelegatorStatus::Active,
            }
        );
    }
}
