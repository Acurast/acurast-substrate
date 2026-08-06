#![allow(clippy::erasing_op)]
// `1 * UNIT` / `0 * UNIT` are kept for column alignment in the expectation tables below.
#![allow(clippy::identity_op)]

pub mod test_actions;

pub use test_actions::{compute_test_flow, events, roll_to_block, setup_balances, Action};

use frame_support::{assert_err, assert_ok};
use sp_core::{bounded_vec, U256};
use sp_runtime::{traits::Zero, AccountId32, FixedU128, Perbill, Perquintill};

use crate::{
	datastructures::{ProvisionalBuffer, SlidingBuffer},
	mock::*,
	stub::*,
	types::*,
	Config, Cycle, Error, Event,
};
use acurast_common::{CommitmentIdProvider, ComputeHooks, ManagerIdProvider, ManagerLookup};

fn commit_actions_2_processors() -> Vec<Action> {
	vec![
		Action::RollToBlock {
			block_number: 10,
			expected_cycle: Cycle { epoch: 0, epoch_start: 2 },
		},
		Action::ProcessorCommit { processor: "A".to_string(), metrics: vec![(1, 1000, 1)] },
		Action::RollToBlock {
			block_number: 20,
			expected_cycle: Cycle { epoch: 0, epoch_start: 2 },
		},
		Action::ProcessorCommit { processor: "B".to_string(), metrics: vec![(1, 1000, 1)] },
		Action::RollToBlock {
			block_number: 150,
			expected_cycle: Cycle { epoch: 1, epoch_start: 102 },
		},
		Action::ProcessorCommit {
			processor: "A".to_string(),
			metrics: vec![(1, 1000, 1), (2, 2000, 1)], // A commits 2000 to pool 2 later used for compute commitment
		},
		Action::ProcessorCommit { processor: "B".to_string(), metrics: vec![(2, 6000, 1)] }, // B commits 6000 to pool 2 later used for compute commitment
		Action::RollToBlock {
			block_number: 202,
			expected_cycle: Cycle { epoch: 2, epoch_start: 202 },
		},
	]
}

fn commit_actions_4_processors() -> Vec<Action> {
	vec![
		Action::RollToBlock {
			block_number: 10,
			expected_cycle: Cycle { epoch: 0, epoch_start: 2 },
		},
		Action::ProcessorCommit { processor: "A".to_string(), metrics: vec![(1, 1000, 1)] },
		Action::ProcessorCommit { processor: "B".to_string(), metrics: vec![(1, 1000, 1)] },
		Action::ProcessorCommit { processor: "E".to_string(), metrics: vec![(1, 1000, 1)] },
		Action::ProcessorCommit { processor: "F".to_string(), metrics: vec![(1, 1000, 1)] },
		Action::RollToBlock {
			block_number: 150,
			expected_cycle: Cycle { epoch: 1, epoch_start: 102 },
		},
		Action::ProcessorCommit {
			processor: "A".to_string(),
			metrics: vec![(1, 1000, 1), (2, 2000, 1)],
		},
		Action::ProcessorCommit { processor: "B".to_string(), metrics: vec![(2, 6000, 1)] },
		Action::ProcessorCommit {
			processor: "E".to_string(),
			metrics: vec![(1, 10_000, 1), (3, 10_000, 1)],
		},
		Action::ProcessorCommit {
			processor: "F".to_string(),
			metrics: vec![(1, 6000, 1), (3, 10_000, 1)],
		},
		Action::RollToBlock {
			block_number: 202,
			expected_cycle: Cycle { epoch: 2, epoch_start: 202 },
		},
	]
}

fn commit_actions_4_processors_from(epoch: u64) -> Vec<Action> {
	vec![
		Action::RollToBlock {
			block_number: epoch * 100 + 2,
			expected_cycle: Cycle { epoch, epoch_start: epoch * 100 + 2 },
		},
		Action::ProcessorCommit {
			processor: "A".to_string(),
			metrics: vec![(1, 1000, 1), (2, 2000, 1)],
		},
		Action::ProcessorCommit { processor: "B".to_string(), metrics: vec![(2, 6000, 1)] },
		Action::ProcessorCommit {
			processor: "E".to_string(),
			metrics: vec![(1, 10_000, 1), (3, 10_000, 1)],
		},
		Action::ProcessorCommit {
			processor: "F".to_string(),
			metrics: vec![(1, 6000, 1), (3, 10_000, 1)],
		},
		Action::RollToBlock {
			block_number: (epoch + 1) * 100 + 2,
			expected_cycle: Cycle { epoch: (epoch + 1), epoch_start: (epoch + 1) * 100 + 2 },
		},
		Action::ProcessorCommit {
			processor: "A".to_string(),
			metrics: vec![(1, 1000, 1), (2, 2000, 1)],
		},
		Action::ProcessorCommit { processor: "B".to_string(), metrics: vec![(2, 6000, 1)] },
		Action::ProcessorCommit {
			processor: "E".to_string(),
			metrics: vec![(1, 10_000, 1), (3, 10_000, 1)],
		},
		Action::ProcessorCommit {
			processor: "F".to_string(),
			metrics: vec![(1, 6000, 1), (3, 10_000, 1)],
		},
		Action::RollToBlock {
			block_number: (epoch + 2) * 100 + 2,
			expected_cycle: Cycle { epoch: (epoch + 2), epoch_start: (epoch + 2) * 100 + 2 },
		},
		Action::ProcessorCommit {
			processor: "A".to_string(),
			metrics: vec![(1, 1000, 1), (2, 2000, 1)],
		},
		Action::ProcessorCommit { processor: "B".to_string(), metrics: vec![(2, 6000, 1)] },
		Action::ProcessorCommit {
			processor: "E".to_string(),
			metrics: vec![(1, 10_000, 1), (3, 10_000, 1)],
		},
		Action::ProcessorCommit {
			processor: "F".to_string(),
			metrics: vec![(1, 6000, 1), (3, 10_000, 1)],
		},
		Action::RollToBlock {
			block_number: (epoch + 3) * 100 + 2,
			expected_cycle: Cycle { epoch: (epoch + 3), epoch_start: (epoch + 3) * 100 + 2 },
		},
	]
}

/// Warmup + epoch-1 commits for three committers on pool 2:
/// committer C is backed by processors A, B; committer D by E, F; committer H by I, J.
/// Ends at block 202 (epoch 2), ready for all of them to `commit_compute` on pool 2.
fn commit_actions_three_committers() -> Vec<Action> {
	vec![
		Action::RollToBlock {
			block_number: 10,
			expected_cycle: Cycle { epoch: 0, epoch_start: 2 },
		},
		Action::ProcessorCommit { processor: "A".to_string(), metrics: vec![(1, 1000, 1)] },
		Action::ProcessorCommit { processor: "E".to_string(), metrics: vec![(1, 1000, 1)] },
		Action::ProcessorCommit { processor: "I".to_string(), metrics: vec![(1, 1000, 1)] },
		Action::RollToBlock {
			block_number: 20,
			expected_cycle: Cycle { epoch: 0, epoch_start: 2 },
		},
		Action::ProcessorCommit { processor: "B".to_string(), metrics: vec![(1, 1000, 1)] },
		Action::ProcessorCommit { processor: "F".to_string(), metrics: vec![(1, 1000, 1)] },
		Action::ProcessorCommit { processor: "J".to_string(), metrics: vec![(1, 1000, 1)] },
		Action::RollToBlock {
			block_number: 150,
			expected_cycle: Cycle { epoch: 1, epoch_start: 102 },
		},
		Action::ProcessorCommit {
			processor: "A".to_string(),
			metrics: vec![(1, 1000, 1), (2, 2000, 1)],
		},
		Action::ProcessorCommit { processor: "B".to_string(), metrics: vec![(2, 6000, 1)] },
		Action::ProcessorCommit {
			processor: "E".to_string(),
			metrics: vec![(1, 1000, 1), (2, 2000, 1)],
		},
		Action::ProcessorCommit { processor: "F".to_string(), metrics: vec![(2, 6000, 1)] },
		Action::ProcessorCommit {
			processor: "I".to_string(),
			metrics: vec![(1, 1000, 1), (2, 2000, 1)],
		},
		Action::ProcessorCommit { processor: "J".to_string(), metrics: vec![(2, 6000, 1)] },
		Action::RollToBlock {
			block_number: 202,
			expected_cycle: Cycle { epoch: 2, epoch_start: 202 },
		},
	]
}

/// Re-commit pool-2 metrics for all six processors at the start of `epoch`, keeping the three
/// commitments' scores fresh (needed so overstake/ratio checks stay valid across epochs).
fn recommit_three_committers(epoch: u64) -> Vec<Action> {
	vec![
		Action::RollToBlock {
			block_number: epoch * 100 + 2,
			expected_cycle: Cycle { epoch, epoch_start: epoch * 100 + 2 },
		},
		Action::ProcessorCommit {
			processor: "A".to_string(),
			metrics: vec![(1, 1000, 1), (2, 2000, 1)],
		},
		Action::ProcessorCommit { processor: "B".to_string(), metrics: vec![(2, 6000, 1)] },
		Action::ProcessorCommit {
			processor: "E".to_string(),
			metrics: vec![(1, 1000, 1), (2, 2000, 1)],
		},
		Action::ProcessorCommit { processor: "F".to_string(), metrics: vec![(2, 6000, 1)] },
		Action::ProcessorCommit {
			processor: "I".to_string(),
			metrics: vec![(1, 1000, 1), (2, 2000, 1)],
		},
		Action::ProcessorCommit { processor: "J".to_string(), metrics: vec![(2, 6000, 1)] },
	]
}

const REDELEGATE_POOLS: &[u8] = &[0, 100];
const REDELEGATE_COMMITTERS: &[(&str, &[&str])] =
	&[("C", &["A", "B"]), ("D", &["E", "F"]), ("H", &["I", "J"])];
/// Commitment ids follow the order of [`REDELEGATE_COMMITTERS`].
const COMMITMENT_C: u128 = 0;
const COMMITMENT_D: u128 = 1;
const COMMITMENT_H: u128 = 2;
const REDELEGATE_STAKE: Balance = 5 * UNIT;
/// Equal to the mock's `MinCooldownPeriod`.
const REDELEGATE_COOLDOWN: u64 = 36;
/// Equal to the mock's `MaxCooldownPeriod`.
const REDELEGATE_COOLDOWN_LONG: u64 = 108;
const REDELEGATE_DELEGATION: Balance = 10 * UNIT;

/// All three committers commit compute on pool 2 (at block 202), then delegator `G` delegates to C.
/// If `also_delegate_to_d`, `G` additionally delegates to D.
fn redelegate_commit_and_delegate(also_delegate_to_d: bool) -> Vec<Action> {
	let mut actions = ["C", "D", "H"]
		.into_iter()
		.map(|committer| Action::CommitCompute {
			committer: committer.to_string(),
			stake: REDELEGATE_STAKE,
			cooldown: REDELEGATE_COOLDOWN,
			metrics: vec![(2, 4000u128 * 4 / 5, 1u128)],
			commission: Perbill::from_percent(0),
		})
		.collect::<Vec<_>>();
	actions.push(Action::Delegate {
		delegator: "G".to_string(),
		committer: "C".to_string(),
		amount: REDELEGATE_DELEGATION,
		cooldown: REDELEGATE_COOLDOWN,
	});
	if also_delegate_to_d {
		actions.push(Action::Delegate {
			delegator: "G".to_string(),
			committer: "D".to_string(),
			amount: REDELEGATE_DELEGATION,
			cooldown: REDELEGATE_COOLDOWN,
		});
	}
	actions
}

/// Full setup rolled past the redelegation blocking period (3 epochs), leaving `G` delegated to C
/// (and optionally D) with fresh scores, ready to redelegate at block 502.
fn redelegate_setup_past_blocking(also_delegate_to_d: bool) -> Vec<Action> {
	let mut actions = commit_actions_three_committers();
	actions.extend(redelegate_commit_and_delegate(also_delegate_to_d));
	actions.extend(recommit_three_committers(3));
	actions.extend(recommit_three_committers(4));
	actions.extend(recommit_three_committers(5));
	actions
}

/// Runs [`redelegate_setup_past_blocking`] and returns `G`'s account.
fn redelegate_flow(also_delegate_to_d: bool) -> AccountId32 {
	compute_test_flow(
		1,
		REDELEGATE_POOLS,
		REDELEGATE_COMMITTERS,
		&redelegate_setup_past_blocking(also_delegate_to_d),
	);
	george_account_id()
}

/// Like [`redelegate_flow`], but with per-committer cooldown periods, so the period `G`'s delegation to
/// C runs on and the period D's committer allows can differ — needed to exercise a redelegation
/// carrying the source's cooldown period over to a target. `delegation_d` additionally delegates to D
/// on the given period.
///
/// Returns `G`'s account.
fn redelegate_flow_with_cooldowns(
	committer_c: u64,
	committer_d: u64,
	delegation_c: u64,
	delegation_d: Option<u64>,
) -> AccountId32 {
	let mut actions = commit_actions_three_committers();
	for (committer, cooldown) in
		[("C", committer_c), ("D", committer_d), ("H", REDELEGATE_COOLDOWN_LONG)]
	{
		actions.push(Action::CommitCompute {
			committer: committer.to_string(),
			stake: REDELEGATE_STAKE,
			cooldown,
			metrics: vec![(2, 4000u128 * 4 / 5, 1u128)],
			commission: Perbill::from_percent(0),
		});
	}
	let mut delegations = vec![("C", delegation_c)];
	if let Some(cooldown) = delegation_d {
		delegations.push(("D", cooldown));
	}
	for (committer, cooldown) in delegations {
		actions.push(Action::Delegate {
			delegator: "G".to_string(),
			committer: committer.to_string(),
			amount: REDELEGATE_DELEGATION,
			cooldown,
		});
	}
	actions.extend(recommit_three_committers(3));
	actions.extend(recommit_three_committers(4));
	actions.extend(recommit_three_committers(5));

	compute_test_flow(1, REDELEGATE_POOLS, REDELEGATE_COMMITTERS, &actions);
	george_account_id()
}

/// `targets` as the bounded list [`Compute::redelegate_v2`] expects.
fn redelegate_targets(targets: Vec<(AccountId32, Balance)>) -> RedelegationTargetsFor<Test, ()> {
	targets.try_into().unwrap()
}

/// A partial redelegation moves part of the delegation to a second committer, keeping the
/// remainder on the first — and conserves the delegator's total and both commitments' aggregates.
#[test]
fn test_redelegate_v2_conserves_totals() {
	ExtBuilder.build().execute_with(|| {
		let moved = 4 * UNIT;
		let mut actions = redelegate_setup_past_blocking(false);
		actions.push(Action::RedelegateV2 {
			delegator: "G".to_string(),
			old_committer: "C".to_string(),
			// The share staying with C is listed explicitly; the targets must add up to the delegation.
			targets: vec![
				("D".to_string(), moved),
				("C".to_string(), REDELEGATE_DELEGATION - moved),
			],
		});

		compute_test_flow(1, REDELEGATE_POOLS, REDELEGATE_COMMITTERS, &actions);

		let g = george_account_id();
		// C keeps the remainder, D receives the moved part.
		assert_eq!(
			Compute::delegations(&g, COMMITMENT_C).unwrap().stake.amount,
			REDELEGATE_DELEGATION - moved
		);
		assert_eq!(Compute::delegations(&g, COMMITMENT_D).unwrap().stake.amount, moved);
		// Nothing minted or lost.
		assert_eq!(Compute::delegator_total(&g), REDELEGATE_DELEGATION);
		assert_eq!(
			Compute::commitments(COMMITMENT_C).unwrap().delegations_total_amount,
			REDELEGATE_DELEGATION - moved
		);
		assert_eq!(Compute::commitments(COMMITMENT_D).unwrap().delegations_total_amount, moved);
	});
}

/// A single call fans a delegation out over several committers, keeping a remainder. Every target
/// stake moved to gets its own `Redelegated` event — the share staying with the source does not — and
/// all aggregates stay consistent.
#[test]
fn test_redelegate_v2_multi_target_conserves_totals() {
	ExtBuilder.build().execute_with(|| {
		let g = redelegate_flow(false);
		let total_stake_before = Compute::total_stake();
		let locked_before = Balances::locks(&g)[0].amount;

		let to_d = 3 * UNIT;
		let to_h = 2 * UNIT;
		let remainder = REDELEGATE_DELEGATION - to_d - to_h;
		assert_ok!(Compute::redelegate_v2(
			RuntimeOrigin::signed(g.clone()),
			charlie_account_id(),
			redelegate_targets(vec![
				(dave_account_id(), to_d),
				(henry_account_id(), to_h),
				(charlie_account_id(), remainder),
			]),
		));

		assert_eq!(Compute::delegations(&g, COMMITMENT_C).unwrap().stake.amount, remainder);
		assert_eq!(Compute::delegations(&g, COMMITMENT_D).unwrap().stake.amount, to_d);
		assert_eq!(Compute::delegations(&g, COMMITMENT_H).unwrap().stake.amount, to_h);
		assert_eq!(Compute::commitments(COMMITMENT_C).unwrap().delegations_total_amount, remainder);
		assert_eq!(Compute::commitments(COMMITMENT_D).unwrap().delegations_total_amount, to_d);
		assert_eq!(Compute::commitments(COMMITMENT_H).unwrap().delegations_total_amount, to_h);
		// Neither the delegator's total, the global total nor the currency lock changed.
		assert_eq!(Compute::delegator_total(&g), REDELEGATE_DELEGATION);
		assert_eq!(Compute::total_stake(), total_stake_before);
		assert_eq!(Balances::locks(&g)[0].amount, locked_before);

		// One `Redelegated` per target stake moved to, none for the remainder staying with C.
		let redelegated = events()
			.into_iter()
			.filter_map(|e| match e {
				RuntimeEvent::Compute(Event::Redelegated(who, old, new)) => Some((who, old, new)),
				_ => None,
			})
			.collect::<Vec<_>>();
		assert_eq!(
			redelegated,
			vec![(g.clone(), COMMITMENT_C, COMMITMENT_D), (g.clone(), COMMITMENT_C, COMMITMENT_H)]
		);
	});
}

/// Moving the whole delegation over several targets leaves nothing behind: the source delegation and
/// its blocking-period lock are both gone.
#[test]
fn test_redelegate_v2_full_move_across_targets() {
	ExtBuilder.build().execute_with(|| {
		let g = redelegate_flow(false);

		let half = REDELEGATE_DELEGATION / 2;
		assert_ok!(Compute::redelegate_v2(
			RuntimeOrigin::signed(g.clone()),
			charlie_account_id(),
			redelegate_targets(vec![(dave_account_id(), half), (henry_account_id(), half)]),
		));

		assert!(
			Compute::delegations(&g, COMMITMENT_C).is_none(),
			"source delegation must be fully removed"
		);
		assert_eq!(Compute::delegations(&g, COMMITMENT_D).unwrap().stake.amount, half);
		assert_eq!(Compute::delegations(&g, COMMITMENT_H).unwrap().stake.amount, half);
		assert_eq!(Compute::delegator_total(&g), REDELEGATE_DELEGATION);
	});
}

/// The deprecated `redelegate` extrinsic still performs a full move.
#[test]
fn test_redelegate_full_moves() {
	ExtBuilder.build().execute_with(|| {
		let g = redelegate_flow(false);

		assert_ok!(Compute::redelegate(
			RuntimeOrigin::signed(g.clone()),
			charlie_account_id(),
			dave_account_id(),
		));

		assert!(
			Compute::delegations(&g, COMMITMENT_C).is_none(),
			"source delegation must be fully removed"
		);
		assert_eq!(
			Compute::delegations(&g, COMMITMENT_D).unwrap().stake.amount,
			REDELEGATE_DELEGATION
		);
	});
}

/// Past the blocking period, invalid amounts are rejected with the right errors while a valid
/// redelegation still succeeds.
#[test]
fn test_redelegate_v2_rejects_bad_amounts() {
	ExtBuilder.build().execute_with(|| {
		let g = redelegate_flow(false);

		// Zero share fails the minimum-delegation check.
		assert_err!(
			Compute::redelegate_v2(
				RuntimeOrigin::signed(g.clone()),
				charlie_account_id(),
				redelegate_targets(vec![
					(dave_account_id(), 0),
					(charlie_account_id(), REDELEGATE_DELEGATION),
				]),
			),
			Error::<Test, ()>::BelowMinDelegation
		);
		// A dust share is rejected too, however it adds up.
		assert_err!(
			Compute::redelegate_v2(
				RuntimeOrigin::signed(g.clone()),
				charlie_account_id(),
				redelegate_targets(vec![
					(dave_account_id(), REDELEGATE_DELEGATION - 1),
					(charlie_account_id(), 1),
				]),
			),
			Error::<Test, ()>::BelowMinDelegation
		);
		// Shares summing to less than the delegation are rejected: the remainder has to be listed.
		assert_err!(
			Compute::redelegate_v2(
				RuntimeOrigin::signed(g.clone()),
				charlie_account_id(),
				redelegate_targets(vec![(dave_account_id(), 4 * UNIT)]),
			),
			Error::<Test, ()>::RedelegationAmountMismatch
		);
		// So are shares summing to more than it holds, in a single target and across targets.
		assert_err!(
			Compute::redelegate_v2(
				RuntimeOrigin::signed(g.clone()),
				charlie_account_id(),
				redelegate_targets(vec![(dave_account_id(), REDELEGATE_DELEGATION + 1)]),
			),
			Error::<Test, ()>::RedelegationAmountMismatch
		);
		assert_err!(
			Compute::redelegate_v2(
				RuntimeOrigin::signed(g.clone()),
				charlie_account_id(),
				redelegate_targets(vec![
					(dave_account_id(), REDELEGATE_DELEGATION),
					(henry_account_id(), 1 * UNIT),
				]),
			),
			Error::<Test, ()>::RedelegationAmountMismatch
		);
		// Moving the whole delegation to a single target is allowed (nothing stays behind).
		assert_ok!(Compute::redelegate_v2(
			RuntimeOrigin::signed(g.clone()),
			charlie_account_id(),
			redelegate_targets(vec![(dave_account_id(), REDELEGATE_DELEGATION)]),
		));
		assert_eq!(
			Compute::delegations(&g, COMMITMENT_D).unwrap().stake.amount,
			REDELEGATE_DELEGATION
		);
	});
}

/// The target list must be non-empty and free of duplicates — including a duplicated source. A repeated
/// target is caught where the second leg is created, since it finds the delegation the first one made.
#[test]
fn test_redelegate_v2_rejects_invalid_targets() {
	ExtBuilder.build().execute_with(|| {
		let g = redelegate_flow(false);

		assert_err!(
			Compute::redelegate_v2(
				RuntimeOrigin::signed(g.clone()),
				charlie_account_id(),
				redelegate_targets(vec![]),
			),
			Error::<Test, ()>::RedelegationAmountMismatch
		);
		assert_err!(
			Compute::redelegate_v2(
				RuntimeOrigin::signed(g.clone()),
				charlie_account_id(),
				redelegate_targets(vec![
					(dave_account_id(), 5 * UNIT),
					(dave_account_id(), 5 * UNIT),
				]),
			),
			Error::<Test, ()>::AlreadyDelegating
		);
		assert_err!(
			Compute::redelegate_v2(
				RuntimeOrigin::signed(g.clone()),
				charlie_account_id(),
				redelegate_targets(vec![
					(charlie_account_id(), 5 * UNIT),
					(charlie_account_id(), 5 * UNIT),
				]),
			),
			Error::<Test, ()>::AlreadyDelegating
		);
		// Both rejections roll back in full: the source delegation is still whole and nothing landed on D.
		assert_eq!(
			Compute::delegations(&g, COMMITMENT_C).unwrap().stake.amount,
			REDELEGATE_DELEGATION
		);
		assert!(Compute::delegations(&g, COMMITMENT_D).is_none());
		assert_eq!(Compute::delegator_total(&g), REDELEGATE_DELEGATION);
	});
}

/// The source committer is a legal target, holding the share that stays with him: listing him alone
/// for the whole amount moves nothing. The delegation is left as it was, except that — like any other
/// target of a redelegation — its blocking period restarts.
#[test]
fn test_redelegate_v2_source_only_target_moves_nothing_but_restarts_lock() {
	ExtBuilder.build().execute_with(|| {
		let g = redelegate_flow(false);
		let before = Compute::delegations(&g, COMMITMENT_C).unwrap();

		assert_ok!(Compute::redelegate_v2(
			RuntimeOrigin::signed(g.clone()),
			charlie_account_id(),
			redelegate_targets(vec![(charlie_account_id(), REDELEGATE_DELEGATION)]),
		));

		let after = Compute::delegations(&g, COMMITMENT_C).unwrap();
		assert_eq!(after.stake.amount, before.stake.amount);
		assert_eq!(after.stake.created, System::block_number());
		assert_eq!(after.stake.cooldown_period, before.stake.cooldown_period);
		assert_eq!(Compute::delegator_total(&g), REDELEGATE_DELEGATION);
		assert_eq!(
			Compute::commitments(COMMITMENT_C).unwrap().delegations_total_amount,
			REDELEGATE_DELEGATION
		);
		// Nothing moved, so no `Redelegated` event was emitted.
		assert!(!events()
			.into_iter()
			.any(|e| matches!(e, RuntimeEvent::Compute(Event::Redelegated(_, _, _)))));
	});
}

/// A redelegation before the blocking period elapses is rejected.
#[test]
fn test_redelegate_v2_blocked_before_period() {
	ExtBuilder.build().execute_with(|| {
		let mut actions = commit_actions_three_committers();
		actions.extend(redelegate_commit_and_delegate(false));

		compute_test_flow(1, REDELEGATE_POOLS, REDELEGATE_COMMITTERS, &actions);

		// No time rolled past `RedelegationBlockingPeriod` since the delegation was created.
		assert_err!(
			Compute::redelegate_v2(
				RuntimeOrigin::signed(george_account_id()),
				charlie_account_id(),
				redelegate_targets(vec![
					(dave_account_id(), 4 * UNIT),
					(charlie_account_id(), REDELEGATE_DELEGATION - 4 * UNIT),
				]),
			),
			Error::<Test, ()>::RedelegateBlocked
		);
	});
}

