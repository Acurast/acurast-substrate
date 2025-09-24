pub mod test_actions;

pub use test_actions::{compute_test_flow, events, roll_to_block, setup, Action};

use frame_support::{assert_err, assert_ok};
use sp_core::{bounded_vec, U256};
use sp_runtime::{
	traits::{AccountIdConversion, Zero},
	AccountId32, FixedU128, Perbill, Perquintill,
};

use crate::{
	datastructures::{ProvisionalBuffer, SlidingBuffer},
	mock::*,
	stub::*,
	types::*,
	Config, Cycle, Error, Event,
};
use acurast_common::{CommitmentIdProvider, ComputeHooks, ManagerIdProvider};

fn commit_actions() -> Vec<Action> {
	vec![
		Action::RollToBlock {
			block_number: 10,
			expected_cycle: Cycle { epoch: 0, epoch_start: 2, era: 0, era_start: 2 },
		},
		Action::ProcessorCommit { processor: "A".to_string(), metrics: vec![(1, 1000, 1)] },
		Action::RollToBlock {
			block_number: 20,
			expected_cycle: Cycle { epoch: 0, epoch_start: 2, era: 0, era_start: 2 },
		},
		Action::ProcessorCommit { processor: "B".to_string(), metrics: vec![(1, 1000, 1)] },
		Action::RollToBlock {
			block_number: 150,
			expected_cycle: Cycle { epoch: 1, epoch_start: 102, era: 0, era_start: 2 },
		},
		Action::ProcessorCommit {
			processor: "A".to_string(),
			metrics: vec![(1, 1000, 1), (2, 2000, 1)], // A commits 2000 to pool 2 later used for compute commitment
		},
		Action::ProcessorCommit { processor: "B".to_string(), metrics: vec![(2, 6000, 1)] }, // B commits 6000 to pool 2 later used for compute commitment
		Action::RollToBlock {
			block_number: 302, // skipping epoch 2 since average is taken from last era (not epoch)
			expected_cycle: Cycle { epoch: 3, epoch_start: 302, era: 1, era_start: 302 },
		},
	]
}

