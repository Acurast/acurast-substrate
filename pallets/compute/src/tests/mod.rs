#![allow(clippy::erasing_op)]

pub mod test_actions;

pub use test_actions::{compute_test_flow, events, roll_to_block, setup_balances, Action};

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
			block_number: 302, // skipping epoch 2 since average is taken from last era (not epoch)
			expected_cycle: Cycle { epoch: 3, epoch_start: 302 },
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
			block_number: 302, // skipping epoch 2 since average is taken from last era (not epoch)
			expected_cycle: Cycle { epoch: 3, epoch_start: 302 },
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
			block_number: 602, // skipping epoch 2 since average is taken from last era (not epoch)
			expected_cycle: Cycle { epoch: 6, epoch_start: 602 },
		},
	]
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
				accrued: 0,
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
				accrued: 0,
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
				accrued: 0,
				paid: 0
			})
		);

		roll_to_block(302 + 130);
		assert_eq!(
			Compute::commit(&alice_account_id(), &manager, &[(1u8, 1000u128, 1u128)]).0,
			642123287671233
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
				accrued: 0,
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
				accrued: 0,
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
			642123287671233
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
				accrued: 0,
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
				accrued: 0,
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
			1926369863013699,
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
				accrued: 0,
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
				accrued: 0,
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
				(U256::from(73029674), U256::from(73029674))
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
fn test_commit_compute_with_slash() {
	ExtBuilder.build().execute_with(|| {
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

		// roll to block where delegator_2's cooldown is over
		roll_to_block(438);
		assert_eq!(Compute::current_cycle(), Cycle { epoch: 4, epoch_start: 402 });

		assert_ok!(Compute::end_delegation(
			RuntimeOrigin::signed(delegator_2.clone()),
			committer.clone()
		));

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

#[test]
fn test_delegate_more() {
	ExtBuilder.build().execute_with(|| {
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