/// A redelegation never merges: a target the delegator already delegates to is rejected where that
/// target's delegation would be created, and the whole call rolls back — the source delegation ended
/// earlier in the same call is restored. Whether the target's delegation is in cooldown makes no
/// difference, which is what keeps a redelegation from cancelling a cooldown running elsewhere.
#[test]
fn test_redelegate_v2_rejects_target_already_delegated_to() {
	ExtBuilder.build().execute_with(|| {
		let g = redelegate_flow(true);
		let locked_before = Balances::locks(&g)[0].amount;
		let created_c_before = Compute::delegations(&g, COMMITMENT_C).unwrap().stake.created;

		let targets = || {
			redelegate_targets(vec![
				(dave_account_id(), 4 * UNIT),
				(charlie_account_id(), REDELEGATE_DELEGATION - 4 * UNIT),
			])
		};

		assert_err!(
			Compute::redelegate_v2(
				RuntimeOrigin::signed(g.clone()),
				charlie_account_id(),
				targets()
			),
			Error::<Test, ()>::AlreadyDelegating
		);

		// Same rejection once D's delegation is in cooldown: the cooldown survives the attempt.
		assert_ok!(Compute::cooldown_delegation(
			RuntimeOrigin::signed(g.clone()),
			dave_account_id()
		));
		assert_err!(
			Compute::redelegate_v2(
				RuntimeOrigin::signed(g.clone()),
				charlie_account_id(),
				targets()
			),
			Error::<Test, ()>::AlreadyDelegating
		);
		assert!(Compute::delegations(&g, COMMITMENT_D).unwrap().stake.cooldown_started.is_some());

		// Both delegations are untouched, as are the delegator's total and lock.
		assert_eq!(
			Compute::delegations(&g, COMMITMENT_C).unwrap().stake.amount,
			REDELEGATE_DELEGATION
		);
		assert_eq!(
			Compute::delegations(&g, COMMITMENT_D).unwrap().stake.amount,
			REDELEGATE_DELEGATION
		);
		assert_eq!(Compute::delegations(&g, COMMITMENT_C).unwrap().stake.created, created_c_before);
		assert_eq!(Compute::delegator_total(&g), 2 * REDELEGATE_DELEGATION);
		assert_eq!(Balances::locks(&g)[0].amount, locked_before);
	});
}

/// Every target delegation is created with the source's cooldown period, unchanged. Without merges
/// there is no second period to reconcile, so a redelegation can never hand a share a longer period
/// than it already ran on.
#[test]
fn test_redelegate_v2_carries_source_cooldown_to_targets() {
	ExtBuilder.build().execute_with(|| {
		// Both committers allow the long period, but `G`'s delegation to C runs on the short one.
		let g = redelegate_flow_with_cooldowns(
			REDELEGATE_COOLDOWN_LONG,
			REDELEGATE_COOLDOWN_LONG,
			REDELEGATE_COOLDOWN,
			None,
		);

		let moved = 4 * UNIT;
		assert_ok!(Compute::redelegate_v2(
			RuntimeOrigin::signed(g.clone()),
			charlie_account_id(),
			redelegate_targets(vec![
				(dave_account_id(), moved),
				(charlie_account_id(), REDELEGATE_DELEGATION - moved),
			]),
		));

		// The share moved to D keeps the source's short period rather than stretching to what D allows,
		// and so does the remainder staying with C.
		assert_eq!(
			Compute::delegations(&g, COMMITMENT_D).unwrap().stake.cooldown_period,
			REDELEGATE_COOLDOWN
		);
		assert_eq!(
			Compute::delegations(&g, COMMITMENT_C).unwrap().stake.cooldown_period,
			REDELEGATE_COOLDOWN
		);
	});
}

/// The carried-over cooldown period still has to fit the target committer's own cooldown; if it does
/// not, the whole call is rejected — nothing is applied partially.
#[test]
fn test_redelegate_v2_rejected_when_cooldown_exceeds_target_committer() {
	ExtBuilder.build().execute_with(|| {
		// C allows the long period and `G` delegates to C on it, but D only allows the short one.
		let g = redelegate_flow_with_cooldowns(
			REDELEGATE_COOLDOWN_LONG,
			REDELEGATE_COOLDOWN,
			REDELEGATE_COOLDOWN_LONG,
			None,
		);

		assert_err!(
			Compute::redelegate_v2(
				RuntimeOrigin::signed(g.clone()),
				charlie_account_id(),
				redelegate_targets(vec![
					(dave_account_id(), 4 * UNIT),
					(charlie_account_id(), REDELEGATE_DELEGATION - 4 * UNIT),
				]),
			),
			Error::<Test, ()>::DelegationCooldownMustBeShorterThanCommitment
		);
		// Nothing changed.
		assert_eq!(
			Compute::delegations(&g, COMMITMENT_C).unwrap().stake.amount,
			REDELEGATE_DELEGATION
		);
		assert!(Compute::delegations(&g, COMMITMENT_D).is_none());
	});
}

/// Only a full move is allowed while the source delegation is in cooldown: re-creating the share that
/// stays would silently cancel the delegator's cooldown.
#[test]
fn test_redelegate_v2_partial_move_rejected_while_source_in_cooldown() {
	ExtBuilder.build().execute_with(|| {
		let g = redelegate_flow(false);

		assert_ok!(Compute::cooldown_delegation(
			RuntimeOrigin::signed(g.clone()),
			charlie_account_id()
		));

		assert_err!(
			Compute::redelegate_v2(
				RuntimeOrigin::signed(g.clone()),
				charlie_account_id(),
				redelegate_targets(vec![
					(dave_account_id(), 4 * UNIT),
					(charlie_account_id(), REDELEGATE_DELEGATION - 4 * UNIT),
				]),
			),
			Error::<Test, ()>::DelegationInCooldown
		);
		// A full move is still possible.
		assert_ok!(Compute::redelegate_v2(
			RuntimeOrigin::signed(g.clone()),
			charlie_account_id(),
			redelegate_targets(vec![(dave_account_id(), REDELEGATE_DELEGATION)]),
		));
	});
}

/// A partial move restarts the blocking period on every resulting delegation, the remainder left on
/// the source included: one redelegation blocks the delegation as a whole, so a delegator cannot keep
/// moving stake away from the source in slices while the shares that arrived elsewhere are held.
#[test]
fn test_redelegate_v2_remainder_restarts_lock() {
	ExtBuilder.build().execute_with(|| {
		let g = redelegate_flow(false);
		let created_before = Compute::delegations(&g, COMMITMENT_C).unwrap().stake.created;

		assert_ok!(Compute::redelegate_v2(
			RuntimeOrigin::signed(g.clone()),
			charlie_account_id(),
			redelegate_targets(vec![
				(dave_account_id(), 3 * UNIT),
				(charlie_account_id(), REDELEGATE_DELEGATION - 3 * UNIT),
			]),
		));

		let now = System::block_number();
		assert!(now > created_before, "the flow must have advanced past the original `created`");
		assert_eq!(Compute::delegations(&g, COMMITMENT_D).unwrap().stake.created, now);
		assert_eq!(
			Compute::delegations(&g, COMMITMENT_C).unwrap().stake.created,
			now,
			"the remainder left on the source must have its blocking period restarted too"
		);

		// Moving another slice away from the source is therefore blocked, ...
		assert_err!(
			Compute::redelegate_v2(
				RuntimeOrigin::signed(g.clone()),
				charlie_account_id(),
				redelegate_targets(vec![
					(henry_account_id(), 3 * UNIT),
					(charlie_account_id(), REDELEGATE_DELEGATION - 6 * UNIT),
				]),
			),
			Error::<Test, ()>::RedelegateBlocked
		);
		// ... just as stake that just arrived at D cannot leave again.
		assert_err!(
			Compute::redelegate_v2(
				RuntimeOrigin::signed(g.clone()),
				dave_account_id(),
				redelegate_targets(vec![(henry_account_id(), 3 * UNIT)]),
			),
			Error::<Test, ()>::RedelegateBlocked
		);
	});
}

/// Growing a delegation neither restarts the blocking period nor is blocked by a running one: the
/// added stake could as well have been delegated elsewhere.
#[test]
fn test_delegate_more_keeps_redelegation_lock() {
	ExtBuilder.build().execute_with(|| {
		let g = redelegate_flow(false);
		let created_before = Compute::delegations(&g, COMMITMENT_C).unwrap().stake.created;

		assert_ok!(Compute::delegate_more(
			RuntimeOrigin::signed(g.clone()),
			charlie_account_id(),
			1 * UNIT,
			None,
			None,
		));

		assert_eq!(
			Compute::delegations(&g, COMMITMENT_C).unwrap().stake.created,
			created_before,
			"growing a delegation must not restart the blocking period"
		);
		// Redelegating the grown delegation is therefore still possible.
		assert_ok!(Compute::redelegate_v2(
			RuntimeOrigin::signed(g.clone()),
			charlie_account_id(),
			redelegate_targets(vec![(dave_account_id(), REDELEGATE_DELEGATION + 1 * UNIT)]),
		));

		// Nor is `delegate_more` itself blocked by a running period: the stake that just arrived at D
		// restarted it, and it can still be grown.
		assert_ok!(Compute::delegate_more(
			RuntimeOrigin::signed(g.clone()),
			dave_account_id(),
			1 * UNIT,
			None,
			None,
		));
	});
}

/// Auto-compounding is triggerable by a third party, so it must not be able to extend a delegator's
/// blocking period.
#[test]
fn test_compound_delegation_does_not_restart_redelegation_lock() {
	ExtBuilder.build().execute_with(|| {
		let g = redelegate_flow(false);
		let created_before = Compute::delegations(&g, COMMITMENT_C).unwrap().stake.created;

		// Settle the accrued reward first: compounding several epochs worth of it would grow the
		// delegation past `MaxDelegationRatio` relative to the committer's own stake, which has nothing
		// to do with the blocking period under test here.
		assert_ok!(Compute::withdraw_delegation(
			RuntimeOrigin::signed(g.clone()),
			charlie_account_id(),
		));

		// `Action::Delegate` sets `allow_auto_compound`, so anyone may compound for `G`.
		assert_ok!(Compute::compound_delegation(
			RuntimeOrigin::signed(alice_account_id()),
			charlie_account_id(),
			Some(g.clone()),
		));

		assert_eq!(
			Compute::delegations(&g, COMMITMENT_C).unwrap().stake.created,
			created_before,
			"compounding must not restart the blocking period"
		);
		// Redelegating is therefore still possible.
		let amount = Compute::delegations(&g, COMMITMENT_C).unwrap().stake.amount;
		assert_ok!(Compute::redelegate_v2(
			RuntimeOrigin::signed(g.clone()),
			charlie_account_id(),
			redelegate_targets(vec![(dave_account_id(), amount)]),
		));
	});
}

#[test]
fn test_compute_flow_no_delegations_no_rewards() {
	ExtBuilder.build().execute_with(|| {
		compute_test_flow(
			2,
			&[30, 50, 20],
			&[
				("C", &["A", "B"]), // committer C with processors A, B
			],
			&[
				&commit_actions_2_processors()[..],
				&[
					Action::CommitCompute {
						committer: "C".to_string(),
						stake: 5 * UNIT,
						cooldown: 108, // 1/3 of max
						metrics: vec![(2, 8000u128 * 4 / 5, 1u128)],
						commission: Perbill::from_percent(10),
					}, // Maximal possible commitment value: 80% of average for pool 2
					Action::CooldownComputeCommitment { committer: "C".to_string() },
					Action::RollToBlock {
						block_number: 410, // Advance past cooldown period (started at 302, +108 blocks)
						expected_cycle: Cycle { epoch: 4, epoch_start: 402 },
					},
					Action::EndComputeCommitment { committer: "C".to_string(), expected_reward: 0 },
				][..],
			]
			.concat(),
		);
	});
}

#[test]
fn test_compute_flow_no_delegations() {
	ExtBuilder.build().execute_with(|| {
		compute_test_flow(
			2,
			&[30, 50, 20],
			&[
				("C", &["A", "B"]), // committer C with processors A, B
			],
			&[
				&commit_actions_2_processors()[..],
				&[
					// commit in epoch 2
					Action::CommitCompute {
						committer: "C".to_string(),
						stake: 5 * UNIT,
						cooldown: 108, // max
						metrics: vec![(2, 8000u128 * 4 / 5, 1u128)],
						commission: Perbill::from_percent(10),
					}, // Maximal possible commitment value: 80% of average for pool 2
					// commit but not yet score since only committed in same epoch
					Action::ProcessorCommit {
						processor: "A".to_string(),
						metrics: vec![(1, 1000, 1), (2, 2000, 1)],
					},
					Action::ProcessorCommit {
						processor: "B".to_string(),
						metrics: vec![(2, 6000, 1)],
					},
					Action::RollToBlock {
						block_number: 302,
						expected_cycle: Cycle { epoch: 3, epoch_start: 302 },
					},
					// heartbeat that scores for epoch 3
					Action::ProcessorCommit {
						processor: "A".to_string(),
						metrics: vec![(1, 1000, 1), (2, 2000, 1)],
					},
					Action::ProcessorCommit {
						processor: "B".to_string(),
						metrics: vec![(2, 6000, 1)],
					},
					Action::RollToBlock {
						block_number: 402,
						expected_cycle: Cycle { epoch: 4, epoch_start: 402 },
					},
					// heartbeat that distributes for epoch 3 (in epoch 4)
					Action::ProcessorCommit {
						processor: "A".to_string(),
						metrics: vec![(1, 1000, 1), (2, 2000, 1)],
					},
					Action::ProcessorCommit {
						processor: "B".to_string(),
						metrics: vec![(2, 6000, 1)],
					},
					Action::CooldownComputeCommitment { committer: "C".to_string() },
					Action::RollToBlock {
						block_number: 510, // Advance past cooldown period (started at 402, +108 blocks)
						expected_cycle: Cycle { epoch: 5, epoch_start: 502 },
					},
					Action::EndComputeCommitment {
						committer: "C".to_string(),
						expected_reward: 50 * UNIT,
					},
				][..],
			]
			.concat(),
		);
	});
}

#[test]
fn test_compute_flow_delegate_to_self() {
	ExtBuilder.build().execute_with(|| {
		compute_test_flow(
			2,
			&[0, 100],
			&[
				("C", &["A", "B"]), // committer C with processors A, B
			],
			&[
				&commit_actions_2_processors()[..],
				&[
					Action::CommitCompute {
						committer: "C".to_string(),
						stake: 5 * UNIT,
						cooldown: 36, // 1/3 of max
						metrics: vec![(2, 4000u128 * 4 / 5, 1u128)],
						commission: Perbill::from_percent(0),
					}, // Maximal possible commitment value: 80% of average for pool 2
					Action::Delegate {
						delegator: "C".to_string(),
						committer: "C".to_string(),
						amount: 5 * UNIT,
						cooldown: 36,
					},
					// commit but not yet score since only committed in same epoch
					Action::ProcessorCommit {
						processor: "A".to_string(),
						metrics: vec![(1, 1000, 1), (2, 2000, 1)],
					},
					Action::ProcessorCommit {
						processor: "B".to_string(),
						metrics: vec![(2, 6000, 1)],
					},
					Action::RollToBlock {
						block_number: 302,
						expected_cycle: Cycle { epoch: 3, epoch_start: 302 },
					},
					// heartbeat that scores for epoch 3
					Action::ProcessorCommit {
						processor: "A".to_string(),
						metrics: vec![(1, 1000, 1), (2, 2000, 1)],
					},
					Action::ProcessorCommit {
						processor: "B".to_string(),
						metrics: vec![(2, 6000, 1)],
					},
					Action::RollToBlock {
						block_number: 402,
						expected_cycle: Cycle { epoch: 4, epoch_start: 402 },
					},
					// heartbeat that distributes for epoch 3 (in epoch 4)
					Action::ProcessorCommit {
						processor: "A".to_string(),
						metrics: vec![(1, 1000, 1), (2, 2000, 1)],
					},
					Action::ProcessorCommit {
						processor: "B".to_string(),
						metrics: vec![(2, 6000, 1)],
					},
					Action::CooldownComputeCommitment { committer: "C".to_string() },
					Action::RollToBlock {
						block_number: 510, // Advance past cooldown period (started at 402, +108 blocks)
						expected_cycle: Cycle { epoch: 5, epoch_start: 502 },
					},
					Action::EndComputeCommitment {
						committer: "C".to_string(),
						expected_reward: 50 * UNIT,
					},
					Action::EndDelegation {
						delegator: "C".to_string(),
						committer: "C".to_string(),
						expected_reward: 50 * UNIT,
					},
				][..],
			]
			.concat(),
		);
	});
}

#[test]
fn test_compute_flow_delegate_to_self_after_reward() {
	ExtBuilder.build().execute_with(|| {
		compute_test_flow(
			2,
			&[0, 100],
			&[
				("C", &["A", "B"]), // committer C with processors A, B
			],
			&[
				&commit_actions_2_processors()[..],
				&[
					Action::CommitCompute {
						committer: "C".to_string(),
						stake: 5 * UNIT,
						cooldown: 36, // 1/3 of max
						metrics: vec![(2, 4000u128 * 4 / 5, 1u128)],
						commission: Perbill::from_percent(0),
					}, // Maximal possible commitment value: 80% of average for pool 2
					// commit but not yet score since only committed in same epoch
					Action::ProcessorCommit {
						processor: "A".to_string(),
						metrics: vec![(1, 1000, 1), (2, 2000, 1)],
					},
					Action::ProcessorCommit {
						processor: "B".to_string(),
						metrics: vec![(2, 6000, 1)],
					},
					Action::RollToBlock {
						block_number: 302,
						expected_cycle: Cycle { epoch: 3, epoch_start: 302 },
					},
					// heartbeat that scores for epoch 3
					Action::ProcessorCommit {
						processor: "A".to_string(),
						metrics: vec![(1, 1000, 1), (2, 2000, 1)],
					},
					Action::ProcessorCommit {
						processor: "B".to_string(),
						metrics: vec![(2, 6000, 1)],
					},
					Action::RollToBlock {
						block_number: 402,
						expected_cycle: Cycle { epoch: 4, epoch_start: 402 },
					},
					// delegate in epoch 4 but should not influence distribution for epoch 3
					Action::Delegate {
						delegator: "C".to_string(),
						committer: "C".to_string(),
						amount: 5 * UNIT,
						cooldown: 36,
					},
					// heartbeat that distributes for epoch 3 (in epoch 4)
					Action::ProcessorCommit {
						processor: "A".to_string(),
						metrics: vec![(1, 1000, 1), (2, 2000, 1)],
					},
					Action::ProcessorCommit {
						processor: "B".to_string(),
						metrics: vec![(2, 6000, 1)],
					},
					Action::CooldownComputeCommitment { committer: "C".to_string() },
					Action::RollToBlock {
						block_number: 510, // Advance past cooldown period (started at 402, +108 blocks)
						expected_cycle: Cycle { epoch: 5, epoch_start: 502 },
					},
					Action::EndComputeCommitment {
						committer: "C".to_string(),
						expected_reward: 100 * UNIT,
					},
					Action::EndDelegation {
						delegator: "C".to_string(),
						committer: "C".to_string(),
						expected_reward: 0 * UNIT,
					},
				][..],
			]
			.concat(),
		);
	});
}

#[test]
fn test_compute_flow_2_committers() {
	ExtBuilder.build().execute_with(|| {
		compute_test_flow(
			2,
			&[25, 25, 50],
			&[
				("C", &["A", "B"]), // committer C with processors A, B
				("D", &["E", "F"]), // committer D with processors E, F
			],
			&[
				// (1, 1000, 1),  (2, 2000, 1)
				//                (2, 6000, 1)
				// (1, 10_000, 1)             (3, 10_000, 1)
				// (1, 6000, 1)               (3, 10_000, 1)
				&commit_actions_4_processors()[..],
				&[
					Action::CommitCompute {
						committer: "C".to_string(),
						stake: 5 * UNIT,
						cooldown: 36, // 1/3 of max
						metrics: vec![(2, 8000u128 * 4 / 5, 1u128)],
						commission: Perbill::from_percent(10),
					}, // Maximal possible commitment value: 80% of average for pool 2
					Action::CommitCompute {
						committer: "D".to_string(),
						stake: 5 * UNIT,
						cooldown: 36, // 1/3 of max
						metrics: vec![
							(1, 16_000u128 * 4 / 5, 1u128),
							(3, 20_000u128 * 4 / 5, 1u128),
						],
						commission: Perbill::from_percent(10),
					}, // Maximal possible commitment value: 80% of average for pool 2
				][..],
				&commit_actions_4_processors_from(2)[..],
				&[
					Action::CooldownComputeCommitment { committer: "C".to_string() },
					Action::CooldownComputeCommitment { committer: "D".to_string() },
					Action::RollToBlock {
						block_number: 538, // Advance past cooldown period (started at 502, +36 blocks)
						expected_cycle: Cycle { epoch: 5, epoch_start: 502 },
					},
					Action::EndComputeCommitment {
						committer: "C".to_string(),
						expected_reward: 25 * UNIT,
					},
					Action::EndComputeCommitment {
						committer: "D".to_string(),
						expected_reward: 75 * UNIT,
					},
				][..],
			]
			.concat(),
		);
	});
}

// FIX FROM Here
// #[test]
// fn test_compute_flow_2_committers_subsequential_overlapping_pools() {
// 	ExtBuilder.build().execute_with(|| {
// 		compute_test_flow(
// 			2,
// 			&[25, 25, 50],
// 			&[
// 				("C", &["A", "B"]), // committer C with processors A, B
// 				("D", &["E", "F"]), // committer D with processors E, F
// 			],
// 			&[
// 				&commit_actions_4_processors()[..],
// 				&[
// 					Action::CommitCompute {
// 						committer: "C".to_string(),
// 						stake: 5 * UNIT,
// 						cooldown: 36, // 1/3 of max
// 						metrics: vec![(1, 1000u128 * 4 / 5, 1u128), (2, 4000u128 * 4 / 5, 1u128)],
// 						commission: Perbill::from_percent(10),
// 					}, // Maximal possible commitment value: 80% of average for pool 2
// 					// Action::Reward { amount: 10 * UNIT },
// 					Action::CooldownComputeCommitment { committer: "C".to_string() },
// 					Action::RollToBlock {
// 						block_number: 700, // Advance past cooldown period (started at 602, +36 blocks + buffer)
// 						expected_cycle: Cycle { epoch: 6, epoch_start: 602 },
// 					},
// 					Action::EndComputeCommitment {
// 						committer: "C".to_string(),
// 						expected_reward: 5 * UNIT,
// 					},
// 					Action::CommitCompute {
// 						committer: "D".to_string(),
// 						stake: 5 * UNIT,
// 						cooldown: 36, // 1/3 of max
// 						metrics: vec![(1, 8000u128 * 4 / 5, 1u128), (3, 10_000u128 * 4 / 5, 1u128)],
// 						commission: Perbill::from_percent(10),
// 					}, // Maximal possible commitment value: 80% of average for pool 2
// 					// Action::Reward { amount: 10 * UNIT },
// 					Action::CooldownComputeCommitment { committer: "D".to_string() },
// 					Action::RollToBlock {
// 						block_number: 737, // Advance past cooldown period (started at 700, +36 blocks + buffer)
// 						expected_cycle: Cycle { epoch: 7, epoch_start: 702 },
// 					},
// 					Action::EndComputeCommitment {
// 						committer: "D".to_string(),
// 						expected_reward: 7500 * MILLIUNIT,
// 					},
// 				][..],
// 			]
// 			.concat(),
// 		);
// 	});
// }

// #[test]
// fn test_compute_flow_2_committers_one_withdraws() {
// 	ExtBuilder.build().execute_with(|| {
// 		compute_test_flow(
// 			2,
// 			&[25, 25, 50],
// 			&[
// 				("C", &["A", "B"]), // committer C with processors A, B
// 				("D", &["E", "F"]), // committer D with processors E, F
// 			],
// 			&[
// 				&commit_actions_4_processors()[..],
// 				&[
// 					Action::CommitCompute {
// 						committer: "C".to_string(),
// 						stake: 5 * UNIT,
// 						cooldown: 36, // 1/3 of max
// 						metrics: vec![(2, 4000u128 * 4 / 5, 1u128)],
// 						commission: Perbill::from_percent(10),
// 					}, // Maximal possible commitment value: 80% of average for pool 2
// 					Action::CommitCompute {
// 						committer: "D".to_string(),
// 						stake: 5 * UNIT,
// 						cooldown: 36, // 1/3 of max
// 						metrics: vec![(1, 8000u128 * 4 / 5, 1u128), (3, 10_000u128 * 4 / 5, 1u128)],
// 						commission: Perbill::from_percent(10),
// 					}, // Maximal possible commitment value: 80% of average for pool 2
// 					// Action::Reward { amount: 10 * UNIT },
// 					Action::WithdrawCommitment {
// 						committer: "C".to_string(),
// 						expected_reward: 2500 * MILLIUNIT,
// 					},
// 					Action::CooldownComputeCommitment { committer: "C".to_string() },
// 					Action::CooldownComputeCommitment { committer: "D".to_string() },
// 					Action::RollToBlock {
// 						block_number: 700, // Advance past cooldown period (started at 602, +36 blocks + buffer)
// 						expected_cycle: Cycle { epoch: 6, epoch_start: 602 },
// 					},
// 					Action::EndComputeCommitment { committer: "C".to_string(), expected_reward: 0 },
// 					Action::EndComputeCommitment {
// 						committer: "D".to_string(),
// 						expected_reward: 7500 * MILLIUNIT,
// 					},
// 				][..],
// 			]
// 			.concat(),
// 		);
// 	});
// }