#[test]
fn test_compute_flow_no_delegations_no_rewards() {
	ExtBuilder.build().execute_with(|| {
		compute_test_flow(
			&[30, 50, 20], // three pools matching original test
			&[
				("C", &["A", "B"]), // committer C with processors A, B
			],
			&[
				&commit_actions()[..],
				&[
					Action::CommitCompute {
						committer: "C".to_string(),
						stake: 5 * UNIT,
						cooldown: 108, // 1/3 of max
						metrics: vec![(2, 4000u128 * 4 / 5, 1u128)],
						commission: Perbill::from_percent(10),
					}, // Maximal possible commitment value: 80% of average for pool 2
					Action::CooldownComputeCommitment { committer: "C".to_string() },
					Action::RollToBlock {
						block_number: 410, // Advance past cooldown period (started at 302, +108 blocks)
						expected_cycle: Cycle {
							epoch: 4,
							epoch_start: 402,
							era: 1,
							era_start: 302,
						},
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
			&[30, 50, 20], // three pools matching original test
			&[
				("C", &["A", "B"]), // committer C with processors A, B
			],
			&[
				&commit_actions()[..],
				&[
					Action::CommitCompute {
						committer: "C".to_string(),
						stake: 5 * UNIT,
						cooldown: 108, // 1/3 of max
						metrics: vec![(2, 4000u128 * 4 / 5, 1u128)],
						commission: Perbill::from_percent(10),
					}, // Maximal possible commitment value: 80% of average for pool 2
					Action::Reward { amount: 10 * UNIT },
					Action::CooldownComputeCommitment { committer: "C".to_string() },
					Action::RollToBlock {
						block_number: 410, // Advance past cooldown period (started at 302, +108 blocks)
						expected_cycle: Cycle {
							epoch: 4,
							epoch_start: 402,
							era: 1,
							era_start: 302,
						},
					},
					Action::EndComputeCommitment {
						committer: "C".to_string(),
						expected_reward: 5 * UNIT,
					},
				][..],
			]
			.concat(),
		);
	});
}

#[test]
fn test_compute_no_rewards() {
	ExtBuilder.build().execute_with(|| {
		compute_test_flow(
			&[30, 50, 20], // three pools matching original test
			&[
				("C", &["A", "B"]), // committer C with processors A, B
			],
			&[
				&commit_actions()[..],
				&[
					Action::CommitCompute {
						committer: "C".to_string(),
						stake: 5 * UNIT,
						cooldown: 36, // 1/3 of max
						metrics: vec![(2, 4000u128 * 4 / 5, 1u128)],
						commission: Perbill::from_percent(10),
					}, // Maximal possible commitment value: 80% of average for pool 2
					Action::Delegate {
						delegator: "D".to_string(),
						committer: "C".to_string(),
						amount: 40 * UNIT,
						cooldown: 36,
					},
					Action::Delegate {
						delegator: "E".to_string(),
						committer: "C".to_string(),
						amount: 5 * UNIT,
						cooldown: 36,
					},
					Action::CooldownComputeCommitment { committer: "C".to_string() },
					Action::CooldownDelegation {
						delegator: "D".to_string(),
						committer: "C".to_string(),
					},
					Action::CooldownDelegation {
						delegator: "E".to_string(),
						committer: "C".to_string(),
					},
					Action::RollToBlock {
						block_number: 400, // Advance past cooldown period (started at 302, +36 blocks + buffer)
						expected_cycle: Cycle {
							epoch: 3,
							epoch_start: 302,
							era: 1,
							era_start: 302,
						},
					},
					Action::EndComputeCommitment { committer: "C".to_string(), expected_reward: 0 },
					Action::EndDelegation {
						delegator: "D".to_string(),
						committer: "C".to_string(),
						expected_reward: 0,
					},
					Action::EndDelegation {
						delegator: "E".to_string(),
						committer: "C".to_string(),
						expected_reward: 0,
					},
				][..],
			]
			.concat(),
		);
	});
}

#[test]
fn test_compute_flow_1() {
	ExtBuilder.build().execute_with(|| {
		compute_test_flow(
			&[30, 50, 20], // three pools matching original test
			&[
				("C", &["A", "B"]), // committer C with processors A, B
			],
			&[
				&commit_actions()[..],
				&[
					Action::CommitCompute {
						committer: "C".to_string(),
						stake: 5 * UNIT,
						cooldown: 36, // 1/3 of max
						metrics: vec![(2, 4000u128 * 4 / 5, 1u128)],
						commission: Perbill::from_percent(10),
					}, // Maximal possible commitment value: 80% of average for pool 2
					Action::Delegate {
						delegator: "D".to_string(),
						committer: "C".to_string(),
						amount: 40 * UNIT,
						cooldown: 36,
					},
					Action::Delegate {
						delegator: "E".to_string(),
						committer: "C".to_string(),
						amount: 5 * UNIT,
						cooldown: 36,
					},
					Action::Reward { amount: 10 * UNIT },
					Action::CooldownComputeCommitment { committer: "C".to_string() },
					Action::CooldownDelegation {
						delegator: "D".to_string(),
						committer: "C".to_string(),
					},
					Action::CooldownDelegation {
						delegator: "E".to_string(),
						committer: "C".to_string(),
					},
					Action::RollToBlock {
						block_number: 400, // Advance past cooldown period (started at 302, +36 blocks + buffer)
						expected_cycle: Cycle {
							epoch: 3,
							epoch_start: 302,
							era: 1,
							era_start: 302,
						},
					},
					Action::EndComputeCommitment {
						committer: "C".to_string(),
						expected_reward: 950 * MILLIUNIT,
					},
					// D committed 40 for 1/3 of max cooldown
					// vs E committed 5 for 1/3 of max cooldown
					// That makes D get 40/45 of delegators' total payout
					//
					// NOTE: committer has equal 1/3 of max cooldown so equal weight as delegators from this perspective
					// total delegator payout = 10 [single reward] * 0.5 [metric commitment] * 1/1 [cooldown ratio] * 45/(45 + 5) [delegator vs total commitment stake] * 0.9 [commission]
					//                        = 4.05
					//
					// Delegator D's payout = 4.05 * 40/45
					//                      = 3.6
					Action::EndDelegation {
						delegator: "D".to_string(),
						committer: "C".to_string(),
						expected_reward: 3_600 * MILLIUNIT,
					},
					// Delegator D's payout = 4.05 * 5/45
					//                      = 0.45
					Action::EndDelegation {
						delegator: "E".to_string(),
						committer: "C".to_string(),
						expected_reward: 450_000_000_000,
					},
				][..],
			]
			.concat(),
		);
	});
}

#[test]
fn test_compute_flow_varied_cooldown() {
	ExtBuilder.build().execute_with(|| {
		compute_test_flow(
			&[30, 50, 20], // same pools as original
			&[
				("C", &["A", "B"]), // committer C with processors A, B
			],
			&[
				&commit_actions()[..],
				&[
					// Test with maximum cooldown (108)
					Action::CommitCompute {
						committer: "C".to_string(),
						stake: 5 * UNIT,
						cooldown: 108, // maximum cooldown
						metrics: vec![(2, 4000u128 * 4 / 5, 1u128)],
						commission: Perbill::from_percent(10),
					},
					Action::Delegate {
						delegator: "D".to_string(),
						committer: "C".to_string(),
						amount: 40 * UNIT,
						cooldown: 108, // matching maximum cooldown
					},
					Action::Delegate {
						delegator: "E".to_string(),
						committer: "C".to_string(),
						amount: 5 * UNIT,
						cooldown: 72, // different cooldown to test weight calculation
					},
					Action::Reward { amount: 10 * UNIT },
					Action::CooldownComputeCommitment { committer: "C".to_string() },
					Action::CooldownDelegation {
						delegator: "D".to_string(),
						committer: "C".to_string(),
					},
					Action::CooldownDelegation {
						delegator: "E".to_string(),
						committer: "C".to_string(),
					},
					Action::RollToBlock {
						block_number: 480, // Advance past max cooldown period (started at 302, +108 blocks + buffer)
						expected_cycle: Cycle {
							epoch: 4,
							epoch_start: 402,
							era: 1,
							era_start: 302,
						},
					},
					// D committed 40 for 1/1 of max cooldown
					// vs E committed 5 for 2/3 of max cooldown
					// That makes D get 40/45 * 4/6 of delegators' total payout
					// and E gets the remaining 5/45 * 2/6 of delegators' total payout
					//
					// NOTE: Cooldown ratio between delegators and committer is (1/1 + 2/3)/(1/1 + 2/3 + 1/1) = (3/3 + 2/3)/(3/3 + 2/3 + 3/3) = (5/3)/(8/3) = 5/8
					//
					// total delegator payout = 10 [single reward] * 0.5 [metric commitment] * 5/8 [cooldown ratio] * 45/(45 + 5) [delegator vs total commitment stake] * 0.9 [commission]
					//                        = 2.53125
					//
					// Delegator D's payout = 2.53125 * 40/45 * 3/5
					//                      = 1.35
					Action::EndComputeCommitment {
						committer: "C".to_string(),
						expected_reward: 965517241378,
					},
					Action::EndDelegation {
						delegator: "D".to_string(),
						committer: "C".to_string(),
						expected_reward: 3724137931034,
					},
					Action::EndDelegation {
						delegator: "E".to_string(),
						committer: "C".to_string(),
						expected_reward: 310344827586,
					},
				][..],
			]
			.concat(),
		);
	});
}

#[test]
fn test_compute_flow_varied_stakes() {
	ExtBuilder.build().execute_with(|| {
		compute_test_flow(
			&[30, 50, 20], // same pools as original
			&[
				("C", &["A", "B"]), // committer C with processors A, B
			],
			&[
				&commit_actions()[..],
				&[
					// Test with different stake amount (10 instead of 5)
					Action::CommitCompute {
						committer: "C".to_string(),
						stake: 10 * UNIT, // doubled stake
						cooldown: 36,
						metrics: vec![(2, 4000u128 * 4 / 5, 1u128)],
						commission: Perbill::from_percent(10),
					},
					Action::Delegate {
						delegator: "D".to_string(),
						committer: "C".to_string(),
						amount: 80 * UNIT, // doubled amount to maintain ratio
						cooldown: 36,
					},
					Action::Delegate {
						delegator: "E".to_string(),
						committer: "C".to_string(),
						amount: 10 * UNIT, // doubled amount
						cooldown: 36,
					},
					Action::Reward { amount: 10 * UNIT },
					Action::CooldownComputeCommitment { committer: "C".to_string() },
					Action::CooldownDelegation {
						delegator: "D".to_string(),
						committer: "C".to_string(),
					},
					Action::CooldownDelegation {
						delegator: "E".to_string(),
						committer: "C".to_string(),
					},
					Action::RollToBlock {
						block_number: 400,
						expected_cycle: Cycle {
							epoch: 3,
							epoch_start: 302,
							era: 1,
							era_start: 302,
						},
					},
					Action::EndComputeCommitment {
						committer: "C".to_string(),
						expected_reward: 950 * MILLIUNIT,
					},
					Action::EndDelegation {
						delegator: "D".to_string(),
						committer: "C".to_string(),
						expected_reward: 3600 * MILLIUNIT,
					},
					Action::EndDelegation {
						delegator: "E".to_string(),
						committer: "C".to_string(),
						expected_reward: 450 * MILLIUNIT,
					},
				][..],
			]
			.concat(),
		);
	});
}

#[test]
fn test_compute_flow_multi_pool_metrics() {
	ExtBuilder.build().execute_with(|| {
		compute_test_flow(
			&[30, 30, 20, 20], // four pools with different allocations
			&[
				("C", &["A", "B"]), // committer C with processors A, B
			],
			&[
				// Use first part of commit_actions but with modified metrics for multi-pool test
				Action::RollToBlock {
					block_number: 10,
					expected_cycle: Cycle { epoch: 0, epoch_start: 2, era: 0, era_start: 2 },
				},
				Action::ProcessorCommit { processor: "A".to_string(), metrics: vec![(1, 1000, 1)] },
				Action::RollToBlock {
					block_number: 20,
					expected_cycle: Cycle { epoch: 0, epoch_start: 2, era: 0, era_start: 2 },
				},
				Action::ProcessorCommit { processor: "B".to_string(), metrics: vec![(1, 1000, 1)] },
				Action::RollToBlock {
					block_number: 150,
					expected_cycle: Cycle { epoch: 1, epoch_start: 102, era: 0, era_start: 2 },
				},
				// A commits to multiple pools
				Action::ProcessorCommit {
					processor: "A".to_string(),
					metrics: vec![(1, 1000, 1), (2, 2000, 1), (3, 1500, 1)],
				},
				// B commits to different pools
				Action::ProcessorCommit {
					processor: "B".to_string(),
					metrics: vec![(2, 6000, 1), (3, 3000, 1), (4, 2500, 1)],
				},
				Action::RollToBlock {
					block_number: 302,
					expected_cycle: Cycle { epoch: 3, epoch_start: 302, era: 1, era_start: 302 },
				},
				// Commit to multiple pools with different metrics
				Action::CommitCompute {
					committer: "C".to_string(),
					stake: 5 * UNIT,
					cooldown: 36,
					metrics: vec![
						(2, 4000u128 * 4 / 5, 1u128), // pool 2
						(3, 2250u128 * 4 / 5, 1u128), // pool 3 (average of 1500 and 3000)
					],
					commission: Perbill::from_percent(10),
				},
				Action::Delegate {
					delegator: "D".to_string(),
					committer: "C".to_string(),
					amount: 40 * UNIT,
					cooldown: 36,
				},
				Action::Delegate {
					delegator: "E".to_string(),
					committer: "C".to_string(),
					amount: 5 * UNIT,
					cooldown: 36,
				},
				Action::Reward { amount: 10 * UNIT },
				Action::CooldownComputeCommitment { committer: "C".to_string() },
				Action::CooldownDelegation {
					delegator: "D".to_string(),
					committer: "C".to_string(),
				},
				Action::CooldownDelegation {
					delegator: "E".to_string(),
					committer: "C".to_string(),
				},
				Action::RollToBlock {
					block_number: 400,
					expected_cycle: Cycle { epoch: 3, epoch_start: 302, era: 1, era_start: 302 },
				},
				Action::EndComputeCommitment {
					committer: "C".to_string(),
					expected_reward: 950 * MILLIUNIT, // Actual reward with multi-pool metrics
				},
				Action::EndDelegation {
					delegator: "D".to_string(),
					committer: "C".to_string(),
					expected_reward: 3600 * MILLIUNIT, // Actual reward with multi-pool metrics
				},
				Action::EndDelegation {
					delegator: "E".to_string(),
					committer: "C".to_string(),
					expected_reward: 450 * MILLIUNIT, // Actual reward with multi-pool metrics
				},
			],
		);
	});
}

#[test]
fn test_compute_flow_large_metrics() {
	ExtBuilder.build().execute_with(|| {
		compute_test_flow(
			&[30, 30, 20, 20], // four pools with different allocations
			&[
				("C", &["A", "B"]), // committer C with processors A, B
			],
			&[
				// Use first part of commit_actions but with modified metrics for multi-pool test
				Action::RollToBlock {
					block_number: 10,
					expected_cycle: Cycle { epoch: 0, epoch_start: 2, era: 0, era_start: 2 },
				},
				Action::ProcessorCommit { processor: "A".to_string(), metrics: vec![(1, 1000, 1)] },
				Action::RollToBlock {
					block_number: 20,
					expected_cycle: Cycle { epoch: 0, epoch_start: 2, era: 0, era_start: 2 },
				},
				Action::ProcessorCommit { processor: "B".to_string(), metrics: vec![(1, 1000, 1)] },
				Action::RollToBlock {
					block_number: 150,
					expected_cycle: Cycle { epoch: 1, epoch_start: 102, era: 0, era_start: 2 },
				},
				// A commits to multiple pools
				Action::ProcessorCommit {
					processor: "A".to_string(),
					metrics: vec![(1, 89844839219, 1), (2, 89844839219, 1), (3, 89844839219, 1)],
				},
				// B commits to different pools
				Action::ProcessorCommit {
					processor: "B".to_string(),
					metrics: vec![(2, 89844839219, 1), (3, 89844839219, 1), (4, 89844839219, 1)],
				},
				Action::RollToBlock {
					block_number: 302,
					expected_cycle: Cycle { epoch: 3, epoch_start: 302, era: 1, era_start: 302 },
				},
				// Commit to multiple pools with different metrics
				Action::CommitCompute {
					committer: "C".to_string(),
					stake: 1_000_000_000 * UNIT,
					cooldown: 36,
					metrics: vec![
						(2, 89844839219 / 5 * 4, 1u128), // pool 2
						(3, 89844839219 / 5 * 4, 1u128), // pool 3 (average of 1500 and 3000)
					],
					commission: Perbill::from_percent(10),
				},
				Action::Delegate {
					delegator: "D".to_string(),
					committer: "C".to_string(),
					amount: 1_000_000_000 * UNIT,
					cooldown: 36,
				},
				Action::Delegate {
					delegator: "E".to_string(),
					committer: "C".to_string(),
					amount: 1_000_000_000 * UNIT,
					cooldown: 36,
				},
				Action::Reward { amount: 100_000_000_000 * UNIT },
				Action::CooldownComputeCommitment { committer: "C".to_string() },
				Action::CooldownDelegation {
					delegator: "D".to_string(),
					committer: "C".to_string(),
				},
				Action::CooldownDelegation {
					delegator: "E".to_string(),
					committer: "C".to_string(),
				},
				Action::RollToBlock {
					block_number: 400,
					expected_cycle: Cycle { epoch: 3, epoch_start: 302, era: 1, era_start: 302 },
				},
				Action::EndComputeCommitment {
					committer: "C".to_string(),
					expected_reward: 20_000_000_000 * UNIT,
				},
				Action::EndDelegation {
					delegator: "D".to_string(),
					committer: "C".to_string(),
					expected_reward: 15_000_000_000 * UNIT,
				},
				Action::EndDelegation {
					delegator: "E".to_string(),
					committer: "C".to_string(),
					expected_reward: 15_000_000_000 * UNIT,
				},
			],
		);
	});
}

#[test]
fn test_compute_flow_large_metrics_tiny_reward() {
	ExtBuilder.build().execute_with(|| {
		compute_test_flow(
			&[30, 30, 20, 20], // four pools with different allocations
			&[
				("C", &["A", "B"]), // committer C with processors A, B
			],
			&[
				// Use first part of commit_actions but with modified metrics for multi-pool test
				Action::RollToBlock {
					block_number: 10,
					expected_cycle: Cycle { epoch: 0, epoch_start: 2, era: 0, era_start: 2 },
				},
				Action::ProcessorCommit { processor: "A".to_string(), metrics: vec![(1, 1000, 1)] },
				Action::RollToBlock {
					block_number: 20,
					expected_cycle: Cycle { epoch: 0, epoch_start: 2, era: 0, era_start: 2 },
				},
				Action::ProcessorCommit { processor: "B".to_string(), metrics: vec![(1, 1000, 1)] },
				Action::RollToBlock {
					block_number: 150,
					expected_cycle: Cycle { epoch: 1, epoch_start: 102, era: 0, era_start: 2 },
				},
				// A commits to multiple pools
				Action::ProcessorCommit {
					processor: "A".to_string(),
					metrics: vec![(1, 89844839219, 1), (2, 89844839219, 1), (3, 89844839219, 1)],
				},
				// B commits to different pools
				Action::ProcessorCommit {
					processor: "B".to_string(),
					metrics: vec![(2, 89844839219, 1), (3, 89844839219, 1), (4, 89844839219, 1)],
				},
				Action::RollToBlock {
					block_number: 302,
					expected_cycle: Cycle { epoch: 3, epoch_start: 302, era: 1, era_start: 302 },
				},
				// Commit to multiple pools with different metrics
				Action::CommitCompute {
					committer: "C".to_string(),
					stake: 1_000_000_000 * UNIT,
					cooldown: 36,
					metrics: vec![
						(2, 89844839219 / 5 * 4, 1u128), // pool 2
						(3, 89844839219 / 5 * 4, 1u128), // pool 3 (average of 1500 and 3000)
					],
					commission: Perbill::from_percent(10),
				},
				Action::Delegate {
					delegator: "D".to_string(),
					committer: "C".to_string(),
					amount: 1_000_000_000 * UNIT,
					cooldown: 36,
				},
				Action::Delegate {
					delegator: "E".to_string(),
					committer: "C".to_string(),
					amount: 1_000_000_000 * UNIT,
					cooldown: 36,
				},
				Action::Reward { amount: 1 * MILLIUNIT },
				Action::CooldownComputeCommitment { committer: "C".to_string() },
				Action::CooldownDelegation {
					delegator: "D".to_string(),
					committer: "C".to_string(),
				},
				Action::CooldownDelegation {
					delegator: "E".to_string(),
					committer: "C".to_string(),
				},
				Action::RollToBlock {
					block_number: 400,
					expected_cycle: Cycle { epoch: 3, epoch_start: 302, era: 1, era_start: 302 },
				},
				Action::EndComputeCommitment {
					committer: "C".to_string(),
					expected_reward: 200 * MICROUNIT,
				},
				Action::EndDelegation {
					delegator: "D".to_string(),
					committer: "C".to_string(),
					expected_reward: 150 * MICROUNIT,
				},
				Action::EndDelegation {
					delegator: "E".to_string(),
					committer: "C".to_string(),
					expected_reward: 150 * MICROUNIT,
				},
			],
		);
	});
}

fn assert_delegator_withdrew_event(expected_event: Event<Test>) {
	let withdrew_events: Vec<_> = events()
		.into_iter()
		.filter_map(|e| match e {
			RuntimeEvent::Compute(Event::DelegatorWithdrew(delegator, cid, amount)) => {
				Some((delegator, cid, amount))
			},
			_ => None,
		})
		.collect();

	// Should have exactly one DelegatorWithdrew event
	assert_eq!(withdrew_events.len(), 1);
	let (event_delegator, event_commitment_id, event_reward_amount) = &withdrew_events[0];

	// Extract expected values from the input event
	if let Event::DelegatorWithdrew(expected_delegator, expected_cid, expected_amount) =
		expected_event
	{
		assert_eq!(
			(event_delegator.clone(), *event_commitment_id, *event_reward_amount),
			(expected_delegator, expected_cid, expected_amount)
		);
	} else {
		panic!("Expected DelegatorWithdrew event");
	}
}

#[test]
fn test_create_pools_name_conflict() {
	ExtBuilder.build().execute_with(|| {
		setup();
		// create pool 1
		{
			assert_ok!(Compute::create_pool(
				RuntimeOrigin::root(),
				*b"cpu-ops-per-second______",
				Perquintill::from_percent(25),
				None,
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
				None,
				bounded_vec![],
			),
			Error::<Test, ()>::PoolNameMustBeUnique
		);
	});
}

#[test]
fn test_single_processor_commit() {
	ExtBuilder.build().execute_with(|| {
		setup();
		assert_ok!(Compute::create_pool(
			RuntimeOrigin::root(),
			*b"cpu-ops-per-second______",
			Perquintill::from_percent(25),
			None,
			bounded_vec![],
		));
		assert_eq!(Compute::last_metric_pool_id(), 1);

		roll_to_block(10);
		assert_eq!(Compute::metrics(alice_account_id(), 1), None);
		assert_eq!(Compute::commit(&alice_account_id(), &[(1u8, 1000u128, 1u128)]), None);
		// With roll_to_block calling on_initialize for each block 1-10, epoch_offset changes
		assert_eq!(
			Compute::processors(alice_account_id()),
			Some(ProcessorState {
				epoch_offset: 8,
				committed: 0,
				claimed: 0,
				status: ProcessorStatus::WarmupUntil(40),
				accrued: 0,
				paid: 0
			})
		);

		roll_to_block(302 + 39);
		assert_eq!(Compute::commit(&alice_account_id(), &[(1u8, 1000u128, 1u128)]), None);
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
				accrued: 0,
				paid: 0
			})
		);

		assert_eq!(Compute::commit(&alice_account_id(), &[(1u8, 1000u128, 1u128)]), None);
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
				accrued: 0,
				paid: 0
			})
		);

		roll_to_block(302 + 130);
		assert_eq!(
			Compute::commit(&alice_account_id(), &[(1u8, 1000u128, 1u128)]),
			Some(642123287671233)
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
				claimed: 3,
				status: ProcessorStatus::Active,
				accrued: 0,
				paid: 642123287671233
			})
		);

		// commit different value in same epoch (does not change existing values for same epoch since first value is kept)
		roll_to_block(302 + 170);
		assert_eq!(Compute::commit(&alice_account_id(), &[(1u8, 2000u128, 1u128)]), None);
		assert_eq!(
			Compute::metrics(alice_account_id(), 1).unwrap(),
			MetricCommit { epoch: 4, metric: FixedU128::from_rational(1000u128, 1u128) }
		);
		assert_eq!(
			Compute::processors(alice_account_id()),
			Some(ProcessorState {
				epoch_offset: 8,
				committed: 4,
				claimed: 3,
				status: ProcessorStatus::Active,
				accrued: 0,
				paid: 642123287671233
			})
		);
		assert_eq!(
			Compute::metric_pools(1).unwrap().total.get(4),
			FixedU128::from_rational(1000u128, 1u128)
		);

		// claim for epoch 1 and commit for epoch 2
		roll_to_block(302 + 230);
		assert_eq!(
			Compute::commit(&alice_account_id(), &[(1u8, 1000u128, 1u128)]),
			/*Some(250000)*/ Some(642123287671233)
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
				claimed: 4,
				status: ProcessorStatus::Active,
				accrued: 0,
				//paid: 250000
				paid: 1284246575342466,
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
				max_stake_metric_ratio: Zero::zero(),
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
			None,
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
			None,
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
			None,
			bounded_vec![],
		));
		assert_eq!(Compute::last_metric_pool_id(), 3);
	}
}

