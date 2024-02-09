// Copyright 2019-2022 PureStake Inc.
// Copyright 2023 Papers AG

//! # Migrations

#![allow(deprecated)]

use frame_support::{
    traits::{GetStorageVersion, StorageVersion},
    weights::Weight,
};
use num_traits::Saturating;
use sp_core::Get;
use sp_runtime::traits::Zero;
use sp_std::prelude::*;

use super::*;

pub mod v1 {
    use frame_support::pallet_prelude::*;

    use crate::*;

    #[derive(Encode, Decode, RuntimeDebug, TypeInfo)]
    /// All candidate info except the top and bottom delegations
    pub struct CandidateMetadata<Balance> {
        /// This candidate's self bond amount
        pub bond: Balance,
        /// Total number of delegations to this candidate
        pub delegation_count: u32,
        /// Self bond + sum of top delegations
        pub total_counted: Balance,
        /// The smallest top delegation amount
        pub lowest_top_delegation_amount: Balance,
        /// The highest bottom delegation amount
        pub highest_bottom_delegation_amount: Balance,
        /// The smallest bottom delegation amount
        pub lowest_bottom_delegation_amount: Balance,
        /// Capacity status for top delegations
        pub top_capacity: CapacityStatus,
        /// Capacity status for bottom delegations
        pub bottom_capacity: CapacityStatus,
        /// Maximum 1 pending request to decrease candidate self bond at any given time
        pub request: Option<CandidateBondLessRequest<Balance>>,
        /// Current status of the collator
        pub status: CollatorStatus,
    }
}

pub fn migrate<T: Config>() -> Weight {
    let migrations: [(u16, &dyn Fn() -> Weight); 1] = [(2, &migrate_to_v1::<T>)];

    let onchain_version = Pallet::<T>::on_chain_storage_version();
    let mut weight: Weight = Default::default();
    for (i, f) in migrations.into_iter() {
        if onchain_version < StorageVersion::new(i) {
            weight += f();
        }
    }

    STORAGE_VERSION.put::<Pallet<T>>();
    weight + T::DbWeight::get().writes(1)
}

fn migrate_to_v1<T: Config>() -> Weight {
    // 0) remember the candidates
    // translate just to preserve the bond (we don't get the key in translate_values closure :( )
    CandidateInfo::<T>::translate_values::<v1::CandidateMetadata<BalanceOf<T>>, _>(|info| {
        Some(CandidateMetadata::<BalanceOf<T>> {
            stake: Stake::new(info.bond, 0u32.into()),
            delegation_count: 0,
            total_stake_counted: Zero::zero(),
            lowest_top_delegation: Zero::zero(),
            highest_bottom_delegation: Zero::zero(),
            lowest_bottom_delegation: Zero::zero(),
            top_capacity: CapacityStatus::Full,
            bottom_capacity: CapacityStatus::Full,
            request: None,
            status: Default::default(),
        })
    });
    let mut count = CandidateInfo::<T>::iter_values().count() as u32;

    // retrieve tuples of (account, bond)
    let previous_candidates: Vec<_> = CandidateInfo::<T>::drain()
        .map(|(acc, info)| (acc, info.stake.amount))
        .collect();

    // 1) clear out all structures that changed to StakeOf<T>
    // we know they are reasonably few items and we can clear them within a single migration
    count += DelegatorState::<T>::clear(10_000, None).loops;
    count += CandidateInfo::<T>::clear(10_000, None).loops;
    count += DelegationScheduledRequests::<T>::clear(10_000, None).loops;
    count += AutoCompoundingDelegations::<T>::clear(10_000, None).loops;
    count += TopDelegations::<T>::clear(10_000, None).loops;
    count += BottomDelegations::<T>::clear(10_000, None).loops;
    Total::<T>::kill();
    count += 1;
    CandidatePool::<T>::kill();
    count += 1;
    count += AtStake::<T>::clear(10_000, None).loops;
    count += Staked::<T>::clear(10_000, None).loops;

    // 2) redo some steps of join_candidate extrinsic for each previous candidate
    let mut candidates = <CandidatePool<T>>::get();
    for (acc, amount) in previous_candidates {
        // Commit to 1/64 of MaximumLockingPeriod
        let stake = Stake::new(amount, amount / 64u32.into());
        candidates.insert(Bond {
            owner: acc.clone(),
            stake,
        });
        let candidate = CandidateMetadata::new(stake);
        <CandidateInfo<T>>::insert(&acc, candidate);
        let empty_delegations: Delegations<T::AccountId, BalanceOf<T>> = Default::default();
        // insert empty top delegations
        <TopDelegations<T>>::insert(&acc, empty_delegations.clone());
        // insert empty bottom delegations
        <BottomDelegations<T>>::insert(&acc, empty_delegations);
        let _ = <Total<T>>::mutate(|total| {
            *total = total.saturating_add(stake);
            *total
        });

        count += 4;
    }
    <CandidatePool<T>>::put(candidates);
    count += 1;

    T::DbWeight::get().writes((count + 1).into())
}
