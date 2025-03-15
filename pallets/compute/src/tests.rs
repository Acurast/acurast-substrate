#![cfg(test)]

use frame_support::{assert_err, assert_ok, weights::WeightMeter};
use sp_core::bounded_vec;
use sp_runtime::{FixedU128, Perquintill};

use crate::{
	datastructures::{ProvisionalBuffer, SlidingBuffer},
	mock::*,
	stub::*,
	types::*,
	Error, Event,
};
use acurast_common::ComputeHooks;

#[test]
fn test_create_pools_name_conflict() {
	ExtBuilder::default().build().execute_with(|| {
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
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(Compute::create_pool(
			RuntimeOrigin::root(),
			*b"cpu-ops-per-second______",
			Perquintill::from_percent(25),
			bounded_vec![],
		));
		assert_eq!(Compute::last_metric_pool_id(), 1);

		System::set_block_number(10);
		assert_eq!(Compute::metrics(alice_account_id(), 1), None);
		assert_eq!(
			Compute::commit(
				&alice_account_id(),
				vec![(1u8, 1000u128, 1u128)],
				&mut WeightMeter::new()
			),
			None
		);
		assert_eq!(
			Compute::processors(alice_account_id()),
			Some(ProcessorState {
				epoch_offset: 10,
				committed: 0,
				claimed: 0,
				status: ProcessorStatus::WarmupUntil(40),
				accrued: 0,
				paid: 0
			})
		);

		System::set_block_number(39);
		assert_eq!(
			Compute::commit(
				&alice_account_id(),
				vec![(1u8, 1000u128, 1u128)],
				&mut WeightMeter::new()
			),
			None
		);
		assert_eq!(
			Compute::metrics(alice_account_id(), 1).unwrap(),
			MetricCommit { epoch: 0, metric: FixedU128::from_rational(1000u128, 1u128) }
		);
		assert_eq!(
			Compute::processors(alice_account_id()),
			Some(ProcessorState {
				epoch_offset: 10,
				committed: 0,
				claimed: 0,
				status: ProcessorStatus::WarmupUntil(40),
				accrued: 0,
				paid: 0
			})
		);

		// Warmup is over
		System::set_block_number(40);
		assert_eq!(
			Compute::commit(
				&alice_account_id(),
				vec![(1u8, 1000u128, 1u128)],
				&mut WeightMeter::new()
			),
			None
		);
		assert_eq!(
			Compute::metrics(alice_account_id(), 1).unwrap(),
			MetricCommit { epoch: 0, metric: FixedU128::from_rational(1000u128, 1u128) }
		);
		assert_eq!(
			Compute::processors(alice_account_id()),
			Some(ProcessorState {
				epoch_offset: 10,
				committed: 0,
				claimed: 0,
				status: ProcessorStatus::Active,
				accrued: 0,
				paid: 0
			})
		);

		System::set_block_number(130);
		assert_eq!(
			Compute::commit(
				&alice_account_id(),
				vec![(1u8, 1000u128, 1u128)],
				&mut WeightMeter::new()
			),
			None
		);
		assert_eq!(
			Compute::metrics(alice_account_id(), 1).unwrap(),
			MetricCommit { epoch: 1, metric: FixedU128::from_rational(1000u128, 1u128) }
		);
		assert_eq!(
			Compute::processors(alice_account_id()),
			Some(ProcessorState {
				epoch_offset: 10,
				committed: 1,
				claimed: 0,
				status: ProcessorStatus::Active,
				accrued: 0,
				paid: 0
			})
		);

		// commit different value in same epoch (does not existing values for same epoch since first value is kept)
		System::set_block_number(170);
		assert_eq!(
			Compute::commit(
				&alice_account_id(),
				vec![(1u8, 2000u128, 1u128)],
				&mut WeightMeter::new()
			),
			None
		);
		assert_eq!(
			Compute::metrics(alice_account_id(), 1).unwrap(),
			MetricCommit { epoch: 1, metric: FixedU128::from_rational(1000u128, 1u128) }
		);
		assert_eq!(
			Compute::processors(alice_account_id()),
			Some(ProcessorState {
				epoch_offset: 10,
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
		System::set_block_number(230);
		assert_eq!(
			Compute::commit(
				&alice_account_id(),
				vec![(1u8, 1000u128, 1u128)],
				&mut WeightMeter::new()
			),
			Some(250000)
		);
		assert_eq!(
			Compute::metrics(alice_account_id(), 1).unwrap(),
			MetricCommit { epoch: 2, metric: FixedU128::from_rational(1000u128, 1u128) }
		);
		assert_eq!(
			Compute::processors(alice_account_id()),
			Some(ProcessorState {
				epoch_offset: 10,
				committed: 2,
				claimed: 1,
				status: ProcessorStatus::Active,
				accrued: 0,
				paid: 250000
			})
		);

		assert_eq!(
			events(),
			[
				RuntimeEvent::Compute(Event::PoolCreated(
					1,
					MetricPool {
						config: bounded_vec![],
						name: *b"cpu-ops-per-second______",
						reward: ProvisionalBuffer::from_inner(Perquintill::from_percent(25), None,),
						total: SlidingBuffer::from_inner(0u64, 0.into(), 0.into()),
					}
				)),
				RuntimeEvent::MockPallet(mock_pallet::Event::CalculateReward(
					Perquintill::from_percent(25),
					1
				)),
				RuntimeEvent::MockPallet(mock_pallet::Event::DistributeReward(
					alice_account_id(),
					250000
				))
			]
		);
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

fn commit(with_charlie: bool, modify_reward: bool) {
	// Alice commits first time
	{
		System::set_block_number(10);
		assert_eq!(Compute::metrics(alice_account_id(), 1), None);
		assert_eq!(
			Compute::commit(
				&alice_account_id(),
				vec![(1u8, 1000u128, 1u128)],
				&mut WeightMeter::new()
			),
			None
		);
		assert_eq!(
			Compute::processors(alice_account_id()).unwrap().status,
			ProcessorStatus::WarmupUntil(40)
		);
		assert_eq!(Compute::processors(alice_account_id()).unwrap().epoch_offset, 10);
	}

	// Bob commits first time
	{
		System::set_block_number(20);
		assert_eq!(Compute::metrics(bob_account_id(), 1), None);
		assert_eq!(
			Compute::commit(
				&bob_account_id(),
				vec![(1u8, 1000u128, 1u128)],
				&mut WeightMeter::new()
			),
			None
		);
		assert_eq!(
			Compute::processors(bob_account_id()).unwrap().status,
			ProcessorStatus::WarmupUntil(50)
		);
		assert_eq!(Compute::processors(bob_account_id()).unwrap().epoch_offset, 20);
	}

	// Warmup is over for both Alice and Bob so this commits is rewardable since they commit for an active epoch
	// We use block 150 to ensure the epoch is passed epoch 0 to distinguish from default epoch value
	System::set_block_number(150);

	// Alice commits values for epoch 1 (where she is active) for pool 1 and 2
	{
		assert_eq!(
			Compute::commit(
				&alice_account_id(),
				vec![(1u8, 1000u128, 1u128), (2u8, 2000u128, 1u128)],
				&mut WeightMeter::new()
			),
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
				epoch_offset: 10,
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
			Compute::commit(
				&bob_account_id(),
				vec![(2u8, 6000u128, 1u128)],
				&mut WeightMeter::new()
			),
			None
		);
		assert_eq!(
			Compute::metrics(bob_account_id(), 2).unwrap(),
			MetricCommit { epoch: 1, metric: FixedU128::from_rational(6000u128, 1u128) }
		);
		assert_eq!(
			Compute::processors(bob_account_id()),
			Some(ProcessorState {
				epoch_offset: 20,
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

	// An admin changes the reward from now on (should not influence rewards for epoch 1)
	if modify_reward {
		assert_ok!(Compute::modify_pool(
			RuntimeOrigin::root(),
			1,
			None,
			Some((2, Perquintill::from_percent(35))),
			None,
		));
	}

	// Charlie commits first time (to all pools)
	if with_charlie {
		System::set_block_number(190);
		assert_eq!(Compute::metrics(charlie_account_id(), 1), None);
		assert_eq!(
			Compute::commit(
				&charlie_account_id(),
				vec![(1u8, 1234u128, 10u128), (2u8, 1234u128, 10u128), (3u8, 1234u128, 10u128)],
				&mut WeightMeter::new()
			),
			None
		);
		assert_eq!(
			Compute::processors(charlie_account_id()).unwrap().status,
			ProcessorStatus::WarmupUntil(220)
		);
		assert_eq!(Compute::processors(charlie_account_id()).unwrap().epoch_offset, 90);
	}

	// Only Alice entered her individual epoch 2 and can claim for epoch 1
	System::set_block_number(210);

	// Charlie commits values for epoch 2 (where he is active) for all pools, but should not disturb the reward payment below for epoch 1 for Alice and Bob
	if with_charlie {
		assert_eq!(
			Compute::metrics(charlie_account_id(), 1).unwrap(),
			MetricCommit { epoch: 1, metric: FixedU128::from_rational(1234u128, 10u128) }
		);
		assert_eq!(
			Compute::commit(
				&charlie_account_id(),
				vec![(1u8, 1234u128, 10u128), (2u8, 1234u128, 10u128), (3u8, 1234u128, 10u128)],
				&mut WeightMeter::new()
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
			Compute::commit(
				&alice_account_id(),
				vec![(1u8, 1000u128, 1u128), (2u8, 2000u128, 1u128)],
				&mut WeightMeter::new()
			),
			Some(375000)
		);
		assert_eq!(
			Compute::metrics(alice_account_id(), 1).unwrap(),
			MetricCommit { epoch: 2, metric: FixedU128::from_rational(1000u128, 1u128) }
		);
		assert_eq!(
			Compute::processors(alice_account_id()),
			Some(ProcessorState {
				epoch_offset: 10,
				committed: 2,
				claimed: 1,
				status: ProcessorStatus::Active,
				accrued: 0,
				paid: 375000
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
				vec![(1u8, 1000u128, 1u128), (2u8, 2000u128, 1u128)],
				&mut WeightMeter::new()
			),
			Some(375000)
		);
		assert_eq!(
			Compute::metrics(bob_account_id(), 1).unwrap(),
			MetricCommit { epoch: 2, metric: FixedU128::from_rational(1000u128, 1u128) }
		);
		assert_eq!(
			Compute::processors(bob_account_id()),
			Some(ProcessorState {
				epoch_offset: 20,
				committed: 2,
				claimed: 1,
				status: ProcessorStatus::Active,
				accrued: 0,
				paid: 375000
			})
		);
	}
}

fn check_events() {
	assert_eq!(
		events(),
		[
			RuntimeEvent::Compute(Event::PoolCreated(
				1,
				MetricPool {
					config: bounded_vec![],
					name: *b"cpu-ops-per-second______",
					reward: ProvisionalBuffer::from_inner(Perquintill::from_percent(25), None,),
					total: SlidingBuffer::from_inner(0u64, 0.into(), 0.into()),
				}
			)),
			RuntimeEvent::Compute(Event::PoolCreated(
				2,
				MetricPool {
					config: bounded_vec![],
					name: *b"mem-read-count-per-sec--",
					reward: ProvisionalBuffer::from_inner(Perquintill::from_percent(50), None,),
					total: SlidingBuffer::from_inner(0u64, 0.into(), 0.into()),
				}
			)),
			RuntimeEvent::Compute(Event::PoolCreated(
				3,
				MetricPool {
					config: bounded_vec![],
					name: *b"mem-write-count-per-sec-",
					reward: ProvisionalBuffer::from_inner(Perquintill::from_percent(25), None,),
					total: SlidingBuffer::from_inner(0u64, 0.into(), 0.into()),
				}
			)),
			RuntimeEvent::MockPallet(mock_pallet::Event::CalculateReward(
				Perquintill::from_perthousand(375),
				1
			)),
			RuntimeEvent::MockPallet(mock_pallet::Event::DistributeReward(
				alice_account_id(),
				375000
			)),
			RuntimeEvent::MockPallet(mock_pallet::Event::CalculateReward(
				Perquintill::from_perthousand(375),
				1
			)),
			RuntimeEvent::MockPallet(mock_pallet::Event::DistributeReward(
				bob_account_id(),
				375000
			))
		]
	);
}

#[test]
fn test_multiple_processor_commit() {
	ExtBuilder::default().build().execute_with(|| {
		// Use test helpers to replicate test below with interleaving Charlie's commit (should return same rewards for Alice and Bob)
		create_pools();
		commit(false, false);
		check_events();
	});
}

#[test]
fn test_multiple_processor_commit_reward_modified() {
	ExtBuilder::default().build().execute_with(|| {
		// Use test helpers to replicate test below with interleaving Charlie's commit (should return same rewards for Alice and Bob)
		create_pools();
		commit(false, true);
		check_events();
	});
}

#[test]
fn test_multiple_processor_commit_with_interleaving_charlie() {
	ExtBuilder::default().build().execute_with(|| {
		// Use test helpers to replicate test below with interleaving Charlie's commit (should return same rewards for Alice and Bob)
		create_pools();
		commit(true, false);
		check_events();
	});
}

#[test]
fn test_multiple_processor_commit_with_interleaving_charlie_reward_modified() {
	ExtBuilder::default().build().execute_with(|| {
		// Use test helpers to replicate test below with interleaving Charlie's commit (should return same rewards for Alice and Bob)
		create_pools();
		commit(true, true);
		check_events();
	});
}