fn commit_alice_bob() {
	// Alice commits first time
	{
		roll_to_block(10);
		assert_eq!(
			Compute::current_cycle(),
			Cycle { epoch: 0, epoch_start: 2, era: 0, era_start: 2 }
		);
		assert_eq!(Compute::metrics(alice_account_id(), 1), None);
		assert_eq!(Compute::commit(&alice_account_id(), &[(1u8, 1000u128, 1u128)]), None);
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
		assert_eq!(Compute::commit(&bob_account_id(), &[(1u8, 1000u128, 1u128)]), None);
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
			Compute::commit(&alice_account_id(), &[(1u8, 1000u128, 1u128), (2u8, 2000u128, 1u128)]),
			None
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
				accrued: 0,
				paid: 0
			})
		);
	}

	// Bob commits values for epoch 1 (where he is active) for only pool 2
	{
		assert_eq!(Compute::commit(&bob_account_id(), &[(2u8, 6000u128, 1u128)]), None);
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
				accrued: 0,
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
			1,
			None,
			Some((2, Perquintill::from_percent(35))),
			None,
			None
		));
	}

	// Charlie commits first time (to all pools)
	if with_charlie {
		roll_to_block(190);
		assert_eq!(Compute::metrics(charlie_account_id(), 1), None);
		assert_eq!(
			Compute::commit(
				&charlie_account_id(),
				&[(1u8, 1234u128, 10u128), (2u8, 1234u128, 10u128), (3u8, 1234u128, 10u128)]
			),
			None
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
				&[(1u8, 1234u128, 10u128), (2u8, 1234u128, 10u128), (3u8, 1234u128, 10u128)]
			),
			None
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
			Compute::commit(&alice_account_id(), &[(1u8, 1000u128, 1u128), (2u8, 2000u128, 1u128)]),
			/*Some(375000)*/ Some(963184931506849),
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
				claimed: 1,
				status: ProcessorStatus::Active,
				accrued: 0,
				//paid: 375000
				paid: 963184931506849,
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
			Compute::commit(&bob_account_id(), &[(1u8, 1000u128, 1u128), (2u8, 2000u128, 1u128)]),
			/*Some(375000)*/ Some(963184931506849),
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
				claimed: 1,
				status: ProcessorStatus::Active,
				accrued: 0,
				//paid: 375000
				paid: 963184931506849,
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
				max_stake_metric_ratio: Zero::zero(),
			},
		)),
		RuntimeEvent::Compute(Event::PoolCreated(
			2,
			MetricPool {
				config: bounded_vec![],
				name: *b"mem-read-count-per-sec--",
				reward: ProvisionalBuffer::from_inner(Perquintill::from_percent(50), None),
				total: SlidingBuffer::from_inner(0u64, 0.into(), 0.into()),
				max_stake_metric_ratio: Zero::zero(),
			},
		)),
		RuntimeEvent::Compute(Event::PoolCreated(
			3,
			MetricPool {
				config: bounded_vec![],
				name: *b"mem-write-count-per-sec-",
				reward: ProvisionalBuffer::from_inner(Perquintill::from_percent(25), None),
				total: SlidingBuffer::from_inner(0u64, 0.into(), 0.into()),
				max_stake_metric_ratio: Zero::zero(),
			},
		)),
	];
	assert!(expected.iter().all(|event| events.contains(event)));
}

