use frame_support::{assert_err, assert_ok, traits::Hooks};
use sp_core::bounded_vec;
use sp_runtime::{
	traits::{AccountIdConversion, Zero},
	AccountId32, FixedU128, Perbill, Perquintill,
};

use crate::{
	datastructures::{ProvisionalBuffer, SlidingBuffer},
	mock::*,
	stub::*,
	types::*,
	Config, Error, Event,
};
use acurast_common::{CommitmentIdProvider, ComputeHooks, ManagerIdProvider};

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
				epoch_offset: 8, // Updated to match new behavior
				committed: 0,
				claimed: 0,
				status: ProcessorStatus::WarmupUntil(40),
				accrued: 0,
				paid: 0
			})
		);

		roll_to_block(39);
		assert_eq!(Compute::commit(&alice_account_id(), &[(1u8, 1000u128, 1u128)]), None);
		assert_eq!(
			Compute::metrics(alice_account_id(), 1).unwrap(),
			MetricCommit { epoch: 0, metric: FixedU128::from_rational(1000u128, 1u128) }
		);
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

		// Warmup is over
		roll_to_block(40);
		assert_eq!(Compute::commit(&alice_account_id(), &[(1u8, 1000u128, 1u128)]), None);
		assert_eq!(
			Compute::metrics(alice_account_id(), 1).unwrap(),
			MetricCommit { epoch: 0, metric: FixedU128::from_rational(1000u128, 1u128) }
		);
		assert_eq!(
			Compute::processors(alice_account_id()),
			Some(ProcessorState {
				epoch_offset: 8,
				committed: 0,
				claimed: 0,
				status: ProcessorStatus::Active,
				accrued: 0,
				paid: 0
			})
		);

		roll_to_block(130);
		assert_eq!(Compute::commit(&alice_account_id(), &[(1u8, 1000u128, 1u128)]), None);
		assert_eq!(
			Compute::metrics(alice_account_id(), 1).unwrap(),
			MetricCommit { epoch: 1, metric: FixedU128::from_rational(1000u128, 1u128) }
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

		// commit different value in same epoch (does not change existing values for same epoch since first value is kept)
		roll_to_block(170);
		assert_eq!(Compute::commit(&alice_account_id(), &[(1u8, 2000u128, 1u128)]), None);
		assert_eq!(
			Compute::metrics(alice_account_id(), 1).unwrap(),
			MetricCommit { epoch: 1, metric: FixedU128::from_rational(1000u128, 1u128) }
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
		assert_eq!(
			Compute::metric_pools(1).unwrap().total.get(1),
			FixedU128::from_rational(1000u128, 1u128)
		);

		// claim for epoch 1 and commit for epoch 2
		roll_to_block(230);
		assert_eq!(
			Compute::commit(&alice_account_id(), &[(1u8, 1000u128, 1u128)]),
			/*Some(250000)*/ Some(642123287671233)
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
				//paid: 250000
				paid: 642123287671233,
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

fn roll_to_block(block_number: u64) {
	let current_block = System::block_number();
	for block in current_block + 1..=block_number {
		System::set_block_number(block);
		Compute::on_initialize(block);
	}
}

fn setup() {
	assert_ok!(Balances::force_set_balance(RuntimeOrigin::root(), eve_account_id(), 110_000_000));
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
	// We use block 150 to ensure the epoch is passed epoch 0 to distinguish from default epoch value
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
			metric: FixedU128::from_rational(4000u128 * 4 / 5 + 1, 1u128), // Minimal value
		},];

		let commitment: sp_runtime::BoundedVec<ComputeCommitment, sp_core::ConstU32<30>> =
			bounded_vec![ComputeCommitment {
				pool_id: 2,
				metric: FixedU128::from_rational(4000u128 * 4 / 5, 1u128), // Minimal value
			},];

		let stake_amount = 5 * UNIT; // 5 tokens
		let cooldown_period = 36u64; // 1000 blocks
		let commission = Perbill::from_percent(10); // 10% commission
		let allow_auto_compound = true;

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
		let events = events();
		// At minimum we should see the commitment created event
		assert!(events
			.iter()
			.any(|e| matches!(e, RuntimeEvent::Compute(Event::CommitmentCreated(_, _)))));
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
			495999u128, // ~0.5
		));

		assert_ok!(Compute::withdraw_delegation(
			RuntimeOrigin::signed(delegator_1.clone()),
			committer.clone()
		));

		assert_delegator_withdrew_event(Event::DelegatorWithdrew(
			delegator_1.clone(),
			commitment_id,
			3967992u128, // ~4
		));

		assert_eq!(
			Balances::usable_balance(&delegator_1),
			initial_balance - stake_amount_1 + 3967992u128
		);
		assert_eq!(
			Balances::usable_balance(&delegator_2),
			initial_balance - stake_amount_2 + 495999u128
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
			assert_eq!(
				Balances::usable_balance(&delegator_2),
				initial_balance - stake_amount_2 - stake_amount_2b + 744799
			);
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
			32217591
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
			metric: FixedU128::from_rational(4000u128 * 4 / 5, 1u128), // Minimal value
		},];

	let stake_amount = 5 * UNIT; // 5 tokens
	let cooldown_period = 36u64; // 1000 blocks
	let commission = Perbill::from_percent(10); // 10% commission
	let allow_auto_compound = true;

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