// #[test]
// fn test_compute_flow_2_shifted_committers_competing_metric_pools() {
// 	ExtBuilder.build().execute_with(|| {
// 		compute_test_flow(
// 			2,
// 			&[25, 25, 50],
// 			&[
// 				("C", &["A", "B"]), // committer C with processors A, B
// 				("D", &["E", "F"]), // committer D with processors E, F
// 			],
// 			&[
// 				&commit_actions_4_processors()[..],
// 				&[
// 					Action::CommitCompute {
// 						committer: "C".to_string(),
// 						stake: 5 * UNIT,
// 						cooldown: 36, // 1/3 of max
// 						metrics: vec![(1, 1000u128 * 4 / 5, 1u128), (2, 4000u128 * 4 / 5, 1u128)],
// 						commission: Perbill::from_percent(10),
// 					}, // Maximal possible commitment value: 80% of average for pool 2
// 					// Action::Reward { amount: 10 * UNIT },
// 					Action::CooldownComputeCommitment { committer: "C".to_string() },
// 					Action::CommitCompute {
// 						committer: "D".to_string(),
// 						stake: 5 * UNIT,
// 						cooldown: 36, // 1/3 of max
// 						metrics: vec![(1, 3000u128 * 4 / 5, 1u128), (3, 10_000u128 * 4 / 5, 1u128)],
// 						commission: Perbill::from_percent(10),
// 					}, // Maximal possible commitment value: 80% of average for pool 2
// 					// Action::Reward { amount: 10 * UNIT },
// 					Action::CooldownComputeCommitment { committer: "D".to_string() },
// 					Action::RollToBlock {
// 						block_number: 700, // Advance past cooldown period (started at 602, +36 blocks + buffer)
// 						expected_cycle: Cycle { epoch: 6, epoch_start: 602 },
// 					},
// 					Action::EndComputeCommitment {
// 						committer: "C".to_string(),
// 						expected_reward: 7_692_307_692_303,
// 					},
// 					Action::EndComputeCommitment {
// 						committer: "D".to_string(),
// 						expected_reward: 7_307_692_307_689,
// 					},
// 					// total is 15, 5 got not distributed because nobody was in pool 3 rewarded with 50% at the time of first reward!
// 				][..],
// 			]
// 			.concat(),
// 		);
// 	});
// }

// #[test]
// fn test_compute_flow_2_shifted_committers_competing_metric_pools_with_delegations() {
// 	ExtBuilder.build().execute_with(|| {
// 		compute_test_flow(
// 			2,
// 			&[25, 25, 50],
// 			&[
// 				("C", &["A", "B"]), // committer C with processors A, B
// 				("D", &["E", "F"]), // committer D with processors E, F
// 			],
// 			&[
// 				&commit_actions_4_processors()[..],
// 				&[
// 					Action::CommitCompute {
// 						committer: "C".to_string(),
// 						stake: 5 * UNIT,
// 						cooldown: 36, // 1/3 of max
// 						metrics: vec![(1, 1000u128 * 4 / 5, 1u128), (2, 4000u128 * 4 / 5, 1u128)],
// 						commission: Perbill::from_percent(10),
// 					}, // Maximal possible commitment value: 80% of average for pool 2
// 					Action::Delegate {
// 						delegator: "G".to_string(),
// 						committer: "C".to_string(),
// 						amount: 40 * UNIT,
// 						cooldown: 36,
// 					},
// 					// Action::Reward { amount: 10 * UNIT },
// 					Action::CooldownComputeCommitment { committer: "C".to_string() },
// 					Action::CommitCompute {
// 						committer: "D".to_string(),
// 						stake: 5 * UNIT,
// 						cooldown: 36, // 1/3 of max
// 						metrics: vec![(1, 3000u128 * 4 / 5, 1u128), (3, 10_000u128 * 4 / 5, 1u128)],
// 						commission: Perbill::from_percent(10),
// 					}, // Maximal possible commitment value: 80% of average for pool 2
// 					Action::Delegate {
// 						delegator: "H".to_string(),
// 						committer: "D".to_string(),
// 						amount: 5 * UNIT,
// 						cooldown: 36,
// 					},
// 					// Action::Reward { amount: 10 * UNIT },
// 					Action::CooldownComputeCommitment { committer: "D".to_string() },
// 					Action::RollToBlock {
// 						block_number: 700, // Advance past cooldown period (started at 602, +36 blocks + buffer)
// 						expected_cycle: Cycle { epoch: 6, epoch_start: 602 },
// 					},
// 					Action::EndComputeCommitment {
// 						committer: "C".to_string(),
// 						expected_reward: 763_589_988_839,
// 					},
// 					Action::EndComputeCommitment {
// 						committer: "D".to_string(),
// 						expected_reward: 3_231_707_317_071,
// 					},
// 					Action::EndDelegation {
// 						delegator: "G".to_string(),
// 						committer: "C".to_string(),
// 						expected_reward: 7772995377009,
// 					},
// 					Action::EndDelegation {
// 						delegator: "H".to_string(),
// 						committer: "D".to_string(),
// 						expected_reward: 3_231_707_317_071,
// 					},
// 					// total is
// 					// 763_589_988_839+3_231_707_317_071+7_772_995_377_008+3_231_707_317_071
// 					// ~= 15, 5 got not distributed because nobody was in pool 3 rewarded with 50% at the time of first reward!
// 				][..],
// 			]
// 			.concat(),
// 		);
// 	});
// }

// #[test]
// fn test_compute_flow_2_committers_competing_metric_pools() {
// 	ExtBuilder.build().execute_with(|| {
// 		compute_test_flow(
// 			2,
// 			&[25, 25, 50],
// 			&[
// 				("C", &["A", "B"]), // committer C with processors A, B
// 				("D", &["E", "F"]), // committer D with processors E, F
// 			],
// 			&[
// 				&commit_actions_4_processors()[..],
// 				&[
// 					Action::CommitCompute {
// 						committer: "C".to_string(),
// 						stake: 5 * UNIT,
// 						cooldown: 36, // 1/3 of max
// 						metrics: vec![(1, 1000u128 * 4 / 5, 1u128), (2, 4000u128 * 4 / 5, 1u128)],
// 						commission: Perbill::from_percent(10),
// 					}, // Maximal possible commitment value: 80% of average for pool 2
// 					Action::CommitCompute {
// 						committer: "D".to_string(),
// 						stake: 5 * UNIT,
// 						cooldown: 36, // 1/3 of max
// 						metrics: vec![(1, 3000u128 * 4 / 5, 1u128), (3, 10_000u128 * 4 / 5, 1u128)],
// 						commission: Perbill::from_percent(10),
// 					}, // Maximal possible commitment value: 80% of average for pool 2
// 					// Action::Reward { amount: 10 * UNIT },
// 					Action::CooldownComputeCommitment { committer: "C".to_string() },
// 					Action::CooldownComputeCommitment { committer: "D".to_string() },
// 					Action::RollToBlock {
// 						block_number: 700, // Advance past cooldown period (started at 602, +36 blocks + buffer)
// 						expected_cycle: Cycle { epoch: 6, epoch_start: 602 },
// 					},
// 					Action::EndComputeCommitment {
// 						committer: "C".to_string(),
// 						expected_reward: 2857142857140,
// 					},
// 					Action::EndComputeCommitment {
// 						committer: "D".to_string(),
// 						expected_reward: 10 * UNIT - 2857142857140,
// 					},
// 				][..],
// 			]
// 			.concat(),
// 		);
// 	});
// }

// #[test]
// fn test_compute_flow_4_processors_only_one_commits() {
// 	ExtBuilder.build().execute_with(|| {
// 		compute_test_flow(
// 			2,
// 			&[50, 50],
// 			&[
// 				("C", &["A", "B"]), // committer C with processors A, B
// 				("D", &["E", "F"]), // committer D with processors E, F
// 			],
// 			&[
// 				&commit_actions_4_processors()[..],
// 				&[
// 					Action::CommitCompute {
// 						committer: "C".to_string(),
// 						stake: 5 * UNIT,
// 						cooldown: 36, // 1/3 of max
// 						metrics: vec![(1, 1000u128 * 4 / 5, 1u128), (2, 4000u128 * 4 / 5, 1u128)], // commits for both pools that are rewarded to total 100%
// 						commission: Perbill::from_percent(10),
// 					}, // Maximal possible commitment value: 80% of average for pool 2
// 					// Action::Reward { amount: 10 * UNIT },
// 					Action::CooldownComputeCommitment { committer: "C".to_string() },
// 					Action::RollToBlock {
// 						block_number: 700, // Advance past cooldown period (started at 602, +36 blocks + buffer)
// 						expected_cycle: Cycle { epoch: 6, epoch_start: 602 },
// 					},
// 					Action::EndComputeCommitment {
// 						committer: "C".to_string(),
// 						expected_reward: 10 * UNIT,
// 					},
// 				][..],
// 			]
// 			.concat(),
// 		);
// 	});
// }

// #[test]
// fn test_compute_flow_rewarded_metrics_pool_without_committers() {
// 	ExtBuilder.build().execute_with(|| {
// 		compute_test_flow(
// 			2,
// 			&[50, 50],
// 			&[
// 				("C", &["A", "B"]), // committer C with processors A, B
// 			],
// 			&[
// 				&commit_actions_4_processors()[..],
// 				&[
// 					Action::CommitCompute {
// 						committer: "C".to_string(),
// 						stake: 5 * UNIT,
// 						cooldown: 36,                                // 1/3 of max
// 						metrics: vec![(2, 4000u128 * 4 / 5, 1u128)], // commits for both pools that are rewarded to total 100%
// 						commission: Perbill::from_percent(10),
// 					}, // Maximal possible commitment value: 80% of average for pool 2
// 					// Action::Reward { amount: 10 * UNIT },
// 					Action::CooldownComputeCommitment { committer: "C".to_string() },
// 					Action::RollToBlock {
// 						block_number: 700, // Advance past cooldown period (started at 602, +36 blocks + buffer)
// 						expected_cycle: Cycle { epoch: 6, epoch_start: 602 },
// 					},
// 					Action::EndComputeCommitment {
// 						committer: "C".to_string(),
// 						expected_reward: 5 * UNIT,
// 					},
// 				][..],
// 			]
// 			.concat(),
// 		);
// 	});
// }

// #[test]
// fn test_compute_flow_stake_more() {
// 	ExtBuilder.build().execute_with(|| {
// 		compute_test_flow(
// 			2,
// 			&[30, 50, 20],
// 			&[
// 				("C", &["A", "B"]), // committer C with processors A, B
// 			],
// 			&[
// 				&commit_actions_2_processors()[..],
// 				&[
// 					Action::CommitCompute {
// 						committer: "C".to_string(),
// 						stake: 5 * UNIT,
// 						cooldown: 36, // 1/3 of max
// 						metrics: vec![(2, 4000u128 * 4 / 5, 1u128)],
// 						commission: Perbill::from_percent(10),
// 					}, // Maximal possible commitment value: 80% of average for pool 2
// 					Action::Delegate {
// 						delegator: "D".to_string(),
// 						committer: "C".to_string(),
// 						amount: 40 * UNIT,
// 						cooldown: 36,
// 					},
// 					Action::Delegate {
// 						delegator: "E".to_string(),
// 						committer: "C".to_string(),
// 						amount: 5 * UNIT,
// 						cooldown: 36,
// 					},
// 					// Action::Reward { amount: 10 * UNIT },
// 					Action::StakeMore { committer: "C".to_string(), extra_amount: 10 * UNIT },
// 					Action::CooldownComputeCommitment { committer: "C".to_string() },
// 					Action::CooldownDelegation {
// 						delegator: "D".to_string(),
// 						committer: "C".to_string(),
// 					},
// 					Action::CooldownDelegation {
// 						delegator: "E".to_string(),
// 						committer: "C".to_string(),
// 					},
// 					Action::RollToBlock {
// 						block_number: 400, // Advance past cooldown period (started at 302, +36 blocks + buffer)
// 						expected_cycle: Cycle { epoch: 3, epoch_start: 302 },
// 					},
// 					Action::EndComputeCommitment {
// 						committer: "C".to_string(),
// 						expected_reward: 950 * MILLIUNIT,
// 					},
// 					// D committed 40 for 1/3 of max cooldown
// 					// vs E committed 5 for 1/3 of max cooldown
// 					// That makes D get 40/45 of delegators' total payout
// 					//
// 					// NOTE: committer has equal 1/3 of max cooldown so equal weight as delegators from this perspective
// 					// total delegator payout = 10 [single reward] * 0.5 [metric commitment] * 1/1 [cooldown ratio] * 45/(45 + 5) [delegator vs total commitment stake] * 0.9 [commission]
// 					//                        = 4.05
// 					//
// 					// Delegator D's payout = 4.05 * 40/45
// 					//                      = 3.6
// 					Action::EndDelegation {
// 						delegator: "D".to_string(),
// 						committer: "C".to_string(),
// 						expected_reward: 3_600 * MILLIUNIT,
// 					},
// 					// Delegator D's payout = 4.05 * 5/45
// 					//                      = 0.45
// 					Action::EndDelegation {
// 						delegator: "E".to_string(),
// 						committer: "C".to_string(),
// 						expected_reward: 450_000_000_000,
// 					},
// 				][..],
// 			]
// 			.concat(),
// 		);
// 	});
// }

// #[test]
// fn test_compute_flow_varied_cooldown() {
// 	ExtBuilder.build().execute_with(|| {
// 		compute_test_flow(
// 			2,
// 			&[30, 50, 20], // same pools as original
// 			&[
// 				("C", &["A", "B"]), // committer C with processors A, B
// 			],
// 			&[
// 				&commit_actions_2_processors()[..],
// 				&[
// 					// Test with maximum cooldown (108)
// 					Action::CommitCompute {
// 						committer: "C".to_string(),
// 						stake: 5 * UNIT,
// 						cooldown: 108, // maximum cooldown
// 						metrics: vec![(2, 4000u128 * 4 / 5, 1u128)],
// 						commission: Perbill::from_percent(10),
// 					},
// 					Action::Delegate {
// 						delegator: "D".to_string(),
// 						committer: "C".to_string(),
// 						amount: 40 * UNIT,
// 						cooldown: 108, // matching maximum cooldown
// 					},
// 					Action::Delegate {
// 						delegator: "E".to_string(),
// 						committer: "C".to_string(),
// 						amount: 5 * UNIT,
// 						cooldown: 72, // different cooldown to test weight calculation
// 					},
// 					// Action::Reward { amount: 10 * UNIT },
// 					Action::CooldownComputeCommitment { committer: "C".to_string() },
// 					Action::CooldownDelegation {
// 						delegator: "D".to_string(),
// 						committer: "C".to_string(),
// 					},
// 					Action::CooldownDelegation {
// 						delegator: "E".to_string(),
// 						committer: "C".to_string(),
// 					},
// 					Action::RollToBlock {
// 						block_number: 480, // Advance past max cooldown period (started at 302, +108 blocks + buffer)
// 						expected_cycle: Cycle { epoch: 4, epoch_start: 402 },
// 					},
// 					// D committed 40 for 1/1 of max cooldown
// 					// vs E committed 5 for 2/3 of max cooldown
// 					// That makes D get 40/45 * 4/6 of delegators' total payout
// 					// and E gets the remaining 5/45 * 2/6 of delegators' total payout
// 					//
// 					// NOTE: Cooldown ratio between delegators and committer is (1/1 + 2/3)/(1/1 + 2/3 + 1/1) = (3/3 + 2/3)/(3/3 + 2/3 + 3/3) = (5/3)/(8/3) = 5/8
// 					//
// 					// total delegator payout = 10 [single reward] * 0.5 [metric commitment] * 5/8 [cooldown ratio] * 45/(45 + 5) [delegator vs total commitment stake] * 0.9 [commission]
// 					//                        = 2.53125
// 					//
// 					// Delegator D's payout = 2.53125 * 40/45 * 3/5
// 					//                      = 1.35
// 					Action::EndComputeCommitment {
// 						committer: "C".to_string(),
// 						expected_reward: 965517241378,
// 					},
// 					Action::EndDelegation {
// 						delegator: "D".to_string(),
// 						committer: "C".to_string(),
// 						expected_reward: 3724137931034,
// 					},
// 					Action::EndDelegation {
// 						delegator: "E".to_string(),
// 						committer: "C".to_string(),
// 						expected_reward: 310344827586,
// 					},
// 				][..],
// 			]
// 			.concat(),
// 		);
// 	});
// }

// #[test]
// fn test_compute_flow_varied_stakes() {
// 	ExtBuilder.build().execute_with(|| {
// 		compute_test_flow(
// 			2,
// 			&[30, 50, 20], // same pools as original
// 			&[
// 				("C", &["A", "B"]), // committer C with processors A, B
// 			],
// 			&[
// 				&commit_actions_2_processors()[..],
// 				&[
// 					// Test with different stake amount (10 instead of 5)
// 					Action::CommitCompute {
// 						committer: "C".to_string(),
// 						stake: 10 * UNIT, // doubled stake
// 						cooldown: 36,
// 						metrics: vec![(2, 4000u128 * 4 / 5, 1u128)],
// 						commission: Perbill::from_percent(10),
// 					},
// 					Action::Delegate {
// 						delegator: "D".to_string(),
// 						committer: "C".to_string(),
// 						amount: 80 * UNIT, // doubled amount to maintain ratio
// 						cooldown: 36,
// 					},
// 					Action::Delegate {
// 						delegator: "E".to_string(),
// 						committer: "C".to_string(),
// 						amount: 10 * UNIT, // doubled amount
// 						cooldown: 36,
// 					},
// 					// Action::Reward { amount: 10 * UNIT },
// 					Action::CooldownComputeCommitment { committer: "C".to_string() },
// 					Action::CooldownDelegation {
// 						delegator: "D".to_string(),
// 						committer: "C".to_string(),
// 					},
// 					Action::CooldownDelegation {
// 						delegator: "E".to_string(),
// 						committer: "C".to_string(),
// 					},
// 					Action::RollToBlock {
// 						block_number: 400,
// 						expected_cycle: Cycle { epoch: 3, epoch_start: 302 },
// 					},
// 					Action::EndComputeCommitment {
// 						committer: "C".to_string(),
// 						expected_reward: 950 * MILLIUNIT,
// 					},
// 					Action::EndDelegation {
// 						delegator: "D".to_string(),
// 						committer: "C".to_string(),
// 						expected_reward: 3600 * MILLIUNIT,
// 					},
// 					Action::EndDelegation {
// 						delegator: "E".to_string(),
// 						committer: "C".to_string(),
// 						expected_reward: 450 * MILLIUNIT,
// 					},
// 				][..],
// 			]
// 			.concat(),
// 		);
// 	});
// }

// #[test]
// fn test_compute_flow_multi_pool_metrics() {
// 	ExtBuilder.build().execute_with(|| {
// 		compute_test_flow(
// 			2,
// 			&[30, 30, 20, 20], // four pools with different allocations
// 			&[
// 				("C", &["A", "B"]), // committer C with processors A, B
// 			],
// 			&[
// 				// Use first part of commit_actions but with modified metrics for multi-pool test
// 				Action::RollToBlock {
// 					block_number: 10,
// 					expected_cycle: Cycle { epoch: 0, epoch_start: 2 },
// 				},
// 				Action::ProcessorCommit { processor: "A".to_string(), metrics: vec![(1, 1000, 1)] },
// 				Action::RollToBlock {
// 					block_number: 20,
// 					expected_cycle: Cycle { epoch: 0, epoch_start: 2 },
// 				},
// 				Action::ProcessorCommit { processor: "B".to_string(), metrics: vec![(1, 1000, 1)] },
// 				Action::RollToBlock {
// 					block_number: 150,
// 					expected_cycle: Cycle { epoch: 1, epoch_start: 102 },
// 				},
// 				// A commits to multiple pools
// 				Action::ProcessorCommit {
// 					processor: "A".to_string(),
// 					metrics: vec![(1, 1000, 1), (2, 2000, 1), (3, 1500, 1)],
// 				},
// 				// B commits to different pools
// 				Action::ProcessorCommit {
// 					processor: "B".to_string(),
// 					metrics: vec![(2, 6000, 1), (3, 3000, 1), (4, 2500, 1)],
// 				},
// 				Action::RollToBlock {
// 					block_number: 302,
// 					expected_cycle: Cycle { epoch: 3, epoch_start: 302 },
// 				},
// 				// Commit to multiple pools with different metrics
// 				Action::CommitCompute {
// 					committer: "C".to_string(),
// 					stake: 5 * UNIT,
// 					cooldown: 36,
// 					metrics: vec![
// 						(2, 4000u128 * 4 / 5, 1u128), // pool 2
// 						(3, 2250u128 * 4 / 5, 1u128), // pool 3 (average of 1500 and 3000)
// 					],
// 					commission: Perbill::from_percent(10),
// 				},
// 				Action::Delegate {
// 					delegator: "D".to_string(),
// 					committer: "C".to_string(),
// 					amount: 40 * UNIT,
// 					cooldown: 36,
// 				},
// 				Action::Delegate {
// 					delegator: "E".to_string(),
// 					committer: "C".to_string(),
// 					amount: 5 * UNIT,
// 					cooldown: 36,
// 				},
// 				// Action::Reward { amount: 10 * UNIT },
// 				Action::CooldownComputeCommitment { committer: "C".to_string() },
// 				Action::CooldownDelegation {
// 					delegator: "D".to_string(),
// 					committer: "C".to_string(),
// 				},
// 				Action::CooldownDelegation {
// 					delegator: "E".to_string(),
// 					committer: "C".to_string(),
// 				},
// 				Action::RollToBlock {
// 					block_number: 400,
// 					expected_cycle: Cycle { epoch: 3, epoch_start: 302 },
// 				},
// 				Action::EndComputeCommitment {
// 					committer: "C".to_string(),
// 					expected_reward: 950 * MILLIUNIT, // Actual reward with multi-pool metrics
// 				},
// 				Action::EndDelegation {
// 					delegator: "D".to_string(),
// 					committer: "C".to_string(),
// 					expected_reward: 3600 * MILLIUNIT, // Actual reward with multi-pool metrics
// 				},
// 				Action::EndDelegation {
// 					delegator: "E".to_string(),
// 					committer: "C".to_string(),
// 					expected_reward: 450 * MILLIUNIT, // Actual reward with multi-pool metrics
// 				},
// 			],
// 		);
// 	});
// }

// #[test]
// fn test_compute_flow_large_metrics() {
// 	ExtBuilder.build().execute_with(|| {
// 		compute_test_flow(
// 			2,
// 			&[30, 30, 20, 20], // four pools with different allocations
// 			&[
// 				("C", &["A", "B"]), // committer C with processors A, B
// 			],
// 			&[
// 				// Use first part of commit_actions but with modified metrics for multi-pool test
// 				Action::RollToBlock {
// 					block_number: 10,
// 					expected_cycle: Cycle { epoch: 0, epoch_start: 2 },
// 				},
// 				Action::ProcessorCommit { processor: "A".to_string(), metrics: vec![(1, 1000, 1)] },
// 				Action::RollToBlock {
// 					block_number: 20,
// 					expected_cycle: Cycle { epoch: 0, epoch_start: 2 },
// 				},
// 				Action::ProcessorCommit { processor: "B".to_string(), metrics: vec![(1, 1000, 1)] },
// 				Action::RollToBlock {
// 					block_number: 150,
// 					expected_cycle: Cycle { epoch: 1, epoch_start: 102 },
// 				},
// 				// A commits to multiple pools
// 				Action::ProcessorCommit {
// 					processor: "A".to_string(),
// 					metrics: vec![(1, 89844839219, 1), (2, 89844839219, 1), (3, 89844839219, 1)],
// 				},
// 				// B commits to different pools
// 				Action::ProcessorCommit {
// 					processor: "B".to_string(),
// 					metrics: vec![(2, 89844839219, 1), (3, 89844839219, 1), (4, 89844839219, 1)],
// 				},
// 				Action::RollToBlock {
// 					block_number: 302,
// 					expected_cycle: Cycle { epoch: 3, epoch_start: 302 },
// 				},
// 				// Commit to multiple pools with different metrics
// 				Action::CommitCompute {
// 					committer: "C".to_string(),
// 					stake: 1_000_000_000 * UNIT,
// 					cooldown: 36,
// 					metrics: vec![
// 						(2, 89844839219 / 5 * 4, 1u128), // pool 2
// 						(3, 89844839219 / 5 * 4, 1u128), // pool 3 (average of 1500 and 3000)
// 					],
// 					commission: Perbill::from_percent(10),
// 				},
// 				Action::Delegate {
// 					delegator: "D".to_string(),
// 					committer: "C".to_string(),
// 					amount: 1_000_000_000 * UNIT,
// 					cooldown: 36,
// 				},
// 				Action::Delegate {
// 					delegator: "E".to_string(),
// 					committer: "C".to_string(),
// 					amount: 1_000_000_000 * UNIT,
// 					cooldown: 36,
// 				},
// 				// Action::Reward { amount: 100_000_000_000 * UNIT },
// 				Action::CooldownComputeCommitment { committer: "C".to_string() },
// 				Action::CooldownDelegation {
// 					delegator: "D".to_string(),
// 					committer: "C".to_string(),
// 				},
// 				Action::CooldownDelegation {
// 					delegator: "E".to_string(),
// 					committer: "C".to_string(),
// 				},
// 				Action::RollToBlock {
// 					block_number: 400,
// 					expected_cycle: Cycle { epoch: 3, epoch_start: 302 },
// 				},
// 				Action::EndComputeCommitment {
// 					committer: "C".to_string(),
// 					expected_reward: 20_000_000_000 * UNIT,
// 				},
// 				Action::EndDelegation {
// 					delegator: "D".to_string(),
// 					committer: "C".to_string(),
// 					expected_reward: 15_000_000_000 * UNIT,
// 				},
// 				Action::EndDelegation {
// 					delegator: "E".to_string(),
// 					committer: "C".to_string(),
// 					expected_reward: 15_000_000_000 * UNIT,
// 				},
// 			],
// 		);
// 	});
// }

