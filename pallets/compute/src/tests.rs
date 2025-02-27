#![cfg(test)]

use frame_support::{assert_err, assert_ok};
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
		assert_eq!(Compute::commit(&alice_account_id(), vec![(1u8, 1000u128, 1u128)]), None);
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
		assert_eq!(Compute::commit(&alice_account_id(), vec![(1u8, 1000u128, 1u128)]), None);
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
		assert_eq!(Compute::commit(&alice_account_id(), vec![(1u8, 1000u128, 1u128)]), None);
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
		assert_eq!(Compute::commit(&alice_account_id(), vec![(1u8, 1000u128, 1u128)]), None);
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
		assert_eq!(Compute::commit(&alice_account_id(), vec![(1u8, 2000u128, 1u128)]), None);
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
			Compute::commit(&alice_account_id(), vec![(1u8, 1000u128, 1u128)]),
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