#[test]
fn test_multiple_processor_commit() {
	ExtBuilder.build().execute_with(|| {
		// Use test helpers to replicate test below with interleaving Charlie's commit (should return same rewards for Alice and Bob)
		setup();
		create_pools();
		commit(false, false);
		check_events();
	});
}

#[test]
fn test_multiple_processor_commit_reward_modified() {
	ExtBuilder.build().execute_with(|| {
		// Use test helpers to replicate test below with interleaving Charlie's commit (should return same rewards for Alice and Bob)
		setup();
		create_pools();
		commit(false, true);
		check_events();
	});
}

#[test]
fn test_multiple_processor_commit_with_interleaving_charlie() {
	ExtBuilder.build().execute_with(|| {
		// Use test helpers to replicate test below with interleaving Charlie's commit (should return same rewards for Alice and Bob)
		setup();
		create_pools();
		commit(true, false);
		check_events();
	});
}

#[test]
fn test_multiple_processor_commit_with_interleaving_charlie_reward_modified() {
	ExtBuilder.build().execute_with(|| {
		// Use test helpers to replicate test below with interleaving Charlie's commit (should return same rewards for Alice and Bob)
		setup();
		create_pools();
		commit(true, true);
		check_events();
	});
}

#[test]
fn test_commit_compute() {
	ExtBuilder.build().execute_with(|| {
		setup();
		create_pools();

		// Charlie will act as both manager and committer (same account for simplicity)
		let charlie = charlie_account_id();

		offer_accept_backing(charlie.clone());

		commit_alice_bob();

		const MANAGER_ID: u128 = 1;

		// pool 1 has only commits in warmup, not counting towards average
		assert_eq!(
			Compute::metrics_era_average(MANAGER_ID, 1), // pool 1
			None
		);
		assert_eq!(
			Compute::metrics_era_average(MANAGER_ID, 2).unwrap(), // pool 2
			SlidingBuffer::from_inner(
				0,
				(Zero::zero(), 0),                              // prev
				(FixedU128::from_rational(4000u128, 1u128), 2)  // cur
			)
		);

		// Step 3: Setup initial balance for Charlie to cover the stake amount
		assert_ok!(Balances::force_set_balance(RuntimeOrigin::root(), charlie.clone(), 100 * UNIT));

		// Step 4: Charlie commits compute (acting as committer backing his own manager account)
		// Start with minimal metrics to test if validation passes
		let exceeding_commitment = bounded_vec![ComputeCommitment {
			pool_id: 2,
			metric: FixedU128::from_rational(4000u128 * 4 / 5 + 1, 1u128), // Maximal possible commitment value + 1
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

		roll_to_block(302);

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

		assert_eq!(
			Compute::delegation_pools(0),
			DelegationPool {
				reward_weight: U256::from(1666666666666u64),
				slash_weight: U256::from(1666666666666u64),
				reward_per_token: U256::zero(),
				slash_per_token: U256::zero(),
			}
		);

		// Verify the commit was successful by checking events or storage
		// At minimum we should see the commitment created event
		assert!(events()
			.iter()
			.any(|e| matches!(e, RuntimeEvent::Compute(Event::CommitmentCreated(_, _)))));

		assert_ok!(Compute::reward(RuntimeOrigin::root(), 10 * UNIT));

		assert_eq!(
			Compute::staking_pool_members(0, 2).unwrap(),
			StakingPoolMember {
				reward_weight: U256::from(10666666666666666u64),
				reward_debt: U256::zero()
			}
		);
		assert_eq!(
			Compute::staking_pools(2),
			StakingPool {
				reward_weight: U256::from(10666666666666666u64),
				reward_per_token: U256::from(468750000000000029296875000u128)
			}
		);

		roll_to_block(310);
		assert_ok!(Compute::cooldown_compute_commitment(RuntimeOrigin::signed(charlie.clone()),));

		assert_eq!(
			Compute::staking_pool_members(0, 2).unwrap(),
			StakingPoolMember {
				reward_weight: U256::from(5333333333333333u64),
				reward_debt: U256::from(2500000000000u64)
			}
		);
		assert_eq!(
			Compute::staking_pools(2),
			StakingPool {
				reward_weight: U256::from(5333333333333333u64),
				reward_per_token: U256::from(468750000000000029296875000u128)
			}
		);
		assert_eq!(
			Compute::delegation_pools(0),
			DelegationPool {
				reward_weight: U256::from(833333333333u64),
				slash_weight: U256::from(1666666666666u64),
				reward_per_token: U256::from(3000000000000600000000000240000u128),
				slash_per_token: U256::zero(),
			}
		);
		assert_eq!(
			Compute::self_delegation(0).unwrap(),
			DelegationPoolMember {
				reward_weight: U256::from(833333333333u64),
				slash_weight: U256::from(1666666666666u64),
				reward_debt: 2500000000000,
				slash_debt: 0,
			}
		);
		assert_eq!(Compute::stakes(0).unwrap().accrued_reward, 4999999999998);

		roll_to_block(345);
		assert_err!(
			Compute::end_compute_commitment(RuntimeOrigin::signed(charlie.clone())),
			Error::<Test, ()>::CooldownNotEnded
		);

		roll_to_block(346);
		assert_ok!(Compute::end_compute_commitment(RuntimeOrigin::signed(charlie.clone()),));

		// Verify the reward was payed out
		// At minimum we should see the commitment created event
		// assert_eq!(events(), []);
		assert!(events().iter().any(|e| matches!(
			e,
			RuntimeEvent::Balances(pallet_balances::Event::Transfer {
				from: _,
				to: _,
				amount: 4999999999998
			})
		)));
	});
}

#[test]
fn test_delegate() {
	ExtBuilder.build().execute_with(|| {
		setup();
		create_pools();
		assert_ok!(Balances::force_set_balance(
			RuntimeOrigin::root(),
			<Test as Config>::PalletId::get().into_account_truncating(),
			110_000_000
		));

		// Charlie will act as both manager and committer (same account for simplicity)
		let committer = charlie_account_id();

		offer_accept_backing(committer.clone());
		commit_alice_bob();
		roll_to_block(302);
		commit_compute(committer.clone());

		let delegator_1 = ferdie_account_id();
		let delegator_2 = george_account_id();
		let initial_balance = 100 * UNIT;
		assert_ok!(Balances::force_set_balance(
			RuntimeOrigin::root(),
			delegator_1.clone(),
			initial_balance
		));
		assert_ok!(Balances::force_set_balance(
			RuntimeOrigin::root(),
			delegator_2.clone(),
			initial_balance
		));

		let stake_amount_too_much = 50 * UNIT; // 5 tokens
		let stake_amount_1 = 40 * UNIT; // 5 tokens
		let stake_amount_2_too_much = 6 * UNIT; // 5 tokens
		let stake_amount_2 = 5 * UNIT; // 5 tokens
		let cooldown_period = 36u64; // 1000 blocks
		let allow_auto_compound = true;

		{
			assert_err!(
				Compute::delegate(
					RuntimeOrigin::signed(delegator_1.clone()),
					committer.clone(),
					stake_amount_too_much,
					cooldown_period,
					allow_auto_compound,
				),
				Error::<Test, ()>::MaxDelegationRatioExceeded
			);
			assert_ok!(Compute::delegate(
				RuntimeOrigin::signed(delegator_1.clone()),
				committer.clone(),
				stake_amount_1,
				cooldown_period,
				allow_auto_compound,
			));
			// After delegation, the stake should be locked
			assert_eq!(Balances::usable_balance(&delegator_1), initial_balance - stake_amount_1);
			// At minimum we should see the delegation event
			assert!(events()
				.iter()
				.any(|e| matches!(e, RuntimeEvent::Compute(Event::Delegated(_, _)))));
		}

		{
			assert_err!(
				Compute::delegate(
					RuntimeOrigin::signed(delegator_2.clone()),
					committer.clone(),
					stake_amount_2_too_much,
					cooldown_period,
					allow_auto_compound,
				),
				Error::<Test, ()>::MaxDelegationRatioExceeded
			);

			assert_ok!(Compute::delegate(
				RuntimeOrigin::signed(delegator_2.clone()),
				committer.clone(),
				stake_amount_2,
				cooldown_period,
				allow_auto_compound,
			));
			// After delegation, the stake should be locked
			assert_eq!(Balances::usable_balance(&delegator_2), initial_balance - stake_amount_2);
			// At minimum we should see the delegation event
			assert!(events()
				.iter()
				.any(|e| matches!(e, RuntimeEvent::Compute(Event::Delegated(_, _)))));
		}

		assert_ok!(Compute::reward(RuntimeOrigin::root(), 10 * UNIT));
		assert_ok!(Compute::withdraw_delegation(
			RuntimeOrigin::signed(delegator_2.clone()),
			committer.clone()
		));

		// Get the commitment ID for precise event assertion
		let commitment_id = AcurastCommitmentIdProvider::commitment_id_for(&committer).unwrap();

		// Assert the DelegatorWithdrew event with precise values
		// 5 own stake, delegator 1: 40, delegator 2: 5
		assert_delegator_withdrew_event(Event::DelegatorWithdrew(
			delegator_2.clone(),
			commitment_id,
			450 * MILLIUNIT,
		));

		assert_ok!(Compute::withdraw_delegation(
			RuntimeOrigin::signed(delegator_1.clone()),
			committer.clone()
		));

		assert_delegator_withdrew_event(Event::DelegatorWithdrew(
			delegator_1.clone(),
			commitment_id,
			3600 * MILLIUNIT,
		));

		assert_eq!(
			Balances::usable_balance(&delegator_1),
			initial_balance - stake_amount_1 + 3600 * MILLIUNIT
		);
		assert_eq!(
			Balances::usable_balance(&delegator_2),
			initial_balance - stake_amount_2 + 450 * MILLIUNIT
		);
	});
}

#[test]
fn test_delegate_more() {
	ExtBuilder.build().execute_with(|| {
		setup();
		create_pools();

		// Charlie will act as both manager and committer (same account for simplicity)
		let committer = charlie_account_id();

		offer_accept_backing(committer.clone());
		commit_alice_bob();
		assert_eq!(Compute::current_cycle().era_start, 2);
		assert_eq!(Compute::current_cycle().era, 0);
		commit_compute(committer.clone());

		let delegator_1 = ferdie_account_id();
		let delegator_2 = george_account_id();
		let initial_balance = 100 * UNIT;
		assert_ok!(Balances::force_set_balance(
			RuntimeOrigin::root(),
			delegator_1.clone(),
			initial_balance
		));
		assert_ok!(Balances::force_set_balance(
			RuntimeOrigin::root(),
			delegator_2.clone(),
			initial_balance
		));

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
			assert_eq!(Balances::usable_balance(&delegator_1), initial_balance - stake_amount_1);
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
			assert_eq!(Balances::usable_balance(&delegator_2), initial_balance - stake_amount_2);
			// At minimum we should see the delegation event
			assert!(events()
				.iter()
				.any(|e| matches!(e, RuntimeEvent::Compute(Event::Delegated(_, _)))));
		}

		assert_ok!(Compute::reward(RuntimeOrigin::root(), 10 * UNIT));

		let stake_amount_2b = 15 * UNIT; // makes it a total of 20 for delegator_2
		{
			assert_ok!(Compute::delegate_more(
				RuntimeOrigin::signed(delegator_2.clone()),
				committer.clone(),
				stake_amount_2b,
			));
			// After delegation, the stake should be locked
			let expected = initial_balance - stake_amount_2 - stake_amount_2b;
			assert!(Balances::usable_balance(&delegator_2) - expected < UNIT);
			// At minimum we should see the delegation event
			assert!(events()
				.iter()
				.any(|e| matches!(e, RuntimeEvent::Compute(Event::DelegatedMore(_, _)))));
		}
	});
}

#[test]
fn test_compound_delegation() {
	ExtBuilder.build().execute_with(|| {
		setup();
		create_pools();

		// Charlie will act as both manager and committer
		let committer = charlie_account_id();
		offer_accept_backing(committer.clone());
		commit_alice_bob();
		roll_to_block(302);
		commit_compute(committer.clone());

		let delegator = ferdie_account_id();
		let initial_balance = 100 * UNIT;
		assert_ok!(Balances::force_set_balance(
			RuntimeOrigin::root(),
			delegator.clone(),
			initial_balance
		));

		let stake_amount = 30 * UNIT;
		let cooldown_period = 36u64;
		let allow_auto_compound = true;

		// Get the commitment ID for precise event assertion
		let commitment_id = AcurastCommitmentIdProvider::commitment_id_for(&committer).unwrap();

		// Initial delegation
		assert_ok!(Compute::delegate(
			RuntimeOrigin::signed(delegator.clone()),
			committer.clone(),
			stake_amount,
			cooldown_period,
			allow_auto_compound,
		));

		// Add some rewards to the system
		assert_ok!(Compute::reward(RuntimeOrigin::root(), 5 * UNIT));

		// The staked amount should increase
		assert_eq!(
			Compute::delegations(delegator.clone(), commitment_id).unwrap().amount,
			stake_amount
		);

		// Compound the delegation rewards (compound_delegation takes committer and optional delegator)
		assert_ok!(Compute::compound_delegation(
			RuntimeOrigin::signed(delegator.clone()),
			committer.clone(),
			None, // delegator defaults to caller
		));

		// The staked amount should increase
		assert_eq!(
			Compute::delegations(delegator.clone(), commitment_id).unwrap().amount,
			31928571428571
		);
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

fn commit_compute(who: AccountId32) {
	const MANAGER_ID: u128 = 1;

	// pool 1 has only commits in warmup, not counting towards average
	assert_eq!(
		Compute::metrics_era_average(MANAGER_ID, 1), // pool 1
		None
	);
	assert_eq!(
		Compute::metrics_era_average(MANAGER_ID, 2).unwrap(), // pool 2
		SlidingBuffer::from_inner(
			0,
			(Zero::zero(), 0),                              // prev
			(FixedU128::from_rational(4000u128, 1u128), 2)  // cur
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