// #[test]
// fn test_compute_flow_large_metrics_tiny_reward() {
// 	ExtBuilder.build().execute_with(|| {
// 		compute_test_flow(
// 			2,
// 			&[30, 30, 20, 20], // four pools with different allocations
// 			&[
// 				("C", &["A", "B"]), // committer C with processors A, B
// 			],
// 			&[
// 				// Use first part of commit_actions but with modified metrics for multi-pool test
// 				Action::RollToBlock {
// 					block_number: 10,
// 					expected_cycle: Cycle { epoch: 0, epoch_start: 2 },
// 				},
// 				Action::ProcessorCommit { processor: "A".to_string(), metrics: vec![(1, 1000, 1)] },
// 				Action::RollToBlock {
// 					block_number: 20,
// 					expected_cycle: Cycle { epoch: 0, epoch_start: 2 },
// 				},
// 				Action::ProcessorCommit { processor: "B".to_string(), metrics: vec![(1, 1000, 1)] },
// 				Action::RollToBlock {
// 					block_number: 150,
// 					expected_cycle: Cycle { epoch: 1, epoch_start: 102 },
// 				},
// 				// A commits to multiple pools
// 				Action::ProcessorCommit {
// 					processor: "A".to_string(),
// 					metrics: vec![(1, 89844839219, 1), (2, 89844839219, 1), (3, 89844839219, 1)],
// 				},
// 				// B commits to different pools
// 				Action::ProcessorCommit {
// 					processor: "B".to_string(),
// 					metrics: vec![(2, 89844839219, 1), (3, 89844839219, 1), (4, 89844839219, 1)],
// 				},
// 				Action::RollToBlock {
// 					block_number: 302,
// 					expected_cycle: Cycle { epoch: 3, epoch_start: 302 },
// 				},
// 				// Commit to multiple pools with different metrics
// 				Action::CommitCompute {
// 					committer: "C".to_string(),
// 					stake: 1_000_000_000 * UNIT,
// 					cooldown: 36,
// 					metrics: vec![
// 						(2, 89844839219 / 5 * 4, 1u128), // pool 2
// 						(3, 89844839219 / 5 * 4, 1u128), // pool 3 (average of 1500 and 3000)
// 					],
// 					commission: Perbill::from_percent(10),
// 				},
// 				Action::Delegate {
// 					delegator: "D".to_string(),
// 					committer: "C".to_string(),
// 					amount: 1_000_000_000 * UNIT,
// 					cooldown: 36,
// 				},
// 				Action::Delegate {
// 					delegator: "E".to_string(),
// 					committer: "C".to_string(),
// 					amount: 1_000_000_000 * UNIT,
// 					cooldown: 36,
// 				},
// 				// Action::Reward { amount: 1 * MILLIUNIT },
// 				Action::CooldownComputeCommitment { committer: "C".to_string() },
// 				Action::CooldownDelegation {
// 					delegator: "D".to_string(),
// 					committer: "C".to_string(),
// 				},
// 				Action::CooldownDelegation {
// 					delegator: "E".to_string(),
// 					committer: "C".to_string(),
// 				},
// 				Action::RollToBlock {
// 					block_number: 400,
// 					expected_cycle: Cycle { epoch: 3, epoch_start: 302 },
// 				},
// 				Action::EndComputeCommitment {
// 					committer: "C".to_string(),
// 					expected_reward: 200 * MICROUNIT,
// 				},
// 				Action::EndDelegation {
// 					delegator: "D".to_string(),
// 					committer: "C".to_string(),
// 					expected_reward: 150 * MICROUNIT,
// 				},
// 				Action::EndDelegation {
// 					delegator: "E".to_string(),
// 					committer: "C".to_string(),
// 					expected_reward: 150 * MICROUNIT,
// 				},
// 			],
// 		);
// 	});
// }

//fn assert_delegator_withdrew_event(expected_event: Event<Test>) {
//	let withdrew_events: Vec<_> = events()
//		.into_iter()
//		.filter_map(|e| match e {
//			RuntimeEvent::Compute(Event::DelegatorWithdrew(delegator, cid, amount)) => {
//				Some((delegator, cid, amount))
//			},
//			_ => None,
//		})
//		.collect();
//
//	// Should have exactly one DelegatorWithdrew event
//	assert_eq!(withdrew_events.len(), 1);
//	let (event_delegator, event_commitment_id, event_reward_amount) = &withdrew_events[0];
//
//	// Extract expected values from the input event
//	if let Event::DelegatorWithdrew(expected_delegator, expected_cid, expected_amount) =
//		expected_event
//	{
//		assert_eq!(
//			(event_delegator.clone(), *event_commitment_id, *event_reward_amount),
//			(expected_delegator, expected_cid, expected_amount)
//		);
//	} else {
//		panic!("Expected DelegatorWithdrew event");
//	}
//}

#[test]
fn test_create_pools_name_conflict() {
	ExtBuilder.build().execute_with(|| {
		setup_balances();
		// create pool 1
		{
			assert_ok!(Compute::create_pool(
				RuntimeOrigin::root(),
				*b"cpu-ops-per-second______",
				Perquintill::from_percent(25),
				bounded_vec![],
			));
			assert_eq!(Compute::last_metric_pool_id(), 1);
		}

		// create pool 2
		assert_err!(
			Compute::create_pool(
				RuntimeOrigin::root(),
				*b"cpu-ops-per-second______",
				Perquintill::from_percent(50),
				bounded_vec![],
			),
			Error::<Test, ()>::PoolNameMustBeUnique
		);
	});
}

#[test]
fn test_single_processor_commit() {
	ExtBuilder.build().execute_with(|| {
		assert_ok!(Compute::enable_inflation(RuntimeOrigin::root()));
		setup_balances();
		assert_ok!(Compute::create_pool(
			RuntimeOrigin::root(),
			*b"cpu-ops-per-second______",
			Perquintill::from_percent(25),
			bounded_vec![],
		));
		assert_eq!(Compute::last_metric_pool_id(), 1);

		roll_to_block(10);
		assert_eq!(Compute::metrics(alice_account_id(), 1), None);
		let manager =
			<Test as Config>::ManagerProviderForEligibleProcessor::lookup(&alice_account_id())
				.unwrap();
		assert_eq!(
			Compute::commit(&alice_account_id(), &manager, &[(1u8, 1000u128, 1u128)]).0,
			Zero::zero()
		);
		// With roll_to_block calling on_initialize for each block 1-10, epoch_offset changes
		assert_eq!(
			Compute::processors(alice_account_id()),
			Some(ProcessorState {
				epoch_offset: 8,
				committed: 0,
				claimed: 0,
				status: ProcessorStatus::WarmupUntil(40),
				reward_contribution: 0,
				paid: 0
			})
		);

		roll_to_block(302 + 39);
		assert_eq!(
			Compute::commit(&alice_account_id(), &manager, &[(1u8, 1000u128, 1u128)]).0,
			Zero::zero()
		);
		assert_eq!(
			Compute::metrics(alice_account_id(), 1).unwrap(),
			MetricCommit { epoch: 3, metric: FixedU128::from_rational(1000u128, 1u128) }
		);
		// Warmup is over
		assert_eq!(
			Compute::processors(alice_account_id()),
			Some(ProcessorState {
				epoch_offset: 8,
				committed: 3,
				claimed: 0,
				status: ProcessorStatus::Active,
				reward_contribution: 0,
				paid: 0
			})
		);

		assert_eq!(
			Compute::commit(&alice_account_id(), &manager, &[(1u8, 1000u128, 1u128)]).0,
			Zero::zero()
		);
		assert_eq!(
			Compute::metrics(alice_account_id(), 1).unwrap(),
			MetricCommit { epoch: 3, metric: FixedU128::from_rational(1000u128, 1u128) }
		);
		assert_eq!(
			Compute::processors(alice_account_id()),
			Some(ProcessorState {
				epoch_offset: 8,
				committed: 3,
				claimed: 0,
				status: ProcessorStatus::Active,
				reward_contribution: 0,
				paid: 0
			})
		);

		roll_to_block(302 + 130);
		assert_eq!(
			Compute::commit(&alice_account_id(), &manager, &[(1u8, 1000u128, 1u128)]).0,
			642123287671232
		);
		assert_eq!(
			Compute::metrics(alice_account_id(), 1).unwrap(),
			MetricCommit { epoch: 4, metric: FixedU128::from_rational(1000u128, 1u128) }
		);
		assert_eq!(
			Compute::processors(alice_account_id()),
			Some(ProcessorState {
				epoch_offset: 8,
				committed: 4,
				claimed: 0,
				status: ProcessorStatus::Active,
				reward_contribution: 642123287671232,
				paid: 0
			})
		);

		// commit different value in same epoch (does not change existing values for same epoch since first value is kept)
		roll_to_block(302 + 170);
		assert_eq!(
			Compute::commit(&alice_account_id(), &manager, &[(1u8, 2000u128, 1u128)]).0,
			Zero::zero()
		);
		assert_eq!(
			Compute::metrics(alice_account_id(), 1).unwrap(),
			MetricCommit { epoch: 4, metric: FixedU128::from_rational(1000u128, 1u128) }
		);
		assert_eq!(
			Compute::processors(alice_account_id()),
			Some(ProcessorState {
				epoch_offset: 8,
				committed: 4,
				claimed: 0,
				status: ProcessorStatus::Active,
				reward_contribution: 642123287671232,
				paid: 0
			})
		);
		assert_eq!(
			Compute::metric_pools(1).unwrap().total.get(4),
			FixedU128::from_rational(1000u128, 1u128)
		);

		// claim for epoch 1 and commit for epoch 2
		roll_to_block(302 + 230);
		assert_eq!(
			Compute::commit(&alice_account_id(), &manager, &[(1u8, 1000u128, 1u128)]).0,
			642123287671232
		);
		assert_eq!(
			Compute::metrics(alice_account_id(), 1).unwrap(),
			MetricCommit { epoch: 5, metric: FixedU128::from_rational(1000u128, 1u128) }
		);
		assert_eq!(
			Compute::processors(alice_account_id()),
			Some(ProcessorState {
				epoch_offset: 8,
				committed: 5,
				claimed: 0,
				status: ProcessorStatus::Active,
				reward_contribution: 642123287671232,
				paid: 0,
			})
		);

		let events = events();
		let expected = [RuntimeEvent::Compute(Event::PoolCreated(
			1,
			MetricPool {
				config: bounded_vec![],
				name: *b"cpu-ops-per-second______",
				reward: ProvisionalBuffer::from_inner(Perquintill::from_percent(25), None),
				total: SlidingBuffer::from_inner(0u64, 0.into(), 0.into()),
				total_with_bonus: SlidingBuffer::from_inner(0u64, 0.into(), 0.into()),
			},
		))];
		assert!(expected.iter().all(|event| events.contains(event)));
	});
}

fn create_pools() {
	// create pool 1
	{
		assert_ok!(Compute::create_pool(
			RuntimeOrigin::root(),
			*b"cpu-ops-per-second______",
			Perquintill::from_percent(25),
			bounded_vec![],
		));
		assert_eq!(Compute::last_metric_pool_id(), 1);
	}

	// create pool 2
	{
		assert_ok!(Compute::create_pool(
			RuntimeOrigin::root(),
			*b"mem-read-count-per-sec--",
			Perquintill::from_percent(50),
			bounded_vec![],
		));
		assert_eq!(Compute::last_metric_pool_id(), 2);
	}

	// create pool 3
	{
		assert_ok!(Compute::create_pool(
			RuntimeOrigin::root(),
			*b"mem-write-count-per-sec-",
			Perquintill::from_percent(25),
			bounded_vec![],
		));
		assert_eq!(Compute::last_metric_pool_id(), 3);
	}
}

fn commit_alice_bob() {
	let alice_manager =
		<Test as Config>::ManagerProviderForEligibleProcessor::lookup(&alice_account_id()).unwrap();
	let bob_manager =
		<Test as Config>::ManagerProviderForEligibleProcessor::lookup(&bob_account_id()).unwrap();
	// Alice commits first time
	{
		roll_to_block(10);
		assert_eq!(Compute::current_cycle(), Cycle { epoch: 0, epoch_start: 2 });
		assert_eq!(Compute::metrics(alice_account_id(), 1), None);
		assert_eq!(
			Compute::commit(&alice_account_id(), &alice_manager, &[(1u8, 1000u128, 1u128)]).0,
			Zero::zero()
		);
		assert_eq!(
			Compute::processors(alice_account_id()).unwrap().status,
			ProcessorStatus::WarmupUntil(40)
		);
		assert_eq!(Compute::processors(alice_account_id()).unwrap().epoch_offset, 8);
	}

	// Bob commits first time
	{
		roll_to_block(20);
		assert_eq!(Compute::metrics(bob_account_id(), 1), None);
		assert_eq!(
			Compute::commit(&bob_account_id(), &bob_manager, &[(1u8, 1000u128, 1u128)]).0,
			Zero::zero()
		);
		assert_eq!(
			Compute::processors(bob_account_id()).unwrap().status,
			ProcessorStatus::WarmupUntil(50)
		);
		assert_eq!(Compute::processors(bob_account_id()).unwrap().epoch_offset, 18);
	}

	// Warmup is over for both Alice and Bob so this commits is rewardable since they commit for an active epoch
	roll_to_block(150);
	assert_eq!(Compute::current_cycle().epoch, 1);

	// Alice commits values for epoch 1 (where she is active) for pool 1 and 2
	{
		assert_eq!(
			Compute::commit(
				&alice_account_id(),
				&alice_manager,
				&[(1u8, 1000u128, 1u128), (2u8, 2000u128, 1u128)]
			)
			.0,
			Zero::zero()
		);
		assert_eq!(
			Compute::metrics(alice_account_id(), 1).unwrap(),
			MetricCommit { epoch: 1, metric: FixedU128::from_rational(1000u128, 1u128) }
		);
		assert_eq!(
			Compute::metrics(alice_account_id(), 2).unwrap(),
			MetricCommit { epoch: 1, metric: FixedU128::from_rational(2000u128, 1u128) }
		);
		assert_eq!(
			Compute::processors(alice_account_id()),
			Some(ProcessorState {
				epoch_offset: 8,
				committed: 1,
				claimed: 0,
				status: ProcessorStatus::Active,
				reward_contribution: 0,
				paid: 0
			})
		);
	}

	// Bob commits values for epoch 1 (where he is active) for only pool 2
	{
		assert_eq!(
			Compute::commit(&bob_account_id(), &bob_manager, &[(2u8, 6000u128, 1u128)]).0,
			Zero::zero()
		);
		assert_eq!(
			Compute::metrics(bob_account_id(), 2).unwrap(),
			MetricCommit { epoch: 1, metric: FixedU128::from_rational(6000u128, 1u128) }
		);
		assert_eq!(
			Compute::processors(bob_account_id()),
			Some(ProcessorState {
				epoch_offset: 18,
				committed: 1,
				claimed: 0,
				status: ProcessorStatus::Active,
				reward_contribution: 0,
				paid: 0
			})
		);
	}

	// check totals
	assert_eq!(
		Compute::metric_pools(1).unwrap().total.get(1),
		FixedU128::from_rational(1000u128, 1u128)
	);
	assert_eq!(
		Compute::metric_pools(2).unwrap().total.get(1),
		FixedU128::from_rational(8000u128, 1u128)
	);
}

fn commit(with_charlie: bool, modify_reward: bool) {
	commit_alice_bob();

	// An admin changes the reward from now on (should not influence rewards for epoch 1)
	if modify_reward {
		assert_ok!(Compute::modify_pool(
			RuntimeOrigin::root(),
			2,
			None,
			Some((2, Perquintill::from_percent(40))),
			None
		));
		assert_ok!(Compute::modify_pool(
			RuntimeOrigin::root(),
			1,
			None,
			Some((2, Perquintill::from_percent(35))),
			None
		));
	}

	let alice_manager =
		<Test as Config>::ManagerProviderForEligibleProcessor::lookup(&alice_account_id()).unwrap();
	let bob_manager =
		<Test as Config>::ManagerProviderForEligibleProcessor::lookup(&bob_account_id()).unwrap();
	let charlie_manager =
		<Test as Config>::ManagerProviderForEligibleProcessor::lookup(&charlie_account_id())
			.unwrap();

	// Charlie commits first time (to all pools)
	if with_charlie {
		roll_to_block(190);
		assert_eq!(Compute::metrics(charlie_account_id(), 1), None);
		assert_eq!(
			Compute::commit(
				&charlie_account_id(),
				&charlie_manager,
				&[(1u8, 1234u128, 10u128), (2u8, 1234u128, 10u128), (3u8, 1234u128, 10u128)]
			)
			.0,
			Zero::zero()
		);
		assert_eq!(
			Compute::processors(charlie_account_id()).unwrap().status,
			ProcessorStatus::WarmupUntil(220)
		);
		assert_eq!(Compute::processors(charlie_account_id()).unwrap().epoch_offset, 88);
	}

	// Charlie commits values for epoch 2 (where he is active) for all pools, but should not disturb the reward payment below for epoch 1 for Alice and Bob
	roll_to_block(210);
	if with_charlie {
		assert_eq!(
			Compute::metrics(charlie_account_id(), 1).unwrap(),
			MetricCommit { epoch: 1, metric: FixedU128::from_rational(1234u128, 10u128) }
		);
		assert_eq!(
			Compute::commit(
				&charlie_account_id(),
				&charlie_manager,
				&[(1u8, 1234u128, 10u128), (2u8, 1234u128, 10u128), (3u8, 1234u128, 10u128)]
			)
			.0,
			Zero::zero()
		);
		assert_eq!(
			Compute::processors(charlie_account_id()).unwrap().status,
			ProcessorStatus::WarmupUntil(220)
		);
	}

	// Alice successfully claims
	{
		// Reward calculation for epoch 1:
		// - total reward = 1 UNIT
		// - Alice is sole committer to pool 1, reward is configured at 25% => leaves here with 0.25 UNIT (independent of her metric value committed)
		// - Alice committed 2000 to pool 2 together with Bob which committed 8000, which leaves here with 1/4 of the rewards for pool 2 which are 50% of 1 UNIT
		//   => 0.25 * 0.5 * 1 UNIT = 0.125 UNIT
		// - Sum of reward for pool 1 and pool 2 = 0.25 UNIT + 0.125 UNIT = 0.375
		assert_eq!(
			Compute::commit(
				&alice_account_id(),
				&alice_manager,
				&[(1u8, 1000u128, 1u128), (2u8, 2000u128, 1u128)]
			)
			.0,
			1926369863013698,
		);
		assert_eq!(
			Compute::metrics(alice_account_id(), 1).unwrap(),
			MetricCommit { epoch: 2, metric: FixedU128::from_rational(1000u128, 1u128) }
		);
		assert_eq!(
			Compute::processors(alice_account_id()),
			Some(ProcessorState {
				epoch_offset: 8,
				committed: 2,
				claimed: 0,
				status: ProcessorStatus::Active,
				reward_contribution: 963184931506849,
				paid: 0,
			})
		);
	}

	// Bob successfully claims
	{
		// Reward calculation for epoch 1:
		// - total reward = 1 UNIT
		// - Bob committed 6000 to pool 2 together with Alice which committed 6000, which leaves him with 3/4 of the rewards for pool 2 which are 50% of 1 UNIT
		//   => 0.75 * 0.5 * 1 UNIT = 0.375 UNIT
		assert_eq!(
			Compute::commit(
				&bob_account_id(),
				&bob_manager,
				&[(1u8, 1000u128, 1u128), (2u8, 2000u128, 1u128)]
			)
			.0,
			0, // already claimed by Alice above
		);
		assert_eq!(
			Compute::metrics(bob_account_id(), 1).unwrap(),
			MetricCommit { epoch: 2, metric: FixedU128::from_rational(1000u128, 1u128) }
		);
		assert_eq!(
			Compute::processors(bob_account_id()),
			Some(ProcessorState {
				epoch_offset: 18,
				committed: 2,
				claimed: 0,
				status: ProcessorStatus::Active,
				reward_contribution: 1605308219178081,
				paid: 0,
			})
		);
	}
}

fn check_events() {
	let events = events();
	let expected = [
		RuntimeEvent::Compute(Event::PoolCreated(
			1,
			MetricPool {
				config: bounded_vec![],
				name: *b"cpu-ops-per-second______",
				reward: ProvisionalBuffer::from_inner(Perquintill::from_percent(25), None),
				total: SlidingBuffer::from_inner(0u64, 0.into(), 0.into()),
				total_with_bonus: SlidingBuffer::from_inner(0u64, 0.into(), 0.into()),
			},
		)),
		RuntimeEvent::Compute(Event::PoolCreated(
			2,
			MetricPool {
				config: bounded_vec![],
				name: *b"mem-read-count-per-sec--",
				reward: ProvisionalBuffer::from_inner(Perquintill::from_percent(50), None),
				total: SlidingBuffer::from_inner(0u64, 0.into(), 0.into()),
				total_with_bonus: SlidingBuffer::from_inner(0u64, 0.into(), 0.into()),
			},
		)),
		RuntimeEvent::Compute(Event::PoolCreated(
			3,
			MetricPool {
				config: bounded_vec![],
				name: *b"mem-write-count-per-sec-",
				reward: ProvisionalBuffer::from_inner(Perquintill::from_percent(25), None),
				total: SlidingBuffer::from_inner(0u64, 0.into(), 0.into()),
				total_with_bonus: SlidingBuffer::from_inner(0u64, 0.into(), 0.into()),
			},
		)),
	];
	assert!(expected.iter().all(|event| events.contains(event)));
}

#[test]
fn test_multiple_processor_commit() {
	ExtBuilder.build().execute_with(|| {
		assert_ok!(Compute::enable_inflation(RuntimeOrigin::root()));
		// Use test helpers to replicate test below with interleaving Charlie's commit (should return same rewards for Alice and Bob)
		setup_balances();
		create_pools();
		commit(false, false);
		check_events();
	});
}

#[test]
fn test_multiple_processor_commit_reward_modified() {
	ExtBuilder.build().execute_with(|| {
		assert_ok!(Compute::enable_inflation(RuntimeOrigin::root()));
		// Use test helpers to replicate test below with interleaving Charlie's commit (should return same rewards for Alice and Bob)
		setup_balances();
		create_pools();
		commit(false, true);
		check_events();
	});
}

#[test]
fn test_multiple_processor_commit_with_interleaving_charlie() {
	ExtBuilder.build().execute_with(|| {
		assert_ok!(Compute::enable_inflation(RuntimeOrigin::root()));
		// Use test helpers to replicate test below with interleaving Charlie's commit (should return same rewards for Alice and Bob)
		setup_balances();
		create_pools();
		commit(true, false);
		check_events();
	});
}

#[test]
fn test_multiple_processor_commit_with_interleaving_charlie_reward_modified() {
	ExtBuilder.build().execute_with(|| {
		assert_ok!(Compute::enable_inflation(RuntimeOrigin::root()));
		// Use test helpers to replicate test below with interleaving Charlie's commit (should return same rewards for Alice and Bob)
		setup_balances();
		create_pools();
		commit(true, true);
		check_events();
	});
}

#[test]
fn test_commit_compute() {
	ExtBuilder.build().execute_with(|| {
		assert_ok!(Compute::enable_inflation(RuntimeOrigin::root()));
		setup_balances();
		create_pools();

		// Charlie will act as both manager and committer (same account for simplicity)
		let charlie = charlie_account_id();

		offer_accept_backing(charlie.clone());

		commit_alice_bob();

		const MANAGER_ID: u128 = 1;

		// pool 1 has only commits in warmup, not counting towards average
		assert_eq!(
			Compute::metrics_epoch_sum(MANAGER_ID, 1), // pool 1
			SlidingBuffer::from_inner(
				1,
				(Zero::zero(), Zero::zero()), // prev
				(
					FixedU128::from_rational(1000u128, 1u128),
					FixedU128::from_rational(1000u128, 1u128)
				)  // cur
			)
		);
		assert_eq!(
			Compute::metrics_epoch_sum(MANAGER_ID, 2), // pool 2
			SlidingBuffer::from_inner(
				1,
				(Zero::zero(), Zero::zero()), // prev
				(
					FixedU128::from_rational(8000u128, 1u128),
					FixedU128::from_rational(8000u128, 1u128)
				)  // cur
			)
		);

		// Step 4: Charlie commits compute (acting as committer backing his own manager account)
		// Start with minimal metrics to test if validation passes
		let exceeding_commitment = bounded_vec![ComputeCommitment {
			pool_id: 2,
			metric: FixedU128::from_rational(8000u128 * 4 / 5 + 1, 1u128), // Maximal possible commitment value + 1
		},];

		let commitment: sp_runtime::BoundedVec<ComputeCommitment, sp_core::ConstU32<30>> =
			bounded_vec![ComputeCommitment {
				pool_id: 2,
				metric: FixedU128::from_rational(4000u128 * 4 / 5, 1u128), // Maximal possible commitment value
			},];

		let stake_amount = 5 * UNIT; // 5 tokens
		let cooldown_period = 36u64; // 1000 blocks
		let commission = Perbill::from_percent(10); // 10% commission
		let allow_auto_compound = true;

		roll_to_block(202);
		assert_eq!(Compute::current_cycle(), Cycle { epoch: 2, epoch_start: 202 });

		let alice_manager =
			<Test as Config>::ManagerProviderForEligibleProcessor::lookup(&alice_account_id())
				.unwrap();
		let bob_manager =
			<Test as Config>::ManagerProviderForEligibleProcessor::lookup(&bob_account_id())
				.unwrap();

		// Alice & Bob recommit
		{
			Compute::commit(
				&alice_account_id(),
				&alice_manager,
				&[(1u8, 1000u128, 1u128), (2u8, 2000u128, 1u128)],
			);
			Compute::commit(&bob_account_id(), &bob_manager, &[(2u8, 6000u128, 1u128)]);
		}

		// Step 5: Charlie commits compute (as the committer)
		assert_err!(
			Compute::commit_compute(
				RuntimeOrigin::signed(charlie.clone()),
				stake_amount,
				cooldown_period,
				exceeding_commitment,
				commission,
				allow_auto_compound,
			),
			Error::<Test, ()>::MaxMetricCommitmentExceeded
		);
		assert_ok!(Compute::commit_compute(
			RuntimeOrigin::signed(charlie.clone()),
			stake_amount,
			cooldown_period,
			commitment,
			commission,
			allow_auto_compound,
		));

		// Verify the commit was successful by checking events or storage
		// At minimum we should see the commitment created event
		assert!(events()
			.iter()
			.any(|e| matches!(e, RuntimeEvent::Compute(Event::CommitmentCreated(_, _)))));

		// Make inflation happen
		roll_to_block(302);
		assert_eq!(Compute::current_cycle(), Cycle { epoch: 3, epoch_start: 302 });

		// Alice & Bob recommit
		{
			Compute::commit(
				&alice_account_id(),
				&alice_manager,
				&[(1u8, 1000u128, 1u128), (2u8, 2000u128, 1u128)],
			);
			Compute::commit(&bob_account_id(), &bob_manager, &[(2u8, 6000u128, 1u128)]);
		}

		// Make all rolling sums be complete (and inflation happens again)
		roll_to_block(402);
		assert_eq!(Compute::current_cycle(), Cycle { epoch: 4, epoch_start: 402 });

		// assert some scores are available for epoch 3
		assert_eq!(
			Compute::scores(0, 2),
			SlidingBuffer::from_inner(
				3,
				(U256::from(0), U256::from(0)),
				(U256::from(1666666666666u128), U256::from(1666666666666u128))
			),
		);

		// Alice & Bob recommit
		{
			Compute::commit(
				&alice_account_id(),
				&alice_manager,
				&[(1u8, 1000u128, 1u128), (2u8, 2000u128, 1u128)],
			);
			Compute::commit(&bob_account_id(), &bob_manager, &[(2u8, 6000u128, 1u128)]);
		}

		assert_ok!(Compute::stake_more(
			RuntimeOrigin::signed(charlie.clone()),
			2 * UNIT,
			None,
			None,
			None,
			None,
		));

		roll_to_block(410);
		assert_ok!(Compute::cooldown_compute_commitment(RuntimeOrigin::signed(charlie.clone()),));

		roll_to_block(445);
		assert_err!(
			Compute::end_compute_commitment(RuntimeOrigin::signed(charlie.clone())),
			Error::<Test, ()>::CooldownNotEnded
		);

		roll_to_block(446);
		assert_ok!(Compute::end_compute_commitment(RuntimeOrigin::signed(charlie.clone()),));

		// Verify the reward was payed out
		// At minimum we should see the commitment created event
		// assert_eq!(events(), []);
		assert!(events().iter().any(|e| matches!(
			e,
			RuntimeEvent::Balances(pallet_balances::Event::Transfer {
				from: _,
				to: _,
				amount: 2996575342465753
			})
		)));
	});
}

#[test]
fn test_commit_compute_overstaked() {
	ExtBuilder.build().execute_with(|| {
		assert_ok!(Compute::enable_inflation(RuntimeOrigin::root()));
		// keep total supply at 1_000_000_000 * UNIT
		assert_ok!(Balances::force_set_balance(RuntimeOrigin::root(), alice_account_id(), UNIT));
		assert_ok!(Balances::force_set_balance(RuntimeOrigin::root(), bob_account_id(), UNIT));
		let charlie_initial_balance: Balance = 999_999_998 * UNIT; // give a lot to committer so he can attempt to overstake
		assert_ok!(Balances::force_set_balance(
			RuntimeOrigin::root(),
			charlie_account_id(),
			charlie_initial_balance
		));

		assert_eq!(Balances::total_issuance(), 1_000_000_000 * UNIT);

		create_pools();

		// Charlie will act as both manager and committer (same account for simplicity)
		let charlie = charlie_account_id();

		offer_accept_backing(charlie.clone());

		commit_alice_bob(); // totals: pool 1: 1000, pool 2: 8000

		let commitment: sp_runtime::BoundedVec<ComputeCommitment, sp_core::ConstU32<30>> =
			bounded_vec![ComputeCommitment {
				pool_id: 2,
				metric: FixedU128::from_rational(1000u128, 1u128), // Maximal possible commitment value
			},];
		// target_weight_per_compute_pool_2 = 1_000_000_000 * UNIT / 8000 / 2  (2 is TargetStakedTokenSupply) = 62500.0 (tests showed actual value is 62500.53510273973)
		let stake_amount_limit = 62_500_535_102_739_730 * 1000;
		let overstake_amount = stake_amount_limit + MILLIUNIT;
		let stake_amount = stake_amount_limit - MILLIUNIT;
		let cooldown_period = 108u64;
		let commission = Perbill::from_percent(10); // 10% commission
		let allow_auto_compound = true;

		roll_to_block(202);
		assert_eq!(Compute::current_cycle(), Cycle { epoch: 2, epoch_start: 202 });

		let alice_manager =
			<Test as Config>::ManagerProviderForEligibleProcessor::lookup(&alice_account_id())
				.unwrap();
		let bob_manager =
			<Test as Config>::ManagerProviderForEligibleProcessor::lookup(&bob_account_id())
				.unwrap();

		// Alice & Bob recommit
		{
			Compute::commit(
				&alice_account_id(),
				&alice_manager,
				&[(1u8, 1000u128, 1u128), (2u8, 2000u128, 1u128)],
			);
			Compute::commit(&bob_account_id(), &bob_manager, &[(2u8, 6000u128, 1u128)]);
		}

		// Step 5: Charlie commits compute (as the committer)
		assert_err!(
			Compute::commit_compute(
				RuntimeOrigin::signed(charlie.clone()),
				overstake_amount,
				cooldown_period,
				commitment.clone(),
				commission,
				allow_auto_compound,
			),
			Error::<Test, ()>::MaxStakeMetricRatioExceeded
		);
		assert_ok!(Compute::commit_compute(
			RuntimeOrigin::signed(charlie.clone()),
			stake_amount,
			cooldown_period,
			commitment,
			commission,
			allow_auto_compound,
		));

		// Make inflation happen
		roll_to_block(302);
		assert_eq!(Compute::current_cycle(), Cycle { epoch: 3, epoch_start: 302 });

		// Alice & Bob recommit
		{
			Compute::commit(
				&alice_account_id(),
				&alice_manager,
				&[(1u8, 1000u128, 1u128), (2u8, 2000u128, 1u128)],
			);
			Compute::commit(&bob_account_id(), &bob_manager, &[(2u8, 6000u128, 1u128)]);
		}

		// Make inflation happen
		roll_to_block(402);
		assert_eq!(Compute::current_cycle(), Cycle { epoch: 4, epoch_start: 402 });

		// Get Charlie's commitment ID
		let charlie_commitment_id =
			<Test as Config>::CommitmentIdProvider::commitment_id_for(&charlie).unwrap();

		// Test commission increase before burning - should fail because not overstaked
		assert_err!(
			Compute::stake_more(
				RuntimeOrigin::signed(charlie.clone()),
				charlie_commitment_id,
				None,
				None,                            // no commitment change
				Some(Perbill::from_percent(15)), // Try to increase commission from 10% to 15%
				None,
			),
			Error::<Test, ()>::CommissionCannotIncrease
		);

		// Burn minimum 1000 tokens from total supply
		let total_issuance_before_burn = Balances::total_issuance();

		// Burn from Charlie's account (he has most of the tokens)
		assert_ok!(Balances::force_set_balance(
			RuntimeOrigin::root(),
			charlie.clone(),
			stake_amount + 5 * UNIT
		));

		// Verify burn succeeded
		assert!(Balances::total_issuance() < total_issuance_before_burn);

		// Alice & Bob recommit (we need this to update target_weight_per_compute)
		{
			Compute::commit(
				&alice_account_id(),
				&alice_manager,
				&[(1u8, 1000u128, 1u128), (2u8, 2000u128, 1u128)],
			);
			Compute::commit(&bob_account_id(), &bob_manager, &[(2u8, 6000u128, 1u128)]);
		}

		// Make inflation happen
		roll_to_block(502);
		assert_eq!(Compute::current_cycle(), Cycle { epoch: 5, epoch_start: 502 });

		assert_err!(
			Compute::stake_more(
				RuntimeOrigin::signed(charlie.clone()),
				5 * UNIT,
				None,
				None,
				None,
				None,
			),
			Error::<Test, ()>::MaxStakeMetricRatioExceeded
		);
	});
}

#[test]
fn test_commit_compute_with_slash() {
	ExtBuilder.build().execute_with(|| {
		assert_ok!(Compute::enable_inflation(RuntimeOrigin::root()));
		setup_balances();
		create_pools();

		// Charlie will act as both manager and committer (same account for simplicity)
		let charlie = charlie_account_id();

		offer_accept_backing(charlie.clone());

		let charlie_manager =
			<Test as Config>::ManagerProviderForEligibleProcessor::lookup(&charlie).unwrap();

		// Charlie commits first time in warmup period (epoch 0)
		roll_to_block(10);
		assert_eq!(Compute::current_cycle(), Cycle { epoch: 0, epoch_start: 2 });
		assert_eq!(
			Compute::commit(&charlie, &charlie_manager, &[(2u8, 1000u128, 1u128)]).0,
			Zero::zero()
		);

		// Move to epoch 1 after warmup, Charlie commits again (now active)
		roll_to_block(150);
		assert_eq!(Compute::current_cycle(), Cycle { epoch: 1, epoch_start: 102 });

		// Charlie commits 4000 units for pool 2 as an active processor
		assert_eq!(
			Compute::commit(&charlie, &charlie_manager, &[(2u8, 4000u128, 1u128)]).0,
			Zero::zero()
		);

		// Now move to epoch 2 where Charlie can commit compute based on epoch 1 metrics
		roll_to_block(202);
		assert_eq!(Compute::current_cycle(), Cycle { epoch: 2, epoch_start: 202 });

		// Charlie can commit up to 80% of the previous epoch's metrics (4000 * 0.8 = 3200)
		let commitment: sp_runtime::BoundedVec<ComputeCommitment, sp_core::ConstU32<30>> =
			bounded_vec![ComputeCommitment {
				pool_id: 2,
				metric: FixedU128::from_rational(3200u128, 1u128), // Commit 3200 units (80% of 4000)
			},];

		let stake_amount = 10 * UNIT; // 10 tokens
		let cooldown_period = 36u64;
		let commission = Perbill::from_percent(10);
		let allow_auto_compound = true;

		// Charlie commits compute
		assert_ok!(Compute::commit_compute(
			RuntimeOrigin::signed(charlie.clone()),
			stake_amount,
			cooldown_period,
			commitment,
			commission,
			allow_auto_compound,
		));

		// Get Charlie's commitment ID
		let charlie_commitment_id =
			<Test as Config>::CommitmentIdProvider::commitment_id_for(&charlie).unwrap();

		// Verify initial stake
		let initial_commitment = Compute::commitments(charlie_commitment_id).unwrap();
		let initial_stake = initial_commitment.stake.as_ref().unwrap();
		assert_eq!(initial_stake.amount, stake_amount);

		// Move to next epoch
		roll_to_block(302);
		assert_eq!(Compute::current_cycle(), Cycle { epoch: 3, epoch_start: 302 });

		// Charlie delivers only 50% of committed metrics (1600 instead of 3200)
		Compute::commit(&charlie, &charlie_manager, &[(2u8, 1600u128, 1u128)]);

		// Move to next epoch to allow slashing
		roll_to_block(402);
		assert_eq!(Compute::current_cycle(), Cycle { epoch: 4, epoch_start: 402 });

		// Someone (alice) calls slash on Charlie for the missed metrics in epoch 3
		assert_ok!(Compute::slash(RuntimeOrigin::signed(alice_account_id()), charlie.clone()));

		// Verify Charlie's stake was decreased
		let slashed_commitment = Compute::commitments(charlie_commitment_id).unwrap();
		let slashed_stake = slashed_commitment.stake.as_ref().unwrap();

		// The stake should be less than the initial stake
		assert!(
			slashed_stake.amount < initial_stake.amount,
			"Stake should be decreased after slashing. Initial: {}, After slash: {}",
			initial_stake.amount,
			slashed_stake.amount
		);

		// Verify Slashed event was emitted
		assert!(events().iter().any(|e| matches!(e, RuntimeEvent::Compute(Event::Slashed(_)))));

		// Calculate expected slash amount
		// Charlie failed 50% of commitment in pool 2
		// Pool 2 has a reward ratio, and slash is based on BaseSlashAmount (1% of total stake)
		// with 50% unfulfilled ratio
		let pool = Compute::metric_pools(2).unwrap();
		let pool_reward_ratio = pool.reward.get(3); // epoch 3
		let total_stake = initial_stake.amount; // No delegations in this test
		let base_slash = Perquintill::from_percent(1).mul_floor(total_stake);
		let pool_slash = pool_reward_ratio.mul_floor(base_slash);
		let unfulfilled_ratio = Perquintill::from_percent(50); // 50% missed
		let expected_slash = unfulfilled_ratio.mul_floor(pool_slash);

		let actual_slash = initial_stake.amount - slashed_stake.amount;

		// The actual slash should match expected (allowing for small rounding differences)
		assert!(
			actual_slash >= expected_slash.saturating_sub(1)
				&& actual_slash <= expected_slash.saturating_add(1),
			"Actual slash {} should be close to expected slash {}",
			actual_slash,
			expected_slash
		);
	});
}

#[test]
fn test_delegate_undelegate() {
	ExtBuilder.build().execute_with(|| {
		assert_ok!(Compute::enable_inflation(RuntimeOrigin::root()));
		setup_balances();
		create_pools();

		// Charlie will act as both manager and committer (same account for simplicity)
		let committer = charlie_account_id();

		offer_accept_backing(committer.clone());

		commit_alice_bob();

		let commitment: sp_runtime::BoundedVec<ComputeCommitment, sp_core::ConstU32<30>> =
			bounded_vec![ComputeCommitment {
				pool_id: 2,
				metric: FixedU128::from_rational(4000u128 * 4 / 5, 1u128), // Maximal possible commitment value
			},];

		let stake_amount = 10 * UNIT; // 10 tokens
		let cooldown_period = 36u64; // 1000 blocks
		let commission = Perbill::from_percent(10); // 10% commission
		let allow_auto_compound = true;

		roll_to_block(202);
		assert_eq!(Compute::current_cycle(), Cycle { epoch: 2, epoch_start: 202 });

		let alice_manager =
			<Test as Config>::ManagerProviderForEligibleProcessor::lookup(&alice_account_id())
				.unwrap();
		let bob_manager =
			<Test as Config>::ManagerProviderForEligibleProcessor::lookup(&bob_account_id())
				.unwrap();

		// Alice & Bob recommit
		{
			Compute::commit(
				&alice_account_id(),
				&alice_manager,
				&[(1u8, 1000u128, 1u128), (2u8, 2000u128, 1u128)],
			);
			Compute::commit(&bob_account_id(), &bob_manager, &[(2u8, 6000u128, 1u128)]);
		}

		assert_ok!(Compute::commit_compute(
			RuntimeOrigin::signed(committer.clone()),
			stake_amount,
			cooldown_period,
			commitment,
			commission,
			allow_auto_compound,
		));

		let delegator_1 = ferdie_account_id();
		let delegator_2 = george_account_id();

		let stake_amount_1 = 25 * UNIT; // 5 tokens
		let stake_amount_2 = 5 * UNIT; // 5 tokens
		let cooldown_period = 36u64; // 1000 blocks
		let allow_auto_compound = true;

		{
			assert_ok!(Compute::delegate(
				RuntimeOrigin::signed(delegator_1.clone()),
				committer.clone(),
				stake_amount_1,
				cooldown_period,
				allow_auto_compound,
			));
			// After delegation, the stake should be locked
			assert_eq!(
				Balances::usable_balance(&delegator_1),
				1_000_000_000 * UNIT - stake_amount_1
			);
			// At minimum we should see the delegation event
			assert!(events()
				.iter()
				.any(|e| matches!(e, RuntimeEvent::Compute(Event::Delegated(_, _)))));
		}

		{
			assert_ok!(Compute::delegate(
				RuntimeOrigin::signed(delegator_2.clone()),
				committer.clone(),
				stake_amount_2,
				cooldown_period,
				allow_auto_compound,
			));
			// After delegation, the stake should be locked
			assert_eq!(
				Balances::usable_balance(&delegator_2),
				1_000_000_000 * UNIT - stake_amount_2
			);
			// At minimum we should see the delegation event
			assert!(events()
				.iter()
				.any(|e| matches!(e, RuntimeEvent::Compute(Event::Delegated(_, _)))));
		}

		assert_eq!(
			Compute::commitments(0).unwrap().delegations_total_amount,
			stake_amount_1 + stake_amount_2
		);
		assert_eq!(
			Compute::commitments(0)
				.unwrap()
				.weights
				.get_current()
				.1
				.delegations_reward_weight,
			U256::from(
				(stake_amount_1 + stake_amount_2) * (cooldown_period as u128) / 108u128 - 1u128
			) // 1 rounding error
		);

		// 75% filled, because 30 delegated vs 10 staked, 30/40
		assert_eq!(
			Compute::delegation_weight_ratio(
				Compute::current_cycle().epoch,
				&Compute::commitments(0).unwrap()
			)
			.unwrap(),
			Perquintill::from_percent(75)
		);

		// Make inflation happen
		roll_to_block(302);
		assert_eq!(Compute::current_cycle(), Cycle { epoch: 3, epoch_start: 302 });

		// Alice & Bob recommit
		{
			Compute::commit(
				&alice_account_id(),
				&alice_manager,
				&[(1u8, 1000u128, 1u128), (2u8, 2000u128, 1u128)],
			);
			Compute::commit(&bob_account_id(), &bob_manager, &[(2u8, 6000u128, 1u128)]);
		}

		// Make all rolling sums be complete (and inflation happens again)
		roll_to_block(402);
		assert_eq!(Compute::current_cycle(), Cycle { epoch: 4, epoch_start: 402 });

		// Alice & Bob recommit
		{
			Compute::commit(
				&alice_account_id(),
				&alice_manager,
				&[(1u8, 1000u128, 1u128), (2u8, 2000u128, 1u128)],
			);
			Compute::commit(&bob_account_id(), &bob_manager, &[(2u8, 6000u128, 1u128)]);
		}

		assert_ok!(Compute::withdraw_delegation(
			RuntimeOrigin::signed(delegator_2.clone()),
			committer.clone()
		));

		// assert_eq!(events(), []);
		assert!(events().iter().any(|e| matches!(
			e,
			RuntimeEvent::Compute(Event::DelegatorWithdrew(_, _, 337114726027296))
		)));

		assert_ok!(Compute::cooldown_delegation(
			RuntimeOrigin::signed(delegator_2.clone()),
			committer.clone()
		));

		assert_err!(
			Compute::kick_out(
				RuntimeOrigin::signed(ferdie_account_id()),
				delegator_2.clone(),
				committer.clone()
			),
			Error::<Test, ()>::CooldownNotEnded
		);

		// roll to block where delegator_2's cooldown is over
		roll_to_block(438);
		assert_eq!(Compute::current_cycle(), Cycle { epoch: 4, epoch_start: 402 });

		assert_err!(
			Compute::kick_out(
				RuntimeOrigin::signed(ferdie_account_id()),
				delegator_2.clone(),
				committer.clone()
			),
			Error::<Test, ()>::CannotKickout
		);

		// COMMITTER COOLDOWN
		assert_ok!(Compute::cooldown_compute_commitment(RuntimeOrigin::signed(committer.clone()),));

		assert_ok!(Compute::cooldown_delegation(
			RuntimeOrigin::signed(delegator_1.clone()),
			committer.clone()
		));

		// roll to block where committer's delegator_1's cooldown is over
		roll_to_block(474);
		assert_eq!(Compute::current_cycle(), Cycle { epoch: 4, epoch_start: 402 });

		// committer exits first!
		assert_ok!(Compute::end_compute_commitment(RuntimeOrigin::signed(committer.clone()),));

		assert_ok!(Compute::end_delegation(
			RuntimeOrigin::signed(delegator_1.clone()),
			committer.clone()
		));

		assert_ok!(Compute::kick_out(
			RuntimeOrigin::signed(ferdie_account_id()),
			delegator_2.clone(),
			committer.clone()
		));

		// Verify the reward was payed out
		// assert_eq!(events(), []);
		assert!(events().iter().any(|e| matches!(
			e,
			RuntimeEvent::Balances(pallet_balances::Event::Transfer {
				from: _,
				to: _,
				amount: 1685573630137087
			})
		)));
	});
}

/// Regression test for a double-transfer bug in `delegate_more`/`redelegate`/`kick_out`:
/// those functions call `end_delegation_for`, which already transfers the accrued reward to the
/// delegator internally, and previously transferred it a second time on top.
///
/// Two identical delegators accrue the exact same reward. One claims it through the known-correct
/// single-transfer path (`withdraw_delegation`), the other through `delegate_more`. The reward paid
/// out must be identical; with the double transfer, `delegate_more` would pay out twice as much.
#[test]
fn test_delegate_more_does_not_double_transfer_reward() {
	ExtBuilder.build().execute_with(|| {
		assert_ok!(Compute::enable_inflation(RuntimeOrigin::root()));
		setup_balances();
		create_pools();

		let committer = charlie_account_id();
		offer_accept_backing(committer.clone());
		commit_alice_bob();

		let commitment: sp_runtime::BoundedVec<ComputeCommitment, sp_core::ConstU32<30>> =
			bounded_vec![ComputeCommitment {
				pool_id: 2,
				metric: FixedU128::from_rational(4000u128 * 4 / 5, 1u128),
			},];

		let stake_amount = 10 * UNIT;
		let cooldown_period = 36u64;
		let commission = Perbill::from_percent(10);
		let allow_auto_compound = true;

		roll_to_block(202);
		assert_eq!(Compute::current_cycle(), Cycle { epoch: 2, epoch_start: 202 });

		let alice_manager =
			<Test as Config>::ManagerProviderForEligibleProcessor::lookup(&alice_account_id())
				.unwrap();
		let bob_manager =
			<Test as Config>::ManagerProviderForEligibleProcessor::lookup(&bob_account_id())
				.unwrap();

		// Alice & Bob recommit
		{
			Compute::commit(
				&alice_account_id(),
				&alice_manager,
				&[(1u8, 1000u128, 1u128), (2u8, 2000u128, 1u128)],
			);
			Compute::commit(&bob_account_id(), &bob_manager, &[(2u8, 6000u128, 1u128)]);
		}

		assert_ok!(Compute::commit_compute(
			RuntimeOrigin::signed(committer.clone()),
			stake_amount,
			cooldown_period,
			commitment,
			commission,
			allow_auto_compound,
		));

		// Two delegators with IDENTICAL stake, cooldown and timing => identical accrued reward.
		let delegator_1 = ferdie_account_id();
		let delegator_2 = george_account_id();
		let delegation_amount = 15 * UNIT;

		for delegator in [&delegator_1, &delegator_2] {
			assert_ok!(Compute::delegate(
				RuntimeOrigin::signed(delegator.clone()),
				committer.clone(),
				delegation_amount,
				cooldown_period,
				allow_auto_compound,
			));
		}

		// Accrue rewards across a couple of epochs.
		roll_to_block(302);
		{
			Compute::commit(
				&alice_account_id(),
				&alice_manager,
				&[(1u8, 1000u128, 1u128), (2u8, 2000u128, 1u128)],
			);
			Compute::commit(&bob_account_id(), &bob_manager, &[(2u8, 6000u128, 1u128)]);
		}
		roll_to_block(402);
		{
			Compute::commit(
				&alice_account_id(),
				&alice_manager,
				&[(1u8, 1000u128, 1u128), (2u8, 2000u128, 1u128)],
			);
			Compute::commit(&bob_account_id(), &bob_manager, &[(2u8, 6000u128, 1u128)]);
		}

		// delegator_1 claims via the known-correct single-transfer path.
		let d1_before = Balances::free_balance(&delegator_1);
		assert_ok!(Compute::withdraw_delegation(
			RuntimeOrigin::signed(delegator_1.clone()),
			committer.clone()
		));
		let reward_via_withdraw = Balances::free_balance(&delegator_1) - d1_before;
		assert!(reward_via_withdraw > 0, "test setup must accrue a non-zero reward");

		// delegator_2 claims the same accrued reward as a side effect of `delegate_more` (extra = 0).
		let d2_before = Balances::free_balance(&delegator_2);
		assert_ok!(Compute::delegate_more(
			RuntimeOrigin::signed(delegator_2.clone()),
			committer.clone(),
			0,
			None,
			None
		));
		let reward_via_delegate_more = Balances::free_balance(&delegator_2) - d2_before;

		// The reward must be paid exactly once, matching the withdraw path.
		assert_eq!(
			reward_via_delegate_more, reward_via_withdraw,
			"delegate_more must pay the accrued reward exactly once (double transfer regression)"
		);
	});
}

#[test]
fn test_delegate_more() {
	ExtBuilder.build().execute_with(|| {
		assert_ok!(Compute::enable_inflation(RuntimeOrigin::root()));
		setup_balances();
		create_pools();

		// Charlie will act as both manager and committer (same account for simplicity)
		let committer = charlie_account_id();

		offer_accept_backing(committer.clone());

		commit_alice_bob();

		let commitment: sp_runtime::BoundedVec<ComputeCommitment, sp_core::ConstU32<30>> =
			bounded_vec![ComputeCommitment {
				pool_id: 2,
				metric: FixedU128::from_rational(4000u128 * 4 / 5, 1u128), // Maximal possible commitment value
			},];

		let stake_amount = 10 * UNIT; // 10 tokens
		let cooldown_period = 36u64; // 1000 blocks
		let commission = Perbill::from_percent(10); // 10% commission
		let allow_auto_compound = true;

		roll_to_block(202);
		assert_eq!(Compute::current_cycle(), Cycle { epoch: 2, epoch_start: 202 });

		let alice_manager =
			<Test as Config>::ManagerProviderForEligibleProcessor::lookup(&alice_account_id())
				.unwrap();
		let bob_manager =
			<Test as Config>::ManagerProviderForEligibleProcessor::lookup(&bob_account_id())
				.unwrap();

		// Alice & Bob recommit
		{
			Compute::commit(
				&alice_account_id(),
				&alice_manager,
				&[(1u8, 1000u128, 1u128), (2u8, 2000u128, 1u128)],
			);
			Compute::commit(&bob_account_id(), &bob_manager, &[(2u8, 6000u128, 1u128)]);
		}

		assert_ok!(Compute::commit_compute(
			RuntimeOrigin::signed(committer.clone()),
			stake_amount,
			cooldown_period,
			commitment,
			commission,
			allow_auto_compound,
		));

		let delegator_1 = ferdie_account_id();
		let delegator_2 = george_account_id();
		let delegator_3 = henry_account_id();

		let stake_amount_1 = 25 * UNIT; // 5 tokens
		let stake_amount_2 = 5 * UNIT; // 5 tokens
		let cooldown_period = 36u64; // 1000 blocks
		let allow_auto_compound = true;

		{
			assert_ok!(Compute::delegate(
				RuntimeOrigin::signed(delegator_1.clone()),
				committer.clone(),
				stake_amount_1,
				cooldown_period,
				allow_auto_compound,
			));
			// After delegation, the stake should be locked
			assert_eq!(
				Balances::usable_balance(&delegator_1),
				1_000_000_000 * UNIT - stake_amount_1
			);
			// At minimum we should see the delegation event
			assert!(events()
				.iter()
				.any(|e| matches!(e, RuntimeEvent::Compute(Event::Delegated(_, _)))));
		}

		{
			assert_ok!(Compute::delegate(
				RuntimeOrigin::signed(delegator_2.clone()),
				committer.clone(),
				stake_amount_2,
				cooldown_period,
				allow_auto_compound,
			));
			// After delegation, the stake should be locked
			assert_eq!(
				Balances::usable_balance(&delegator_2),
				1_000_000_000 * UNIT - stake_amount_2
			);
			// At minimum we should see the delegation event
			assert!(events()
				.iter()
				.any(|e| matches!(e, RuntimeEvent::Compute(Event::Delegated(_, _)))));
		}

		assert_eq!(
			Compute::commitments(0).unwrap().delegations_total_amount,
			stake_amount_1 + stake_amount_2
		);
		assert_eq!(
			Compute::commitments(0)
				.unwrap()
				.weights
				.get_current()
				.1
				.delegations_reward_weight,
			U256::from(
				(stake_amount_1 + stake_amount_2) * (cooldown_period as u128) / 108u128 - 1u128
			) // 1 rounding error
		);

		// 75% filled, because 30 delegated vs 10 staked, 30/40
		assert_eq!(
			Compute::delegation_weight_ratio(
				Compute::current_cycle().epoch,
				&Compute::commitments(0).unwrap()
			)
			.unwrap(),
			Perquintill::from_percent(75)
		);

		let stake_amount_2b = 20 * UNIT; // makes it a total of 25 for delegator_2, and total delegations: 50
		{
			assert_ok!(Compute::delegate_more(
				RuntimeOrigin::signed(delegator_2.clone()),
				committer.clone(),
				stake_amount_2b,
				None,
				None
			));
			// After delegation, the stake should be locked
			let expected = 1_000_000_000 * UNIT - stake_amount_2 - stake_amount_2b;
			assert!(Balances::usable_balance(&delegator_2) - expected < UNIT);
			// At minimum we should see the delegation event
			assert!(events()
				.iter()
				.any(|e| matches!(e, RuntimeEvent::Compute(Event::DelegatedMore(_, _)))));
			assert_eq!(
				Compute::commitments(0)
					.unwrap()
					.weights
					.get_current()
					.1
					.delegations_reward_weight,
				U256::from(
					(stake_amount_1 + stake_amount_2 + stake_amount_2b) * (cooldown_period as u128)
						/ 108u128
				)
			);
		}

		{
			let stake_amount_3_exeeds_ratio = 41 * UNIT; // exceeds because (50 +40) /
			assert_err!(
				Compute::delegate(
					RuntimeOrigin::signed(delegator_3.clone()),
					committer.clone(),
					stake_amount_3_exeeds_ratio,
					cooldown_period,
					allow_auto_compound,
				),
				Error::<Test, ()>::MaxDelegationRatioExceeded
			);
		}
	});
}

fn offer_accept_backing(who: AccountId32) {
	const MANAGER_ID: u128 = 1;
	assert_ok!(<Test as crate::Config>::ManagerIdProvider::create_manager_id(MANAGER_ID, &who));

	// Set up the backing relationship using the correct commitment ID
	assert_ok!(Compute::offer_backing(RuntimeOrigin::signed(who.clone()), who.clone(),));
	assert_ok!(Compute::accept_backing_offer(RuntimeOrigin::signed(who.clone()), who.clone(),));

	const COMMITMENT_ID: u128 = 0;
	assert_eq!(
		<Test as crate::Config>::CommitmentIdProvider::commitment_id_for(&who)
			.expect("who should have a commitment ID"),
		COMMITMENT_ID
	);
}

// Helper kept for tests that exercise the commit path; nothing calls it at the moment.
#[allow(dead_code)]
fn commit_compute(who: AccountId32) {
	const MANAGER_ID: u128 = 1;

	// Check MetricsEpochSum instead of the old metrics_epoch_sum
	// pool 1 has only commits in warmup
	let epoch_sum_1 = Compute::metrics_epoch_sum(MANAGER_ID, 1);
	assert_eq!(
		epoch_sum_1,
		SlidingBuffer::from_inner(
			0,
			(Zero::zero(), Zero::zero()), // prev
			(Zero::zero(), Zero::zero()), // cur
		)
	);

	// pool 2 should have metrics
	let epoch_sum_2 = Compute::metrics_epoch_sum(MANAGER_ID, 2);
	assert_eq!(
		epoch_sum_2,
		SlidingBuffer::from_inner(
			0,
			(Zero::zero(), Zero::zero()), // prev
			(Zero::zero(), Zero::zero()), // cur
		)
	);

	// Step 3: Setup initial balance for Charlie to cover the stake amount
	assert_ok!(Balances::force_set_balance(RuntimeOrigin::root(), who.clone(), 100 * UNIT));

	// Step 4: Charlie commits compute (acting as committer backing his own manager account)
	let commitment: sp_runtime::BoundedVec<ComputeCommitment, sp_core::ConstU32<30>> =
		bounded_vec![ComputeCommitment {
			pool_id: 2,
			metric: FixedU128::from_rational(4000u128 * 4 / 5, 1u128), // Maximal possible commitment value
		},];

	let stake_amount = 5 * UNIT; // 5 tokens
	let cooldown_period = 36u64; // 1000 blocks
	let commission = Perbill::from_percent(10); // 10% commission
	let allow_auto_compound = true;

	roll_to_block(302);

	// Step 5: Charlie commits compute (as the committer)
	assert_ok!(Compute::commit_compute(
		RuntimeOrigin::signed(who.clone()),
		stake_amount,
		cooldown_period,
		commitment,
		commission,
		allow_auto_compound,
	));
}

#[test]
fn test_delegate_undelegate_after_slash() {
	ExtBuilder.build().execute_with(|| {
		assert_ok!(Compute::enable_inflation(RuntimeOrigin::root()));
		setup_balances();
		create_pools();

		// Charlie will act as both manager and committer (same account for simplicity)
		let committer = charlie_account_id();

		offer_accept_backing(committer.clone());

		let committer_manager =
			<Test as Config>::ManagerProviderForEligibleProcessor::lookup(&committer).unwrap();

		// Charlie commits first time in warmup period (epoch 0) - setting up as processor
		roll_to_block(10);
		assert_eq!(Compute::current_cycle(), Cycle { epoch: 0, epoch_start: 2 });
		assert_eq!(
			Compute::commit(&committer, &committer_manager, &[(2u8, 1000u128, 1u128)]).0,
			Zero::zero()
		);

		// Move to epoch 1 after warmup, Charlie commits again (now active)
		roll_to_block(150);
		assert_eq!(Compute::current_cycle(), Cycle { epoch: 1, epoch_start: 102 });

		// Charlie commits 4000 units for pool 2 as an active processor
		assert_eq!(
			Compute::commit(&committer, &committer_manager, &[(2u8, 4000u128, 1u128)]).0,
			Zero::zero()
		);

		// Now move to epoch 2 where Charlie can commit compute based on epoch 1 metrics
		roll_to_block(202);
		assert_eq!(Compute::current_cycle(), Cycle { epoch: 2, epoch_start: 202 });

		// Charlie can commit up to 80% of the previous epoch's metrics (4000 * 0.8 = 3200)
		let commitment: sp_runtime::BoundedVec<ComputeCommitment, sp_core::ConstU32<30>> =
			bounded_vec![ComputeCommitment {
				pool_id: 2,
				metric: FixedU128::from_rational(3200u128, 1u128), // Commit 3200 units (80% of 4000)
			},];

		let stake_amount = 10 * UNIT; // 10 tokens
		let cooldown_period = 36u64;
		let commission = Perbill::from_percent(10); // 10% commission
		let allow_auto_compound = true;

		// Committer commits compute
		assert_ok!(Compute::commit_compute(
			RuntimeOrigin::signed(committer.clone()),
			stake_amount,
			cooldown_period,
			commitment,
			commission,
			allow_auto_compound,
		));

		// Get committer's commitment ID
		let committer_commitment_id =
			<Test as Config>::CommitmentIdProvider::commitment_id_for(&committer).unwrap();

		let delegator = ferdie_account_id();
		let delegate_stake_amount = 25 * UNIT;
		let delegate_cooldown_period = 36u64;

		// Delegator delegates to committer
		assert_ok!(Compute::delegate(
			RuntimeOrigin::signed(delegator.clone()),
			committer.clone(),
			delegate_stake_amount,
			delegate_cooldown_period,
			allow_auto_compound,
		));

		// Verify delegation happened
		assert!(events()
			.iter()
			.any(|e| matches!(e, RuntimeEvent::Compute(Event::Delegated(_, _)))));

		// Move to next epoch
		roll_to_block(302);
		assert_eq!(Compute::current_cycle(), Cycle { epoch: 3, epoch_start: 302 });

		// Committer delivers only 50% of committed metrics (1600 instead of 3200) to trigger slash
		Compute::commit(&committer, &committer_manager, &[(2u8, 1600u128, 1u128)]);

		// Move to next epoch to allow slashing
		roll_to_block(402);
		assert_eq!(Compute::current_cycle(), Cycle { epoch: 4, epoch_start: 402 });

		// Verify initial stake before slashing
		let initial_commitment = Compute::commitments(committer_commitment_id).unwrap();
		let initial_stake = initial_commitment.stake.as_ref().unwrap();

		// Someone (alice) calls slash on the committer for the missed metrics
		assert_ok!(Compute::slash(RuntimeOrigin::signed(alice_account_id()), committer.clone()));

		// Verify committer's stake was decreased
		let slashed_commitment = Compute::commitments(committer_commitment_id).unwrap();
		let slashed_stake = slashed_commitment.stake.as_ref().unwrap();
		assert!(
			slashed_stake.amount < initial_stake.amount,
			"Stake should be decreased after slashing. Initial: {}, After slash: {}",
			initial_stake.amount,
			slashed_stake.amount
		);

		assert!(
			slashed_commitment.pool_rewards.get_current().1.slash_per_weight > Zero::zero(),
			"Delegation pool's slash_per_weight should be non-zero but is",
		);

		// Verify Slashed event was emitted
		assert!(events().iter().any(|e| matches!(e, RuntimeEvent::Compute(Event::Slashed(_)))));

		// Committer cooldowns
		assert_ok!(Compute::cooldown_compute_commitment(RuntimeOrigin::signed(committer.clone()),));

		// Delegator cooldowns (must be done before committer ends commitment)
		assert_ok!(Compute::cooldown_delegation(
			RuntimeOrigin::signed(delegator.clone()),
			committer.clone()
		));

		// roll to block where both committer's and delegator's cooldowns are over
		roll_to_block(474);

		// Committer ends commitment first
		assert_ok!(Compute::end_compute_commitment(RuntimeOrigin::signed(committer.clone()),));

		// Delegator tries to end delegation (this is supposed to fail because of bug)
		assert_ok!(Compute::end_delegation(
			RuntimeOrigin::signed(delegator.clone()),
			committer.clone()
		));
	});
}

#[test]
fn test_kick_out_stale_delegation_after_slash_and_commitment_ended() {
	ExtBuilder.build().execute_with(|| {
		assert_ok!(Compute::enable_inflation(RuntimeOrigin::root()));
		setup_balances();
		create_pools();

		// Charlie will act as both manager and committer (same account for simplicity)
		let committer = charlie_account_id();

		offer_accept_backing(committer.clone());

		let committer_manager =
			<Test as Config>::ManagerProviderForEligibleProcessor::lookup(&committer).unwrap();

		// Charlie commits first time in warmup period (epoch 0) - setting up as processor
		roll_to_block(10);
		assert_eq!(Compute::current_cycle(), Cycle { epoch: 0, epoch_start: 2 });
		assert_eq!(
			Compute::commit(&committer, &committer_manager, &[(2u8, 1000u128, 1u128)]).0,
			Zero::zero()
		);

		// Move to epoch 1 after warmup, Charlie commits again (now active)
		roll_to_block(150);
		assert_eq!(Compute::current_cycle(), Cycle { epoch: 1, epoch_start: 102 });

		// Charlie commits 4000 units for pool 2 as an active processor
		assert_eq!(
			Compute::commit(&committer, &committer_manager, &[(2u8, 4000u128, 1u128)]).0,
			Zero::zero()
		);

		// Now move to epoch 2 where Charlie can commit compute based on epoch 1 metrics
		roll_to_block(202);
		assert_eq!(Compute::current_cycle(), Cycle { epoch: 2, epoch_start: 202 });

		// Charlie can commit up to 80% of the previous epoch's metrics (4000 * 0.8 = 3200)
		let commitment: sp_runtime::BoundedVec<ComputeCommitment, sp_core::ConstU32<30>> =
			bounded_vec![ComputeCommitment {
				pool_id: 2,
				metric: FixedU128::from_rational(3200u128, 1u128), // Commit 3200 units (80% of 4000)
			},];

		let stake_amount = 10 * UNIT; // 10 tokens
		let cooldown_period = 36u64;
		let commission = Perbill::from_percent(10); // 10% commission
		let allow_auto_compound = true;

		// Committer commits compute
		assert_ok!(Compute::commit_compute(
			RuntimeOrigin::signed(committer.clone()),
			stake_amount,
			cooldown_period,
			commitment,
			commission,
			allow_auto_compound,
		));

		// Get committer's commitment ID
		let committer_commitment_id =
			<Test as Config>::CommitmentIdProvider::commitment_id_for(&committer).unwrap();

		let delegator = ferdie_account_id();
		let delegate_stake_amount = 25 * UNIT;
		let delegate_cooldown_period = 36u64;

		// Delegator delegates to committer
		assert_ok!(Compute::delegate(
			RuntimeOrigin::signed(delegator.clone()),
			committer.clone(),
			delegate_stake_amount,
			delegate_cooldown_period,
			allow_auto_compound,
		));

		// Verify delegation happened
		assert!(events()
			.iter()
			.any(|e| matches!(e, RuntimeEvent::Compute(Event::Delegated(_, _)))));

		// Move to next epoch
		roll_to_block(302);
		assert_eq!(Compute::current_cycle(), Cycle { epoch: 3, epoch_start: 302 });

		// Committer delivers only 50% of committed metrics (1600 instead of 3200) to trigger slash
		Compute::commit(&committer, &committer_manager, &[(2u8, 1600u128, 1u128)]);

		// Move to next epoch to allow slashing
		roll_to_block(402);
		assert_eq!(Compute::current_cycle(), Cycle { epoch: 4, epoch_start: 402 });

		// Verify initial stake before slashing
		let initial_commitment = Compute::commitments(committer_commitment_id).unwrap();
		let initial_stake = initial_commitment.stake.as_ref().unwrap();

		// Someone (alice) calls slash on the committer for the missed metrics
		assert_ok!(Compute::slash(RuntimeOrigin::signed(alice_account_id()), committer.clone()));

		// Verify committer's stake was decreased
		let slashed_commitment = Compute::commitments(committer_commitment_id).unwrap();
		let slashed_stake = slashed_commitment.stake.as_ref().unwrap();
		assert!(
			slashed_stake.amount < initial_stake.amount,
			"Stake should be decreased after slashing. Initial: {}, After slash: {}",
			initial_stake.amount,
			slashed_stake.amount
		);

		assert!(
			slashed_commitment.pool_rewards.get_current().1.slash_per_weight > Zero::zero(),
			"Delegation pool's slash_per_weight should be non-zero",
		);

		// Verify Slashed event was emitted
		assert!(events().iter().any(|e| matches!(e, RuntimeEvent::Compute(Event::Slashed(_)))));

		// Committer cooldowns FIRST (before delegator)
		assert_ok!(Compute::cooldown_compute_commitment(RuntimeOrigin::signed(committer.clone()),));

		// Roll to block where committer's cooldown is over
		roll_to_block(474);

		// Committer ends commitment FIRST - this leaves delegator with a "stale" delegation
		// because the delegation's created timestamp is from before the commitment ended
		assert_ok!(Compute::end_compute_commitment(RuntimeOrigin::signed(committer.clone()),));

		// Verify commitment stake is now None
		let ended_commitment = Compute::commitments(committer_commitment_id).unwrap();
		assert!(ended_commitment.stake.is_none(), "Commitment stake should be None after ending");

		// Verify delegator's delegation still exists (stale delegation)
		let stale_delegation = Compute::delegations(&delegator, committer_commitment_id);
		assert!(stale_delegation.is_some(), "Delegator should still have a stale delegation");

		// Get delegator's stale delegation to check the accrued slash
		let stale_delegation_info = stale_delegation.unwrap();
		let delegation_amount = stale_delegation_info.stake.amount;

		// Record delegator's balance before kick_out
		// Note: stake is locked, not transferred, so free balance includes the locked amount
		let delegator_balance_before = Balances::free_balance(&delegator);

		// Anyone can now kick out the delegator since their delegation is stale
		// (the commitment they delegated to has ended)
		assert_ok!(Compute::kick_out(
			RuntimeOrigin::signed(alice_account_id()),
			delegator.clone(),
			committer.clone()
		));

		// Verify KickedOut event was emitted
		assert!(events()
			.iter()
			.any(|e| matches!(e, RuntimeEvent::Compute(Event::KickedOut(_, _, _)))));

		// Verify delegator's delegation is now gone
		let delegation_after = Compute::delegations(&delegator, committer_commitment_id);
		assert!(delegation_after.is_none(), "Delegation should be removed after kick_out");

		// Verify delegator's balance changed due to kick_out
		// The slash is applied by burning tokens, so balance should decrease
		let delegator_balance_after = Balances::free_balance(&delegator);

		// The delegator should have been slashed, so their final balance should be less
		// than before the slash was applied (some tokens were burned)
		let slashed_amount = delegator_balance_before.saturating_sub(delegator_balance_after);
		assert!(
			slashed_amount > Zero::zero(),
			"Delegator should have been slashed. Balance before: {}, after: {}, slashed: {}",
			delegator_balance_before,
			delegator_balance_after,
			slashed_amount
		);

		// Verify the slashed amount is reasonable (less than total stake)
		assert!(
			slashed_amount < delegation_amount,
			"Slashed amount should be less than total delegation. Delegation: {}, Slashed: {}",
			delegation_amount,
			slashed_amount
		);
	});
}

/// This test reproduces the bug where pool_rewards history is lost after two re-stakes.
/// The MemoryBuffer only keeps one "past" slot, so after two re-stakes, the original
/// pool_rewards values are lost, and the delegation reads zeros instead.
///
/// Bug scenario:
/// 1. Commitment 1 created, delegation created, slash occurs
/// 2. Commitment 1 ends (pool_rewards: current=(t1, values), past=None)
/// 3. Commitment 2 created (pool_rewards: current=(t2, zeros), past=(t1, values))
/// 4. Delegator claims - works because get_latest(delegation.created) reads past
/// 5. Commitment 2 ends
/// 6. Commitment 3 created (pool_rewards: current=(t3, zeros), past=(t2, zeros))
/// 7. Delegator tries to claim - FAILS because get_latest(delegation.created) returns zeros
///    but slash_debt was set with non-zero values
#[test]
fn test_delegator_claim_after_two_commitment_restakes() {
	ExtBuilder.build().execute_with(|| {
		assert_ok!(Compute::enable_inflation(RuntimeOrigin::root()));
		setup_balances();
		create_pools();

		let committer = charlie_account_id();
		offer_accept_backing(committer.clone());

		let committer_manager =
			<Test as Config>::ManagerProviderForEligibleProcessor::lookup(&committer).unwrap();

		// === COMMITMENT 1 ===
		roll_to_block(10);
		Compute::commit(&committer, &committer_manager, &[(2u8, 1000u128, 1u128)]);

		roll_to_block(150);
		Compute::commit(&committer, &committer_manager, &[(2u8, 4000u128, 1u128)]);

		roll_to_block(202);

		let commitment: sp_runtime::BoundedVec<ComputeCommitment, sp_core::ConstU32<30>> =
			bounded_vec![ComputeCommitment {
				pool_id: 2,
				metric: FixedU128::from_rational(3200u128, 1u128),
			},];

		let stake_amount = 10 * UNIT;
		let cooldown_period = 36u64;
		let commission = Perbill::from_percent(10);

		assert_ok!(Compute::commit_compute(
			RuntimeOrigin::signed(committer.clone()),
			stake_amount,
			cooldown_period,
			commitment.clone(),
			commission,
			true,
		));

		let commitment_id =
			<Test as Config>::CommitmentIdProvider::commitment_id_for(&committer).unwrap();

		// Delegator delegates
		let delegator = ferdie_account_id();
		assert_ok!(Compute::delegate(
			RuntimeOrigin::signed(delegator.clone()),
			committer.clone(),
			25 * UNIT,
			cooldown_period,
			true,
		));

		let delegation_created = System::block_number();
		println!("Delegation created at block: {}", delegation_created);

		// Trigger slash to build up slash_per_weight
		roll_to_block(302);
		Compute::commit(&committer, &committer_manager, &[(2u8, 1600u128, 1u128)]);

		roll_to_block(402);
		assert_ok!(Compute::slash(RuntimeOrigin::signed(alice_account_id()), committer.clone()));

		// Record the slash_per_weight
		let slash_per_weight_after_slash = Compute::commitments(commitment_id)
			.unwrap()
			.pool_rewards
			.get_current()
			.1
			.slash_per_weight;
		println!("Slash per weight after first slash: {}", slash_per_weight_after_slash);

		// Delegator withdraws - this sets slash_debt based on current slash_per_weight
		assert_ok!(Compute::withdraw_delegation(
			RuntimeOrigin::signed(delegator.clone()),
			committer.clone()
		));

		let delegation_after_withdraw1 = Compute::delegations(&delegator, commitment_id).unwrap();
		println!("After withdraw 1 - slash_debt: {}", delegation_after_withdraw1.slash_debt);

		// === END COMMITMENT 1 ===
		assert_ok!(Compute::cooldown_compute_commitment(RuntimeOrigin::signed(committer.clone())));
		roll_to_block(438);
		assert_ok!(Compute::end_compute_commitment(RuntimeOrigin::signed(committer.clone())));
		println!("Commitment 1 ended at block: {}", System::block_number());

		// === COMMITMENT 2 ===
		roll_to_block(502);
		Compute::commit(&committer, &committer_manager, &[(2u8, 4000u128, 1u128)]);
		roll_to_block(602);
		Compute::commit(&committer, &committer_manager, &[(2u8, 4000u128, 1u128)]);
		roll_to_block(702);
		Compute::commit(&committer, &committer_manager, &[(2u8, 4000u128, 1u128)]);
		roll_to_block(802);

		assert_ok!(Compute::commit_compute(
			RuntimeOrigin::signed(committer.clone()),
			stake_amount,
			cooldown_period,
			commitment.clone(),
			commission,
			true,
		));

		let commitment2_created =
			Compute::commitments(commitment_id).unwrap().stake.as_ref().unwrap().created;
		println!("Commitment 2 created at block: {}", commitment2_created);

		// Check pool_rewards after first re-stake
		let pool_rewards_2 = Compute::commitments(commitment_id).unwrap().pool_rewards;
		println!(
			"After commitment 2 - pool_rewards.past: {:?}, pool_rewards.current: {:?}",
			pool_rewards_2.get_past(),
			pool_rewards_2.get_current()
		);

		// === END COMMITMENT 2 ===
		assert_ok!(Compute::cooldown_compute_commitment(RuntimeOrigin::signed(committer.clone())));
		roll_to_block(838);
		assert_ok!(Compute::end_compute_commitment(RuntimeOrigin::signed(committer.clone())));
		println!("Commitment 2 ended at block: {}", System::block_number());

		// === COMMITMENT 3 ===
		roll_to_block(902);
		Compute::commit(&committer, &committer_manager, &[(2u8, 4000u128, 1u128)]);
		roll_to_block(1002);
		Compute::commit(&committer, &committer_manager, &[(2u8, 4000u128, 1u128)]);
		roll_to_block(1102);
		Compute::commit(&committer, &committer_manager, &[(2u8, 4000u128, 1u128)]);
		roll_to_block(1202);

		assert_ok!(Compute::commit_compute(
			RuntimeOrigin::signed(committer.clone()),
			stake_amount,
			cooldown_period,
			commitment.clone(),
			commission,
			true,
		));

		let commitment3_created =
			Compute::commitments(commitment_id).unwrap().stake.as_ref().unwrap().created;
		println!("Commitment 3 created at block: {}", commitment3_created);

		// Check pool_rewards after second re-stake - past should now be zeros!
		let pool_rewards_3 = Compute::commitments(commitment_id).unwrap().pool_rewards;
		println!(
			"After commitment 3 - pool_rewards.past: {:?}, pool_rewards.current: {:?}",
			pool_rewards_3.get_past(),
			pool_rewards_3.get_current()
		);

		// Now delegator tries to withdraw
		// get_latest(delegation_created=202) should return zeros because:
		// - current = (1202, zeros)
		// - past = (802, zeros)
		// - 202 < 802, so returns Default (zeros)
		let delegation_before_final_withdraw =
			Compute::delegations(&delegator, commitment_id).unwrap();
		println!(
			"Before final withdraw - slash_weight: {}, slash_debt: {}",
			delegation_before_final_withdraw.slash_weight,
			delegation_before_final_withdraw.slash_debt
		);

		let what_delegation_reads = pool_rewards_3.get_latest(delegation_created);
		println!(
			"What delegation reads from get_latest({}): {:?}",
			delegation_created, what_delegation_reads
		);

		// THIS IS THE CRITICAL MOMENT
		// Previously: If slash_debt > 0 but get_latest returns 0, then:
		// slash = slash_weight * 0 / DECIMALS = 0
		// slash - slash_debt = 0 - slash_debt = NEGATIVE -> underflow!
		//
		// FIX: accrue_delegator now uses try_get_latest() and skips accrual entirely
		// when pool_rewards history is unavailable for the delegation's creation time.
		let result = Compute::withdraw_delegation(
			RuntimeOrigin::signed(delegator.clone()),
			committer.clone(),
		);

		println!("Final withdraw result: {:?}", result);

		// After the fix: even when slash_debt > 0 but pool_rewards history is lost,
		// accrue_delegator skips accrual instead of erroring out
		println!("EDGE CASE: slash_debt > 0 but get_latest returns 0 - accrual is skipped");
		// The fix ensures this no longer causes an error
		assert!(
			result.is_ok(),
			"Should succeed - accrual is skipped when pool_rewards history unavailable"
		);
	});
}

/// Test that rewardable_amount is corrected (sanitized) upon withdraw_delegation.
///
/// This test force-inserts a delegation with rewardable_amount > amount (the bug state)
/// and verifies that calling withdraw_delegation corrects the invariant.
#[test]
fn test_rewardable_amount_sanitized_on_withdraw() {
	use crate::datastructures::MemoryBuffer;
	use crate::types::{Commitment, CommitmentWeights, Delegation, PoolReward, Stake};
	use crate::{Commitments, CurrentCycle, Delegations};
	use frame_support::traits::Currency;

	ExtBuilder.build().execute_with(|| {
		let delegator = ferdie_account_id();
		let committer = charlie_account_id();

		// Set up cycle and block
		CurrentCycle::<Test, ()>::put(Cycle { epoch: 10, epoch_start: 1000 });
		System::set_block_number(1000);

		// Fund accounts
		let distribution_account = Compute::account_id();
		let _ = Balances::deposit_creating(&distribution_account, 1_000_000 * UNIT);
		let _ = Balances::deposit_creating(&delegator, 1_000_000 * UNIT);

		// Create commitment
		offer_accept_backing(committer.clone());
		let commitment_id =
			<Test as Config>::CommitmentIdProvider::commitment_id_for(&committer).unwrap();

		// Force insert commitment with pool_rewards
		let pool_rewards: MemoryBuffer<u64, PoolReward> = MemoryBuffer::new_with(
			5u64,
			PoolReward {
				reward_per_weight: U256::from(1_000_000_000_000_000_000u128),
				slash_per_weight: U256::zero(),
			},
		);
		let weights: MemoryBuffer<u64, CommitmentWeights> = MemoryBuffer::new_with(
			10u64,
			CommitmentWeights {
				self_reward_weight: U256::from(100 * UNIT),
				self_slash_weight: U256::from(100 * UNIT),
				delegations_reward_weight: U256::from(100 * UNIT),
				delegations_slash_weight: U256::from(100 * UNIT),
			},
		);
		let commitment = Commitment {
			stake: Some(Stake {
				amount: 100 * UNIT,
				rewardable_amount: 100 * UNIT,
				created: 5u64,
				cooldown_period: 100u64,
				cooldown_started: None,
				accrued_reward: 0,
				accrued_slash: 0,
				allow_auto_compound: true,
				paid: 0,
				applied_slash: 0,
			}),
			commission: Perbill::from_percent(10),
			delegations_total_amount: 100 * UNIT,
			delegations_total_rewardable_amount: 120 * UNIT, // Intentionally higher (buggy state)
			weights,
			pool_rewards,
			last_scoring_epoch: 10,
			last_slashing_epoch: 0,
		};
		Commitments::<Test, ()>::insert(commitment_id, commitment);

		// Force insert delegation with rewardable_amount > amount (the bug condition)
		let delegation = Delegation {
			stake: Stake {
				amount: 100 * UNIT,
				rewardable_amount: 120 * UNIT, // BUG: rewardable_amount > amount
				created: 5u64,
				cooldown_period: 100u64,
				cooldown_started: None,
				accrued_reward: 0,
				accrued_slash: 0,
				allow_auto_compound: true,
				paid: 0,
				applied_slash: 0,
			},
			reward_weight: U256::from(120 * UNIT), // Based on buggy rewardable_amount
			slash_weight: U256::from(100 * UNIT),
			reward_debt: 0,
			slash_debt: 0,
		};
		Delegations::<Test, ()>::insert(&delegator, commitment_id, delegation);

		// Verify the buggy state
		let delegation_before = Compute::delegations(&delegator, commitment_id).unwrap();
		println!("=== Before withdraw (buggy state) ===");
		println!("  amount: {}", delegation_before.stake.amount);
		println!("  rewardable_amount: {}", delegation_before.stake.rewardable_amount);
		println!("  reward_weight: {}", delegation_before.reward_weight);
		assert!(
			delegation_before.stake.rewardable_amount > delegation_before.stake.amount,
			"Pre-condition: rewardable_amount should be > amount (buggy state)"
		);

		// Call withdraw_delegation - this triggers accrue_delegator which calls sanitize_delegation
		let result = Compute::withdraw_delegation(
			RuntimeOrigin::signed(delegator.clone()),
			committer.clone(),
		);
		println!("\nWithdraw result: {:?}", result);
		assert!(result.is_ok());

		// Verify the sanitized state
		let delegation_after = Compute::delegations(&delegator, commitment_id).unwrap();
		println!("\n=== After withdraw (sanitized) ===");
		println!("  amount: {}", delegation_after.stake.amount);
		println!("  rewardable_amount: {}", delegation_after.stake.rewardable_amount);
		println!("  reward_weight: {}", delegation_after.reward_weight);

		// The fix should have corrected rewardable_amount
		assert!(
			delegation_after.stake.rewardable_amount <= delegation_after.stake.amount,
			"FIXED: rewardable_amount ({}) should be <= amount ({}) after sanitization",
			delegation_after.stake.rewardable_amount,
			delegation_after.stake.amount
		);

		// Verify rewardable_amount was corrected to equal amount (since not in cooldown)
		assert_eq!(
			delegation_after.stake.rewardable_amount, delegation_after.stake.amount,
			"rewardable_amount should equal amount when not in cooldown"
		);

		println!(
			"\n✓ Sanitization successful: rewardable_amount corrected from {} to {}",
			120 * UNIT,
			delegation_after.stake.rewardable_amount
		);
	});
}

/// Test that rewardable_amount is corrected (sanitized) upon delegate_more.
///
/// This test force-inserts a delegation with rewardable_amount > amount (the bug state)
/// and verifies that calling delegate_more corrects the invariant.
#[test]
fn test_rewardable_amount_sanitized_on_delegate_more() {
	use crate::datastructures::MemoryBuffer;
	use crate::types::{Commitment, CommitmentWeights, Delegation, PoolReward, Stake};
	use crate::{Commitments, CurrentCycle, Delegations, DelegatorTotal, TotalStake};
	use frame_support::traits::Currency;

	ExtBuilder.build().execute_with(|| {
		let delegator = ferdie_account_id();
		let committer = charlie_account_id();

		// Set up cycle and block
		CurrentCycle::<Test, ()>::put(Cycle { epoch: 10, epoch_start: 1000 });
		System::set_block_number(1000);

		// Fund accounts
		let distribution_account = Compute::account_id();
		let _ = Balances::deposit_creating(&distribution_account, 1_000_000 * UNIT);
		let _ = Balances::deposit_creating(&delegator, 1_000_000 * UNIT);

		// Create commitment
		offer_accept_backing(committer.clone());
		let commitment_id =
			<Test as Config>::CommitmentIdProvider::commitment_id_for(&committer).unwrap();

		// Force insert commitment with pool_rewards
		let pool_rewards: MemoryBuffer<u64, PoolReward> = MemoryBuffer::new_with(
			5u64,
			PoolReward {
				reward_per_weight: U256::from(1_000_000_000_000_000_000u128),
				slash_per_weight: U256::zero(),
			},
		);
		let weights: MemoryBuffer<u64, CommitmentWeights> = MemoryBuffer::new_with(
			10u64,
			CommitmentWeights {
				self_reward_weight: U256::from(100 * UNIT),
				self_slash_weight: U256::from(100 * UNIT),
				delegations_reward_weight: U256::from(100 * UNIT),
				delegations_slash_weight: U256::from(100 * UNIT),
			},
		);
		let commitment = Commitment {
			stake: Some(Stake {
				amount: 100 * UNIT,
				rewardable_amount: 100 * UNIT,
				created: 5u64,
				cooldown_period: 100u64,
				cooldown_started: None,
				accrued_reward: 0,
				accrued_slash: 0,
				allow_auto_compound: true,
				paid: 0,
				applied_slash: 0,
			}),
			commission: Perbill::from_percent(10),
			delegations_total_amount: 100 * UNIT,
			delegations_total_rewardable_amount: 120 * UNIT, // Intentionally higher (buggy state)
			weights,
			pool_rewards,
			last_scoring_epoch: 10,
			last_slashing_epoch: 0,
		};
		Commitments::<Test, ()>::insert(commitment_id, commitment);

		// Force insert delegation with rewardable_amount > amount (the bug condition)
		let delegation = Delegation {
			stake: Stake {
				amount: 100 * UNIT,
				rewardable_amount: 120 * UNIT, // BUG: rewardable_amount > amount
				created: 5u64,
				cooldown_period: 100u64,
				cooldown_started: None,
				accrued_reward: 0,
				accrued_slash: 0,
				allow_auto_compound: true,
				paid: 0,
				applied_slash: 0,
			},
			reward_weight: U256::from(120 * UNIT), // Based on buggy rewardable_amount
			slash_weight: U256::from(100 * UNIT),
			reward_debt: 0,
			slash_debt: 0,
		};
		Delegations::<Test, ()>::insert(&delegator, commitment_id, delegation);
		DelegatorTotal::<Test, ()>::insert(&delegator, 100 * UNIT);
		// Set TotalStake to include the delegation amount (needed for end_delegation_for)
		TotalStake::<Test, ()>::put(100 * UNIT);

		// Verify the buggy state
		let delegation_before = Compute::delegations(&delegator, commitment_id).unwrap();
		println!("=== Before delegate_more (buggy state) ===");
		println!("  amount: {}", delegation_before.stake.amount);
		println!("  rewardable_amount: {}", delegation_before.stake.rewardable_amount);
		println!("  reward_weight: {}", delegation_before.reward_weight);
		assert!(
			delegation_before.stake.rewardable_amount > delegation_before.stake.amount,
			"Pre-condition: rewardable_amount should be > amount (buggy state)"
		);

		// Call delegate_more - this triggers accrue_delegator which calls sanitize_delegation
		let extra_amount = 10 * UNIT;
		let result = Compute::delegate_more(
			RuntimeOrigin::signed(delegator.clone()),
			committer.clone(),
			extra_amount,
			None,
			None,
		);
		println!("\nDelegate_more result: {:?}", result);
		assert!(result.is_ok());

		// Verify the sanitized state (and increased amount)
		let delegation_after = Compute::delegations(&delegator, commitment_id).unwrap();
		println!("\n=== After delegate_more (sanitized) ===");
		println!("  amount: {}", delegation_after.stake.amount);
		println!("  rewardable_amount: {}", delegation_after.stake.rewardable_amount);
		println!("  reward_weight: {}", delegation_after.reward_weight);

		// The fix should have corrected rewardable_amount
		assert!(
			delegation_after.stake.rewardable_amount <= delegation_after.stake.amount,
			"FIXED: rewardable_amount ({}) should be <= amount ({}) after sanitization",
			delegation_after.stake.rewardable_amount,
			delegation_after.stake.amount
		);

		// Amount should have increased by extra_amount
		assert_eq!(
			delegation_after.stake.amount,
			100 * UNIT + extra_amount,
			"amount should have increased by extra_amount"
		);

		println!(
			"\n✓ Sanitization successful on delegate_more: rewardable_amount corrected and amount increased to {}",
			delegation_after.stake.amount
		);
	});
}

/// Test that rewardable_amount is corrected (sanitized) upon cooldown_delegation.
///
/// This test force-inserts a delegation with rewardable_amount > amount (the bug state)
/// and verifies that calling cooldown_delegation corrects the invariant.
#[test]
fn test_rewardable_amount_sanitized_on_cooldown() {
	use crate::datastructures::MemoryBuffer;
	use crate::types::{Commitment, CommitmentWeights, Delegation, PoolReward, Stake};
	use crate::{Commitments, CurrentCycle, Delegations, DelegatorTotal};
	use frame_support::traits::Currency;

	ExtBuilder.build().execute_with(|| {
		let delegator = ferdie_account_id();
		let committer = charlie_account_id();

		// Set up cycle and block
		CurrentCycle::<Test, ()>::put(Cycle { epoch: 10, epoch_start: 1000 });
		System::set_block_number(1000);

		// Fund accounts
		let distribution_account = Compute::account_id();
		let _ = Balances::deposit_creating(&distribution_account, 1_000_000 * UNIT);
		let _ = Balances::deposit_creating(&delegator, 1_000_000 * UNIT);

		// Create commitment
		offer_accept_backing(committer.clone());
		let commitment_id =
			<Test as Config>::CommitmentIdProvider::commitment_id_for(&committer).unwrap();

		// Force insert commitment with pool_rewards
		let pool_rewards: MemoryBuffer<u64, PoolReward> = MemoryBuffer::new_with(
			5u64,
			PoolReward {
				reward_per_weight: U256::from(1_000_000_000_000_000_000u128),
				slash_per_weight: U256::zero(),
			},
		);
		let weights: MemoryBuffer<u64, CommitmentWeights> = MemoryBuffer::new_with(
			10u64,
			CommitmentWeights {
				self_reward_weight: U256::from(100 * UNIT),
				self_slash_weight: U256::from(100 * UNIT),
				delegations_reward_weight: U256::from(100 * UNIT),
				delegations_slash_weight: U256::from(100 * UNIT),
			},
		);
		let commitment = Commitment {
			stake: Some(Stake {
				amount: 100 * UNIT,
				rewardable_amount: 100 * UNIT,
				created: 5u64,
				cooldown_period: 100u64,
				cooldown_started: None,
				accrued_reward: 0,
				accrued_slash: 0,
				allow_auto_compound: true,
				paid: 0,
				applied_slash: 0,
			}),
			commission: Perbill::from_percent(10),
			delegations_total_amount: 100 * UNIT,
			delegations_total_rewardable_amount: 120 * UNIT, // Intentionally higher (buggy state)
			weights,
			pool_rewards,
			last_scoring_epoch: 10,
			last_slashing_epoch: 0,
		};
		Commitments::<Test, ()>::insert(commitment_id, commitment);

		// Force insert delegation with rewardable_amount > amount (the bug condition)
		let delegation = Delegation {
			stake: Stake {
				amount: 100 * UNIT,
				rewardable_amount: 120 * UNIT, // BUG: rewardable_amount > amount
				created: 5u64,
				cooldown_period: 100u64,
				cooldown_started: None,
				accrued_reward: 0,
				accrued_slash: 0,
				allow_auto_compound: true,
				paid: 0,
				applied_slash: 0,
			},
			reward_weight: U256::from(120 * UNIT), // Based on buggy rewardable_amount
			slash_weight: U256::from(100 * UNIT),
			reward_debt: 0,
			slash_debt: 0,
		};
		Delegations::<Test, ()>::insert(&delegator, commitment_id, delegation);
		DelegatorTotal::<Test, ()>::insert(&delegator, 100 * UNIT);

		// Verify the buggy state
		let delegation_before = Compute::delegations(&delegator, commitment_id).unwrap();
		println!("=== Before cooldown_delegation (buggy state) ===");
		println!("  amount: {}", delegation_before.stake.amount);
		println!("  rewardable_amount: {}", delegation_before.stake.rewardable_amount);
		println!("  reward_weight: {}", delegation_before.reward_weight);
		println!("  cooldown_started: {:?}", delegation_before.stake.cooldown_started);
		assert!(
			delegation_before.stake.rewardable_amount > delegation_before.stake.amount,
			"Pre-condition: rewardable_amount should be > amount (buggy state)"
		);
		assert!(
			delegation_before.stake.cooldown_started.is_none(),
			"Pre-condition: cooldown should not be started"
		);

		// Call cooldown_delegation - this triggers apply_delegator_slash -> accrue_delegator -> sanitize_delegation
		let result = Compute::cooldown_delegation(
			RuntimeOrigin::signed(delegator.clone()),
			committer.clone(),
		);
		println!("\nCooldown_delegation result: {:?}", result);
		assert!(result.is_ok());

		// Verify the sanitized state (and cooldown started)
		let delegation_after = Compute::delegations(&delegator, commitment_id).unwrap();
		println!("\n=== After cooldown_delegation (sanitized) ===");
		println!("  amount: {}", delegation_after.stake.amount);
		println!("  rewardable_amount: {}", delegation_after.stake.rewardable_amount);
		println!("  reward_weight: {}", delegation_after.reward_weight);
		println!("  cooldown_started: {:?}", delegation_after.stake.cooldown_started);

		// The fix should have corrected rewardable_amount
		assert!(
			delegation_after.stake.rewardable_amount <= delegation_after.stake.amount,
			"FIXED: rewardable_amount ({}) should be <= amount ({}) after sanitization",
			delegation_after.stake.rewardable_amount,
			delegation_after.stake.amount
		);

		// Cooldown should now be started
		assert!(
			delegation_after.stake.cooldown_started.is_some(),
			"cooldown should be started"
		);

		// When in cooldown, rewardable_amount = CooldownRewardRatio * amount
		// CooldownRewardRatio is typically < 1, so rewardable_amount < amount
		println!(
			"\n✓ Sanitization successful on cooldown: rewardable_amount corrected to {} (cooldown ratio applied)",
			delegation_after.stake.rewardable_amount
		);
	});
}

/// Test with exact production data that caused the InternalError.
/// This test force-inserts the exact delegation and commitment data from production
/// and verifies that withdrawal works correctly after the fix.
#[test]
fn test_exact_production_data_withdraw() {
	use crate::datastructures::MemoryBuffer;
	use crate::types::{Commitment, CommitmentWeights, Delegation, PoolReward, Stake};
	use crate::{Commitments, CurrentCycle, Delegations};
	use frame_support::traits::fungible::MutateHold;
	use frame_support::traits::{Currency, LockableCurrency, WithdrawReasons};

	ExtBuilder.build().execute_with(|| {
		// Use any accounts - the exact addresses don't matter
		let delegator = ferdie_account_id();
		let committer = charlie_account_id();

		// Set the exact production cycle: epoch 2205, epoch_start 1984501
		CurrentCycle::<Test, ()>::put(Cycle { epoch: 2204, epoch_start: 1983601 });
		System::set_block_number(1983601);

		// Fund distribution account so rewards can be paid out
		let distribution_account = Compute::account_id();
		let _ = Balances::deposit_creating(&distribution_account, 1_000_000 * UNIT);

		// Fund delegator so locks can be applied
		let _ = Balances::deposit_creating(&delegator, 10_000_000 * UNIT);

		// === Set exact production lock state ===
		// locks: [
		//   { id: 0x636f6d707374616b ("compstak"), amount: 4,017,022,779,575,126, reasons: All }
		//   { id: 0x7079636f6e766f74 ("pyconvot"), amount: 4,017,983,682,467,166, reasons: All }
		// ]
		let compstak_lock: u128 = 4_017_022_779_575_126;
		let pyconvot_lock: u128 = 4_017_983_682_467_166;
		Balances::set_lock(
			*b"compstak", // 0x636f6d707374616b - compute staking lock
			&delegator,
			compstak_lock,
			WithdrawReasons::all(),
		);
		Balances::set_lock(
			*b"pyconvot", // 0x7079636f6e766f74 - pallet conversion voting lock
			&delegator,
			pyconvot_lock,
			WithdrawReasons::all(),
		);
		println!("Locks set on delegator:");
		println!("  compstak (0x636f6d707374616b): {}", compstak_lock);
		println!("  pyconvot (0x7079636f6e766f74): {}", pyconvot_lock);

		// === Set exact production hold state ===
		// holds: [{ id: { AcurastTokenConversion: Conversion }, amount: 4,017,954,262,441,381 }]
		// Note: Mock uses RuntimeHoldReason = (), so we use () as the reason
		let hold_amount: u128 = 4_017_954_262_441_381;
		let _ = <Balances as MutateHold<_>>::hold(&(), &delegator, hold_amount);
		println!("Hold set on delegator:");
		println!("  AcurastTokenConversion::Conversion (simulated as ()): {}", hold_amount);

		// Get commitment_id for charlie (commitment 15 in production)
		offer_accept_backing(committer.clone());
		let commitment_id =
			<Test as Config>::CommitmentIdProvider::commitment_id_for(&committer).unwrap();

		// === Force insert the exact commitment data ===
		// poolRewards: past: null, current: [1,196,946, {rewardPerWeight: 51466117290070969348512659396, slashPerWeight: 243210658256359407742350755}]
		let pool_rewards: MemoryBuffer<u64, PoolReward> = MemoryBuffer::new_with(
			1_196_946u64,
			PoolReward {
				reward_per_weight: U256::from_dec_str("51466117290070969348512659396").unwrap(),
				slash_per_weight: U256::from_dec_str("243210658256359407742350755").unwrap(),
			},
		);

		// weights: past: [1890, {...}], current: [2204, {...}]
		let mut weights: MemoryBuffer<u64, CommitmentWeights> = MemoryBuffer::new_with(
			1890u64,
			CommitmentWeights {
				self_reward_weight: U256::from(49_999_999_999_999_999u128),
				self_slash_weight: U256::from(49_999_999_999_999_999u128),
				delegations_reward_weight: U256::from(4_352_395_776_255_708u128),
				delegations_slash_weight: U256::from(4_352_352_849_468_652u128),
			},
		);
		// Set current weights at epoch 2204
		let _ = weights.set(
			2204u64,
			CommitmentWeights {
				self_reward_weight: U256::from(49_999_999_999_999_999u128),
				self_slash_weight: U256::from(49_999_999_999_999_999u128),
				delegations_reward_weight: U256::from(4_352_395_776_255_708u128),
				delegations_slash_weight: U256::from(4_351_375_629_043_778u128),
			},
		);

		let commitment = Commitment {
			stake: Some(Stake {
				amount: 49_999_999_999_999_999u128,
				rewardable_amount: 49_999_999_999_999_999u128,
				created: 1_196_946u64,
				cooldown_period: 19_353_600u64,
				cooldown_started: None,
				accrued_reward: 2_152_176_750_113u128,
				accrued_slash: 0u128,
				allow_auto_compound: true,
				paid: 3_439_439_722_920_650u128,
				applied_slash: 0u128,
			}),
			commission: Perbill::from_percent(5),
			delegations_total_amount: 5_733_954_916_277_742u128,
			delegations_total_rewardable_amount: 5_734_979_109_589_042u128,
			weights,
			pool_rewards,
			last_scoring_epoch: 2205u64,
			last_slashing_epoch: 1617u64,
		};

		Commitments::<Test, ()>::insert(commitment_id, commitment);

		// === Force insert the exact delegation data ===
		let delegation = Delegation {
			stake: Stake {
				amount: 4_017_022_779_575_126u128,
				rewardable_amount: 4_018_000_000_000_000u128, // NOTE: rewardable > amount (bug!)
				created: 1_209_529u64,
				cooldown_period: 19_353_600u64,
				cooldown_started: None,
				accrued_reward: 0u128,
				accrued_slash: 0u128,
				allow_auto_compound: true,
				paid: 206_627_269_800_165u128,
				applied_slash: 0u128,
			},
			reward_weight: U256::from(4_018_000_000_000_000u128),
			slash_weight: U256::from(4_017_022_779_575_126u128),
			reward_debt: 206_627_269_800_168u128,
			slash_debt: 976_982_754_451u128,
		};

		Delegations::<Test, ()>::insert(&delegator, commitment_id, delegation.clone());

		// Print the state before withdrawal
		println!("=== Exact Production Data Test ===");
		println!("Delegation created: {}", delegation.stake.created);
		println!("Commitment stake created: 1196946");
		println!("Pool rewards current timestamp: 1196946");
		println!(
			"Delegation amount: {}, rewardable_amount: {}",
			delegation.stake.amount, delegation.stake.rewardable_amount
		);
		println!(
			"Delegation slash_weight: {}, slash_debt: {}",
			delegation.slash_weight, delegation.slash_debt
		);

		// Check what get_latest returns for this delegation
		let commitment_read = Compute::commitments(commitment_id).unwrap();
		let pool_rewards_result = commitment_read.pool_rewards.get_latest(delegation.stake.created);
		println!("get_latest({}) returns: {:?}", delegation.stake.created, pool_rewards_result);

		// Verify the delegation is NOT stale (created >= commitment.stake.created)
		let is_stale = delegation.stake.created < commitment_read.stake.as_ref().unwrap().created;
		println!("Is delegation stale? {}", is_stale);

		// Track balances before withdrawal
		let delegator_balance_before = Balances::free_balance(&delegator);
		let distribution_balance_before = Balances::free_balance(&distribution_account);
		println!("\nBalances before withdrawal:");
		println!("  Delegator: {}", delegator_balance_before);
		println!("  Distribution account: {}", distribution_balance_before);

		// Now try to withdraw - this should work with the fix
		println!("\n=== Attempting withdrawal ===");
		let result = Compute::withdraw_delegation(
			RuntimeOrigin::signed(delegator.clone()),
			committer.clone(),
		);
		println!("Withdrawal result: {:?}", result);

		// The withdrawal should succeed after the fix
		assert!(result.is_ok(), "Withdrawal should succeed with the fix applied");

		// Track balances after withdrawal
		let delegator_balance_after = Balances::free_balance(&delegator);
		let distribution_balance_after = Balances::free_balance(&distribution_account);
		println!("\nBalances after withdrawal:");
		println!("  Delegator: {}", delegator_balance_after);
		println!("  Distribution account: {}", distribution_balance_after);

		// Calculate and print the transferred amount
		let amount_transferred_to_delegator =
			delegator_balance_after.saturating_sub(delegator_balance_before);
		let amount_transferred_from_distribution =
			distribution_balance_before.saturating_sub(distribution_balance_after);
		println!("\nAmount transferred:");
		println!("  To delegator: {}", amount_transferred_to_delegator);
		println!("  From distribution account: {}", amount_transferred_from_distribution);

		// Verify delegation still exists and check its state
		if let Some(d) = Compute::delegations(&delegator, commitment_id) {
			println!(
				"\nAfter withdrawal - amount: {}, rewardable_amount: {}, slash_debt: {}",
				d.stake.amount, d.stake.rewardable_amount, d.slash_debt
			);
			println!(
				"  accrued_reward: {}, accrued_slash: {}",
				d.stake.accrued_reward, d.stake.accrued_slash
			);
		}
	});
}

/// Test slashing a committer who has tokens in a hold (e.g. AcurastTokenConversion)
/// and has staked almost all their remaining tokens, leaving only a small free amount.
///
/// This tests the scenario where:
/// - holds: [{ id: { AcurastTokenConversion: Conversion }, amount: 53,975,124,317,718,079 }]
/// - staked: 53,973,834,765,089,252
/// - free balance: ~120,036,022,067
#[test]
fn test_slash_committer_with_hold_and_near_max_stake() {
	use frame_support::traits::{LockableCurrency, WithdrawReasons};
	use sp_runtime::traits::AccountIdConversion;

	ExtBuilder.build().execute_with(|| {
		assert_ok!(Compute::enable_inflation(RuntimeOrigin::root()));

		// Set up required accounts (pallet account and alice for slashing), but NOT charlie
		// We'll set charlie's balance precisely for this test scenario
		assert_ok!(Balances::force_set_balance(
			RuntimeOrigin::root(),
			<Test as Config>::PalletId::get().into_account_truncating(),
			1_000_000_000 * UNIT
		));
		assert_ok!(Balances::force_set_balance(
			RuntimeOrigin::root(),
			alice_account_id(),
			1_000_000_000 * UNIT
		));
		// Also need bob for commit_alice_bob called in offer_accept_backing flow
		assert_ok!(Balances::force_set_balance(
			RuntimeOrigin::root(),
			bob_account_id(),
			1_000_000_000 * UNIT
		));

		create_pools();

		// Charlie will act as both manager and committer
		let charlie = charlie_account_id();

		// The specific amounts from the production scenario
		// We use a lock to simulate the AcurastTokenConversion hold effect
		let lock_amount: u128 = 53_975_124_317_718_079;
		let stake_amount: u128 = 53_973_834_765_089_252;
		let extra_free: u128 = 120_036_022_067;

		// In Substrate, when multiple locks exist, the effective lock is the maximum of all locks.
		// To have exactly `extra_free` remaining usable after staking:
		// - free_balance = lock_amount + extra_free (since lock_amount > stake_amount, it's the max lock)
		// - usable_balance = free_balance - max(lock_amount, stake_amount) = extra_free
		let total_balance = lock_amount + extra_free + EXISTENTIAL_DEPOSIT;

		// Set charlie's balance to the exact amount needed for this scenario
		assert_ok!(Balances::force_set_balance(
			RuntimeOrigin::root(),
			charlie.clone(),
			total_balance
		));

		// Set up a lock (simulating AcurastTokenConversion hold effect)
		// The lock makes these tokens unavailable for transfer but they still count in free_balance
		Balances::set_lock(
			*b"pyconvot", // 0x7079636f6e766f74 - pallet conversion lock (simulating the hold)
			&charlie,
			lock_amount,
			WithdrawReasons::all(),
		);

		println!("=== Test Setup ===");
		println!("Lock amount (simulating AcurastTokenConversion hold): {}", lock_amount);
		println!("Stake amount: {}", stake_amount);
		println!("Extra free: {}", extra_free);
		println!("Total balance: {}", total_balance);
		println!("Free balance after lock: {}", Balances::free_balance(&charlie));
		println!("Usable balance after lock: {}", Balances::usable_balance(&charlie));

		// Set up processor backing for charlie
		offer_accept_backing(charlie.clone());

		let charlie_manager =
			<Test as Config>::ManagerProviderForEligibleProcessor::lookup(&charlie).unwrap();

		// Charlie commits first time in warmup period (epoch 0)
		roll_to_block(10);
		assert_eq!(Compute::current_cycle(), Cycle { epoch: 0, epoch_start: 2 });
		assert_eq!(
			Compute::commit(&charlie, &charlie_manager, &[(2u8, 1000u128, 1u128)]).0,
			Zero::zero()
		);

		// Move to epoch 1 after warmup, Charlie commits again (now active)
		roll_to_block(150);
		assert_eq!(Compute::current_cycle(), Cycle { epoch: 1, epoch_start: 102 });

		// Charlie commits 4000 units for pool 2 as an active processor
		assert_eq!(
			Compute::commit(&charlie, &charlie_manager, &[(2u8, 4000u128, 1u128)]).0,
			Zero::zero()
		);

		// Now move to epoch 2 where Charlie can commit compute based on epoch 1 metrics
		roll_to_block(202);
		assert_eq!(Compute::current_cycle(), Cycle { epoch: 2, epoch_start: 202 });

		// Charlie can commit up to 80% of the previous epoch's metrics (4000 * 0.8 = 3200)
		let commitment: sp_runtime::BoundedVec<ComputeCommitment, sp_core::ConstU32<30>> =
			bounded_vec![ComputeCommitment {
				pool_id: 2,
				metric: FixedU128::from_rational(3200u128, 1u128), // Commit 3200 units (80% of 4000)
			},];

		let cooldown_period = 36u64;
		let commission = Perbill::from_percent(10);
		let allow_auto_compound = true;

		println!("\n=== Committing compute with stake {} ===", stake_amount);
		println!("Free balance before commit: {}", Balances::free_balance(&charlie));
		println!("Usable balance before commit: {}", Balances::usable_balance(&charlie));

		// Charlie commits compute with the near-maximum stake
		assert_ok!(Compute::commit_compute(
			RuntimeOrigin::signed(charlie.clone()),
			stake_amount,
			cooldown_period,
			commitment,
			commission,
			allow_auto_compound,
		));

		println!("Free balance after commit: {}", Balances::free_balance(&charlie));
		println!("Usable balance after commit: {}", Balances::usable_balance(&charlie));

		// Get Charlie's commitment ID
		let charlie_commitment_id =
			<Test as Config>::CommitmentIdProvider::commitment_id_for(&charlie).unwrap();

		// Verify initial stake
		let initial_commitment = Compute::commitments(charlie_commitment_id).unwrap();
		let initial_stake = initial_commitment.stake.as_ref().unwrap();
		assert_eq!(initial_stake.amount, stake_amount);

		println!("\n=== Initial commitment state ===");
		println!("Stake amount: {}", initial_stake.amount);
		println!("Rewardable amount: {}", initial_stake.rewardable_amount);

		// Move to next epoch
		roll_to_block(302);
		assert_eq!(Compute::current_cycle(), Cycle { epoch: 3, epoch_start: 302 });

		// Charlie delivers only 50% of committed metrics (1600 instead of 3200)
		// This will trigger slashing
		Compute::commit(&charlie, &charlie_manager, &[(2u8, 1600u128, 1u128)]);

		// Move to next epoch to allow slashing
		roll_to_block(402);
		assert_eq!(Compute::current_cycle(), Cycle { epoch: 4, epoch_start: 402 });

		let balance_before_slash = Balances::free_balance(&charlie);
		let usable_before_slash = Balances::usable_balance(&charlie);
		println!("\n=== Before slash ===");
		println!("Free balance: {}", balance_before_slash);
		println!("Usable balance: {}", usable_before_slash);

		// Calculate what the expected slash amount would be
		let pool = Compute::metric_pools(2).unwrap();
		let pool_reward_ratio = pool.reward.get(3); // epoch 3
		let total_stake_for_slash = initial_stake.amount; // No delegations in this test
		let base_slash = Perquintill::from_percent(1).mul_floor(total_stake_for_slash);
		let pool_slash = pool_reward_ratio.mul_floor(base_slash);
		let unfulfilled_ratio = Perquintill::from_percent(50); // 50% missed
		let expected_slash = unfulfilled_ratio.mul_floor(pool_slash);

		println!("\n=== Expected slash calculation ===");
		println!("Total stake: {}", total_stake_for_slash);
		println!("Base slash (1%): {}", base_slash);
		println!("Pool reward ratio: {:?}", pool_reward_ratio);
		println!("Pool slash: {}", pool_slash);
		println!("Expected slash amount: {}", expected_slash);
		println!("Usable balance for transfer: {}", usable_before_slash);

		// Someone (alice) calls slash on Charlie for the missed metrics in epoch 3
		// The fix uses slash_for() + imbalance.extract() + resolve() instead of transfer()
		// This works with locked funds because slash_for uses Balanced::withdraw with Fortitude::Force
		let slash_result =
			Compute::slash(RuntimeOrigin::signed(alice_account_id()), charlie.clone());
		println!("\n=== Slash result: {:?} ===", slash_result);
		assert_ok!(slash_result);

		// Verify Charlie's stake was decreased
		let slashed_commitment = Compute::commitments(charlie_commitment_id).unwrap();
		let slashed_stake = slashed_commitment.stake.as_ref().unwrap();

		println!("\n=== After slash ===");
		println!("Initial stake amount: {}", initial_stake.amount);
		println!("Slashed stake amount: {}", slashed_stake.amount);
		println!("Free balance: {}", Balances::free_balance(&charlie));
		println!("Usable balance: {}", Balances::usable_balance(&charlie));

		// The stake should be less than the initial stake
		assert!(
			slashed_stake.amount < initial_stake.amount,
			"Stake should be decreased after slashing. Initial: {}, After slash: {}",
			initial_stake.amount,
			slashed_stake.amount
		);

		// Verify Slashed event was emitted
		assert!(events().iter().any(|e| matches!(e, RuntimeEvent::Compute(Event::Slashed(_)))));

		let actual_slash = initial_stake.amount - slashed_stake.amount;

		println!("\n=== Slash verification ===");
		println!("Expected slash: {}", expected_slash);
		println!("Actual slash: {}", actual_slash);

		// The actual slash should match expected (allowing for small rounding differences)
		assert!(
			actual_slash >= expected_slash.saturating_sub(1)
				&& actual_slash <= expected_slash.saturating_add(1),
			"Actual slash {} should be close to expected slash {}",
			actual_slash,
			expected_slash
		);

		// Verify the lock is still intact - the slashing mechanism should not affect unrelated locks
		println!("\n=== Fix verified ===");
		println!("Slashing succeeded even with limited usable balance!");
		println!("  - Lock/hold amount: {}", lock_amount);
		println!("  - Staked amount: {}", stake_amount);
		println!("  - Usable balance before slash: {}", usable_before_slash);
		println!("  - Slash amount: {}", actual_slash);
	});
}

/// Committer `C` stakes 5 UNIT and each delegator delegates 5 UNIT at the same cooldown, so one
/// incumbent carries exactly the committer's own weight. The commitment earns 100 UNIT for epoch 3.
///
/// `incumbents` delegate before epoch 3 is scored (they are the weight epoch 3 gets settled
/// against); `epoch3_actions` run inside epoch 3, after it has been scored; `gap_actions` run in
/// epoch 4 *before* the heartbeat that distributes for epoch 3.
///
/// With `settle == false` the trailing heartbeat is omitted, leaving epoch 3 unsettled so a test can
/// drive `distribute` itself and inspect its result.
fn settlement_gap_flow(
	incumbents: &[&str],
	epoch3_actions: &[Action],
	gap_actions: &[Action],
	settle: bool,
) -> Vec<Action> {
	let heartbeat = || {
		vec![
			Action::ProcessorCommit {
				processor: "A".to_string(),
				metrics: vec![(1, 1000, 1), (2, 2000, 1)],
			},
			Action::ProcessorCommit { processor: "B".to_string(), metrics: vec![(2, 6000, 1)] },
		]
	};
	let final_heartbeat = if settle { heartbeat() } else { vec![] };
	[
		&commit_actions_2_processors()[..],
		&[Action::CommitCompute {
			committer: "C".to_string(),
			stake: 5 * UNIT,
			cooldown: 36,
			metrics: vec![(2, 4000u128 * 4 / 5, 1u128)],
			commission: Perbill::from_percent(0),
		}][..],
		&incumbents
			.iter()
			.map(|d| Action::Delegate {
				delegator: d.to_string(),
				committer: "C".to_string(),
				amount: 5 * UNIT,
				cooldown: 36,
			})
			.collect::<Vec<_>>()[..],
		&heartbeat()[..],
		&[Action::RollToBlock {
			block_number: 302,
			expected_cycle: Cycle { epoch: 3, epoch_start: 302 },
		}][..],
		// heartbeat that scores for epoch 3
		&heartbeat()[..],
		epoch3_actions,
		&[Action::RollToBlock {
			block_number: 402,
			expected_cycle: Cycle { epoch: 4, epoch_start: 402 },
		}][..],
		// ── settlement gap: epoch 3 is over but has not been distributed yet ──
		gap_actions,
		// heartbeat that lazily distributes for epoch 3
		&final_heartbeat[..],
	]
	.concat()
}

fn run_settlement_gap_flow(incumbents: &[&str], gap_actions: &[Action]) {
	compute_test_flow(
		1,
		&[0, 100],
		&[("C", &["A", "B"])],
		&settlement_gap_flow(incumbents, &[], gap_actions, true),
	);
}

fn withdraw_delegation_reward(delegator: &AccountId32) -> Balance {
	let before = Balances::free_balance(delegator);
	assert_ok!(Compute::withdraw_delegation(
		RuntimeOrigin::signed(delegator.clone()),
		charlie_account_id()
	));
	Balances::free_balance(delegator) - before
}

/// Equality up to the rounding dust a per-weight accumulator inevitably leaves behind (the same
/// tolerance the action harness applies to its expected rewards).
fn assert_close(actual: Balance, expected: Balance, label: &str) {
	let diff = actual.abs_diff(expected);
	assert!(
		diff < MICROUNIT,
		"{}: expected {} but got {} (off by {})",
		label,
		expected,
		actual,
		diff
	);
}

/// Baseline for [`test_delegating_in_settlement_gap_stays_within_epoch_budget`]: with no churn in
/// the gap, the single incumbent collects the whole epoch-3 delegator budget, which is half of the
/// commitment's 100 UNIT because its weight equals the committer's.
#[test]
fn test_settlement_gap_budget_baseline_one_incumbent() {
	ExtBuilder.build().execute_with(|| {
		run_settlement_gap_flow(&["D"], &[]);
		assert_close(
			withdraw_delegation_reward(&dave_account_id()),
			50 * UNIT,
			"epoch 3 delegator budget",
		);
	});
}

/// Net inflow: a delegation entering the gap must not increase the total paid out for epoch 3.
#[test]
fn test_delegating_in_settlement_gap_stays_within_epoch_budget() {
	ExtBuilder.build().execute_with(|| {
		run_settlement_gap_flow(
			&["D"],
			&[Action::Delegate {
				delegator: "G".to_string(),
				committer: "C".to_string(),
				amount: 5 * UNIT,
				cooldown: 36,
			}],
		);

		let incumbent = withdraw_delegation_reward(&dave_account_id());
		let entrant = withdraw_delegation_reward(&george_account_id());

		assert_close(incumbent + entrant, 50 * UNIT, "total paid for epoch 3");
		assert_close(entrant, 25 * UNIT, "gap entrant's share");
		assert_close(incumbent, 25 * UNIT, "incumbent's diluted share");
	});
}

/// Baseline for [`test_cooldown_in_settlement_gap_does_not_strand_epoch_budget`]: two incumbents
/// each carrying the committer's weight take 2/3 of the commitment's 100 UNIT between them.
#[test]
fn test_settlement_gap_budget_baseline_two_incumbents() {
	ExtBuilder.build().execute_with(|| {
		run_settlement_gap_flow(&["D", "E"], &[]);
		let total = withdraw_delegation_reward(&dave_account_id())
			+ withdraw_delegation_reward(&eve_account_id());
		assert_close(total, 200 * UNIT / 3, "epoch 3 delegator budget");
	});
}

/// Net outflow: a delegation shrinking in the gap must not decrease the total paid out either. The
/// share it forfeits goes to the delegations that stayed rather than being stranded in the pot.
#[test]
fn test_cooldown_in_settlement_gap_does_not_strand_epoch_budget() {
	ExtBuilder.build().execute_with(|| {
		// E starts cooling down in the gap, which drops its reward weight to `CooldownRewardRatio`
		// of the original and rotates the weights buffer to epoch 4.
		run_settlement_gap_flow(
			&["D", "E"],
			&[Action::CooldownDelegation {
				delegator: "E".to_string(),
				committer: "C".to_string(),
			}],
		);

		let stayed = withdraw_delegation_reward(&dave_account_id());
		let cooled = withdraw_delegation_reward(&eve_account_id());

		// Before the fix the divisor still carried E's full pre-cooldown weight, so the pair could
		// only ever claim less than the budget and the difference stayed in the pot forever.
		assert_close(stayed + cooled, 200 * UNIT / 3, "total paid for epoch 3");
		assert!(
			stayed > cooled,
			"the delegation that did not cool down takes the larger share ({} vs {})",
			stayed,
			cooled
		);
	});
}

/// Every delegation leaving in the gap drives the current delegation weight to zero while the
/// settled epoch still has weight. That is the case the divisor guard exists for: dividing the
/// delegator share by the current weight would be a division by zero, making `distribute` return
/// `Err` — which the heartbeat swallows with `unwrap_or_default()`, dropping the committer's bonus
/// and leaving the function half-applied. `distribute` is driven directly here because that `Err` is
/// not otherwise observable.
#[test]
fn test_all_delegations_leaving_in_settlement_gap_distributes_without_error() {
	ExtBuilder.build().execute_with(|| {
		compute_test_flow(
			1,
			&[0, 100],
			&[("C", &["A", "B"])],
			&settlement_gap_flow(
				&["D"],
				// Cool down inside epoch 3 so the cooldown has elapsed by epoch 4 and the
				// delegation can actually be ended in the gap. This halves D's epoch-3 weight
				// (`CooldownRewardRatio`), so epoch 3 settles against S=5 UNIT vs W=2.5 UNIT.
				&[
					Action::CooldownDelegation {
						delegator: "D".to_string(),
						committer: "C".to_string(),
					},
					Action::RollToBlock {
						block_number: 350,
						expected_cycle: Cycle { epoch: 3, epoch_start: 302 },
					},
				],
				&[Action::EndDelegation {
					delegator: "D".to_string(),
					committer: "C".to_string(),
					// Accrues against the pre-distribution accumulator, so epoch 3 pays nothing.
					expected_reward: 0,
				}],
				// leave epoch 3 unsettled so `distribute` can be driven directly below
				false,
			),
		);

		let commitment_id =
			<Test as Config>::CommitmentIdProvider::commitment_id_for(&charlie_account_id())
				.unwrap();
		let mut commitment = Compute::commitments(commitment_id).unwrap();
		assert_eq!(
			commitment.weights.get_current().1.delegations_reward_weight,
			U256::zero(),
			"the gap must have left no delegation weight behind"
		);
		assert!(
			commitment.weights.get_latest(3).unwrap().delegations_reward_weight > U256::zero(),
			"epoch 3 itself must still have delegation weight, otherwise this proves nothing"
		);

		let pool_ids: Vec<_> = (1..=Compute::last_metric_pool_id()).collect();
		assert_ok!(Compute::distribute(3, commitment_id, &mut commitment, &pool_ids));

		// Nobody is left to pay, so the delegator share stays in the pot: the accumulator must not
		// have been bumped.
		assert_eq!(
			commitment.pool_rewards.get_current().1.reward_per_weight,
			U256::zero(),
			"delegator share must not be credited when no delegation weight remains"
		);

		// The committer's own share is unaffected — S/(S+W) with S=5 UNIT and W=2.5 UNIT (D halved
		// its weight by cooling down) is 2/3 of the commitment's 100 UNIT.
		assert_close(
			commitment.stake.unwrap().accrued_reward,
			200 * UNIT / 3,
			"committer's epoch 3 share",
		);
	});
}

/// Committer `charlie` (its own manager and processor) commits 3200 units for epoch 3 and then
/// delivers only half, making it slashable for epoch 3 throughout epoch 4. Returns the commitment
/// id, positioned at the start of epoch 4 with the slash not yet triggered.
///
/// `epoch3_cooldowns` start their cooldown inside epoch 3, which has elapsed by epoch 4 so those
/// delegations can be ended before the slash is triggered. Note cooldown reduces only *reward*
/// weight, so epoch 3's slash weight is unaffected by it.
fn slashable_commitment_flow(
	delegators: &[(AccountId32, Balance)],
	epoch3_cooldowns: &[AccountId32],
) -> u128 {
	assert_ok!(Compute::enable_inflation(RuntimeOrigin::root()));
	setup_balances();
	create_pools();

	let committer = charlie_account_id();
	offer_accept_backing(committer.clone());
	let manager =
		<Test as Config>::ManagerProviderForEligibleProcessor::lookup(&committer).unwrap();

	roll_to_block(10);
	Compute::commit(&committer, &manager, &[(2u8, 1000u128, 1u128)]);
	roll_to_block(150);
	Compute::commit(&committer, &manager, &[(2u8, 4000u128, 1u128)]);

	roll_to_block(202);
	assert_eq!(Compute::current_cycle(), Cycle { epoch: 2, epoch_start: 202 });
	assert_ok!(Compute::commit_compute(
		RuntimeOrigin::signed(committer.clone()),
		10 * UNIT,
		36,
		bounded_vec![ComputeCommitment {
			pool_id: 2,
			metric: FixedU128::from_rational(3200u128, 1u128),
		}],
		Perbill::from_percent(0),
		true,
	));
	for (who, amount) in delegators {
		assert_ok!(Compute::delegate(
			RuntimeOrigin::signed(who.clone()),
			committer.clone(),
			*amount,
			36,
			true,
		));
	}

	roll_to_block(302);
	assert_eq!(Compute::current_cycle(), Cycle { epoch: 3, epoch_start: 302 });
	// deliver only half of the committed 3200 -> slashable for epoch 3
	Compute::commit(&committer, &manager, &[(2u8, 1600u128, 1u128)]);

	for who in epoch3_cooldowns {
		assert_ok!(Compute::cooldown_delegation(
			RuntimeOrigin::signed(who.clone()),
			committer.clone(),
		));
	}

	roll_to_block(402);
	assert_eq!(Compute::current_cycle(), Cycle { epoch: 4, epoch_start: 402 });

	<Test as Config>::CommitmentIdProvider::commitment_id_for(&committer).unwrap()
}

/// Realizes a delegation's pending slash (any delegator operation accrues then applies it) and
/// returns how much stake it actually cost.
fn realize_delegator_slash(delegator: &AccountId32, commitment_id: u128) -> Balance {
	let before = Compute::delegations(delegator, commitment_id).unwrap().stake.amount;
	assert_ok!(Compute::withdraw_delegation(
		RuntimeOrigin::signed(delegator.clone()),
		charlie_account_id()
	));
	before - Compute::delegations(delegator, commitment_id).unwrap().stake.amount
}

/// The slash mirror. `do_slash` divides the delegators' share of the slash by the same stale weight
/// `distribute` used to, so a delegation entering before the slash lands made the delegators
/// collectively absorb *more* than the slash actually apportioned to them.
///
/// The window here is a whole epoch wide, not just until the first heartbeat: a slash for epoch 3
/// can be triggered at any block of epoch 4.
#[test]
fn test_delegating_before_slash_does_not_inflate_absorbed_slash() {
	ExtBuilder.build().execute_with(|| {
		let incumbent = ferdie_account_id();
		let entrant = george_account_id();
		let commitment_id = slashable_commitment_flow(&[(incumbent.clone(), 25 * UNIT)], &[]);

		// A new delegation lands in epoch 4 before anyone calls `slash`.
		assert_ok!(Compute::delegate(
			RuntimeOrigin::signed(entrant.clone()),
			charlie_account_id(),
			10 * UNIT,
			36,
			true,
		));

		let committer_stake_before =
			Compute::commitments(commitment_id).unwrap().stake.unwrap().amount;
		assert_ok!(Compute::slash(RuntimeOrigin::signed(alice_account_id()), charlie_account_id()));
		let committer_slash = committer_stake_before
			- Compute::commitments(commitment_id).unwrap().stake.unwrap().amount;
		assert!(committer_slash > 0, "committer must have been slashed");

		// Epoch 3's slash weights are the committer's 10 UNIT against the incumbent's 25 UNIT (both
		// at the same cooldown), and the slash splits along them. Deriving the expected delegator
		// share from the committer's *observed* slash keeps this independent of the delegator-side
		// arithmetic under test.
		let expected_delegations_slash = committer_slash * 25 / 10;

		let absorbed = realize_delegator_slash(&incumbent, commitment_id)
			+ realize_delegator_slash(&entrant, commitment_id);

		// Before the fix the divisor held only the incumbent's 25 UNIT of weight while both
		// delegations absorbed against it, so this came out 1.4x too high.
		assert_close(absorbed, expected_delegations_slash, "slash absorbed by delegators");

		// The accepted residual, mirroring the reward side: the entrant is still slashed for an
		// epoch it was absent for, but the total is now right.
		assert!(
			realize_delegator_slash(&entrant, commitment_id) == 0,
			"slash must be applied exactly once"
		);
	});
}

/// Slash-side mirror of [`test_all_delegations_leaving_in_settlement_gap_distributes_without_error`].
///
/// Here the missing-guard consequence is worse than on the reward side: `slash` is an extrinsic, so
/// the `Err` is not swallowed — it makes the call fail outright, and since epoch 3 can only be
/// slashed during epoch 4, the slash would be lost for good.
#[test]
fn test_all_delegations_leaving_before_slash_slashes_without_error() {
	ExtBuilder.build().execute_with(|| {
		let incumbent = ferdie_account_id();
		let commitment_id = slashable_commitment_flow(
			&[(incumbent.clone(), 25 * UNIT)],
			std::slice::from_ref(&incumbent),
		);

		// The cooldown started in epoch 3 and has elapsed, so the delegation can leave in epoch 4
		// before anyone triggers the slash.
		assert_ok!(Compute::end_delegation(
			RuntimeOrigin::signed(incumbent.clone()),
			charlie_account_id()
		));

		let commitment = Compute::commitments(commitment_id).unwrap();
		assert_eq!(
			commitment.weights.get_current().1.delegations_slash_weight,
			U256::zero(),
			"no delegation slash weight must remain"
		);
		assert!(
			commitment.weights.get_latest(3).unwrap().delegations_slash_weight > U256::zero(),
			"epoch 3 itself must still have slash weight, otherwise this proves nothing"
		);

		assert_ok!(Compute::slash(RuntimeOrigin::signed(alice_account_id()), charlie_account_id()));
	});
}
