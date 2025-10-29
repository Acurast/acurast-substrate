use frame_support::{
	assert_err, assert_ok,
	sp_runtime::{
		bounded_vec,
		traits::{Hash, Scale},
		Permill, Perquintill,
	},
	traits::{Hooks, TypedGet},
};

use hex_literal::hex;
use pallet_acurast::{
	utils::validate_and_extract_attestation, Attestation, ComputeHooks, JobModules,
	JobRegistrationFor, ManagerLookup, MultiOrigin, Schedule,
};
use pallet_acurast_compute::{MetricPool, ProvisionalBuffer, SlidingBuffer};
use parity_scale_codec::Encode;
use reputation::{BetaReputation, ReputationEngine};
use sp_core::H256;

use crate::{
	mock::*, payments::JobBudget, stub::*, AdvertisementRestriction, Assignment,
	AssignmentStrategy, Config, Error, ExecutionMatch, ExecutionResult, ExecutionSpecifier,
	FeeManager, JobRequirements, JobStatus, Match, PlannedExecution, PlannedExecutions, PubKeys,
	RegistrationExtra, Runtime, SLA,
};

/// Job is not assigned and gets deregistered successfully.
#[test]
fn test_valid_deregister() {
	// 1000 is the smallest amount accepted by T::AssetTransactor::lock_asset for the asset used
	let ad = advertisement(1000, 1, 100_000, 50_000, 8);
	let registration1 = JobRegistrationFor::<Test> {
		script: script(),
		allowed_sources: None,
		allow_only_verified_sources: false,
		schedule: Schedule {
			duration: 5000,
			start_time: 1_671_800_400_000, // 23.12.2022 13:00
			end_time: 1_671_804_000_000,   // 23.12.2022 14:00 (one hour later)
			interval: 1_800_000,           // 30min
			max_start_delay: 5000,
		},
		memory: 5_000u32,
		network_requests: 5,
		storage: 20_000u32,
		required_modules: JobModules::default(),
		extra: RegistrationExtra {
			requirements: JobRequirements {
				assignment_strategy: AssignmentStrategy::Single(None),
				slots: 1,
				reward: 3_000_000 * 2,
				min_reputation: None,
				processor_version: None,
				runtime: Runtime::NodeJS,
			},
		},
	};

	ExtBuilder.build().execute_with(|| {
		let initial_job_id = Acurast::job_id_sequence();

		assert_ok!(AcurastMarketplace::advertise(
			RuntimeOrigin::signed(processor_account_id()),
			ad.clone(),
		));
		assert_eq!(
			Some(AdvertisementRestriction {
				max_memory: 50_000,
				network_request_quota: 8,
				storage_capacity: 100_000,
				allowed_consumers: ad.allowed_consumers.clone(),
				available_modules: JobModules::default(),
			}),
			AcurastMarketplace::stored_advertisement(processor_account_id())
		);
		assert_eq!(
			Some(ad.pricing.clone()),
			AcurastMarketplace::stored_advertisement_pricing(processor_account_id())
		);

		let job_id1 = (MultiOrigin::Acurast(alice_account_id()), initial_job_id + 1);

		assert_ok!(Acurast::register(
			RuntimeOrigin::signed(alice_account_id()),
			registration1.clone(),
		));
		assert_eq!(12_000_000, AcurastMarketplace::reserved(&job_id1));
		assert_eq!(
			Some(JobStatus::Open),
			AcurastMarketplace::stored_job_status(
				MultiOrigin::Acurast(alice_account_id()),
				initial_job_id + 1
			)
		);

		assert_ok!(Acurast::deregister(RuntimeOrigin::signed(alice_account_id()), job_id1.1,));

		assert_eq!(None, AcurastMarketplace::stored_job_status(&job_id1.0, &job_id1.1));
		// Job KeyID got removed
		assert_eq!(None, AcurastMarketplace::job_key_ids(&job_id1));

		// the remaining budget got refunded
		assert_eq!(0, AcurastMarketplace::reserved(&job_id1));

		assert_eq!(
			events(),
			[
				RuntimeEvent::AcurastMarketplace(crate::Event::AdvertisementStoredV2(
					processor_account_id()
				)),
				RuntimeEvent::Balances(pallet_balances::Event::Transfer {
					from: alice_account_id(),
					to: pallet_acurast_acount(),
					amount: 12_000_000
				}),
				RuntimeEvent::Acurast(pallet_acurast::Event::JobRegistrationStoredV2(
					job_id1.clone(),
				)),
				RuntimeEvent::Balances(pallet_balances::Event::Transfer {
					from: pallet_acurast_acount(),
					to: alice_account_id(),
					amount: 12_000_000
				}),
				RuntimeEvent::Acurast(pallet_acurast::Event::JobRegistrationRemoved(
					job_id1.clone()
				)),
			]
		);
	});
}

#[test]
fn test_deregister_on_matched_job() {
	let now: u64 = 1_671_800_100_000; // 23.12.2022 12:55;

	// 1000 is the smallest amount accepted by T::AssetTransactor::lock_asset for the asset used
	let ad = advertisement(1000, 1, 100_000, 50_000, 8);
	let registration1 = JobRegistrationFor::<Test> {
		script: script(),
		allowed_sources: None,
		allow_only_verified_sources: false,
		schedule: Schedule {
			duration: 5000,
			start_time: 1_671_800_400_000, // 23.12.2022 13:00
			end_time: 1_671_804_000_000,   // 23.12.2022 14:00 (one hour later)
			interval: 1_800_000,           // 30min
			max_start_delay: 5000,
		},
		memory: 5_000u32,
		network_requests: 5,
		storage: 20_000u32,
		required_modules: JobModules::default(),
		extra: RegistrationExtra {
			requirements: JobRequirements {
				assignment_strategy: AssignmentStrategy::Single(Some(bounded_vec![
					PlannedExecution { source: processor_account_id(), start_delay: 0 },
					PlannedExecution { source: processor_2_account_id(), start_delay: 0 }
				])),
				slots: 2,
				reward: 3_000_000 * 2,
				min_reputation: None,
				processor_version: None,
				runtime: Runtime::NodeJS,
			},
		},
	};

	ExtBuilder.build().execute_with(|| {
		let initial_job_id = Acurast::job_id_sequence();

		// pretend current time
		later(now);

		assert_ok!(AcurastMarketplace::advertise(
			RuntimeOrigin::signed(processor_account_id()),
			ad.clone(),
		));
		assert_ok!(AcurastMarketplace::advertise(
			RuntimeOrigin::signed(processor_2_account_id()),
			ad.clone(),
		));
		assert_eq!(
			Some(AdvertisementRestriction {
				max_memory: 50_000,
				network_request_quota: 8,
				storage_capacity: 100_000,
				allowed_consumers: ad.allowed_consumers.clone(),
				available_modules: JobModules::default(),
			}),
			AcurastMarketplace::stored_advertisement(processor_account_id())
		);
		assert_eq!(
			Some(ad.pricing.clone()),
			AcurastMarketplace::stored_advertisement_pricing(processor_account_id())
		);

		let job_id1 = (MultiOrigin::Acurast(alice_account_id()), initial_job_id + 1);

		assert_ok!(Acurast::register(
			RuntimeOrigin::signed(alice_account_id()),
			registration1.clone(),
		));
		assert_eq!(Balances::free_balance(alice_account_id()), 76_000_000);

		assert_eq!(24_000_000, AcurastMarketplace::reserved(&job_id1));
		assert_eq!(
			Some(JobStatus::Matched),
			AcurastMarketplace::stored_job_status(&job_id1.0, &job_id1.1)
		);

		assert_ok!(Acurast::deregister(RuntimeOrigin::signed(alice_account_id()), job_id1.1));
		// The amount should have been refunded
		assert_eq!(Balances::free_balance(alice_account_id()), 100_000_000);

		// Job got removed after the deregister call
		assert_eq!(None, AcurastMarketplace::stored_job_status(&job_id1.0, &job_id1.1));
		// Job KeyID got removed
		assert_eq!(None, AcurastMarketplace::job_key_ids(&job_id1));

		// the full budget got refunded
		assert_eq!(0, AcurastMarketplace::reserved(&job_id1));

		assert_eq!(
			events(),
			[
				RuntimeEvent::AcurastMarketplace(crate::Event::AdvertisementStoredV2(
					processor_account_id()
				)),
				RuntimeEvent::AcurastMarketplace(crate::Event::AdvertisementStoredV2(
					processor_2_account_id()
				)),
				RuntimeEvent::AcurastMarketplace(crate::Event::JobRegistrationMatchedV2(
					job_id1.clone()
				)),
				RuntimeEvent::Balances(pallet_balances::Event::Transfer {
					from: alice_account_id(),
					to: pallet_acurast_acount(),
					amount: 24_000_000
				}),
				RuntimeEvent::Acurast(pallet_acurast::Event::JobRegistrationStoredV2(
					job_id1.clone(),
				)),
				RuntimeEvent::Balances(pallet_balances::Event::Transfer {
					from: pallet_acurast_acount(),
					to: alice_account_id(),
					amount: 24_000_000
				}),
				RuntimeEvent::Acurast(pallet_acurast::Event::JobRegistrationRemoved(
					job_id1.clone()
				)),
			]
		);
	});
}

#[test]
fn test_deregister_on_assigned_job() {
	let now: u64 = 1_671_800_100_000; // 23.12.2022 12:55;

	// 1000 is the smallest amount accepted by T::AssetTransactor::lock_asset for the asset used
	let ad = advertisement(1000, 1, 100_000, 50_000, 8);
	let registration1 = JobRegistrationFor::<Test> {
		script: script(),
		allowed_sources: None,
		allow_only_verified_sources: false,
		schedule: Schedule {
			duration: 5000,
			start_time: 1_671_800_400_000, // 23.12.2022 13:00
			end_time: 1_671_804_000_000,   // 23.12.2022 14:00 (one hour later)
			interval: 1_800_000,           // 30min
			max_start_delay: 0,
		},
		memory: 5_000u32,
		network_requests: 5,
		storage: 20_000u32,
		required_modules: JobModules::default(),
		extra: RegistrationExtra {
			requirements: JobRequirements {
				assignment_strategy: AssignmentStrategy::Single(Some(bounded_vec![
					PlannedExecution { source: processor_account_id(), start_delay: 0 },
					PlannedExecution { source: processor_2_account_id(), start_delay: 0 }
				])),
				slots: 2,
				reward: 3_000_000 * 2,
				min_reputation: None,
				processor_version: None,
				runtime: Runtime::NodeJS,
			},
		},
	};

	ExtBuilder.build().execute_with(|| {
		let initial_job_id = Acurast::job_id_sequence();

		// pretend current time
		later(now);

		let _ = Balances::force_set_balance(RuntimeOrigin::root(), alice_account_id(), 100_000_000);

		let consumer_initial_balance = 100_000_000u128;
		let processor_initial_balance = 10_000_000u128;
		let pallet_initial_balance = 10_000_000u128;

		assert_eq!(Balances::free_balance(alice_account_id()), consumer_initial_balance);
		assert_eq!(Balances::free_balance(processor_2_account_id()), processor_initial_balance);
		assert_eq!(Balances::free_balance(processor_account_id()), processor_initial_balance);
		assert_eq!(Balances::free_balance(pallet_acurast_acount()), pallet_initial_balance);
		assert_eq!(Balances::free_balance(pallet_fees_account()), pallet_initial_balance);

		assert_ok!(AcurastMarketplace::advertise(
			RuntimeOrigin::signed(processor_account_id()),
			ad.clone(),
		));
		assert_ok!(AcurastMarketplace::advertise(
			RuntimeOrigin::signed(processor_2_account_id()),
			ad.clone(),
		));
		assert_eq!(
			Some(AdvertisementRestriction {
				max_memory: 50_000,
				network_request_quota: 8,
				storage_capacity: 100_000,
				allowed_consumers: ad.allowed_consumers.clone(),
				available_modules: JobModules::default(),
			}),
			AcurastMarketplace::stored_advertisement(processor_account_id())
		);
		assert_eq!(
			Some(ad.pricing.clone()),
			AcurastMarketplace::stored_advertisement_pricing(processor_account_id())
		);

		let job_id1 = (MultiOrigin::Acurast(alice_account_id()), initial_job_id + 1);

		assert_ok!(Acurast::register(
			RuntimeOrigin::signed(alice_account_id()),
			registration1.clone(),
		));

		assert_eq!(24_000_000, AcurastMarketplace::reserved(&job_id1));
		assert_eq!(
			Some(JobStatus::Matched),
			AcurastMarketplace::stored_job_status(&job_id1.0, &job_id1.1)
		);

		assert_ok!(AcurastMarketplace::acknowledge_match(
			RuntimeOrigin::signed(processor_account_id()),
			job_id1.clone(),
			PubKeys::default(),
		));
		let assignment =
			AcurastMarketplace::stored_matches(processor_account_id(), job_id1.clone()).unwrap();
		let total_reward = registration1.extra.requirements.reward
			* (registration1.extra.requirements.slots as u128)
			* (registration1.schedule.execution_count() as u128);
		assert_eq!(
			Balances::free_balance(alice_account_id()),
			consumer_initial_balance - total_reward
		);
		// assert_eq!(Balances::free_balance(&alice_account_id()), 76_000_000);
		assert_eq!(Balances::free_balance(processor_account_id()), processor_initial_balance);
		assert_eq!(Balances::free_balance(processor_2_account_id()), processor_initial_balance);

		assert_ok!(Acurast::deregister(RuntimeOrigin::signed(alice_account_id()), job_id1.1));
		assert_eq!(
			Balances::free_balance(alice_account_id()),
			consumer_initial_balance - assignment.fee_per_execution
		);
		assert_eq!(Balances::free_balance(processor_2_account_id()), processor_initial_balance);

		// Job got removed after the deregister call
		assert_eq!(None, AcurastMarketplace::stored_job_status(&job_id1.0, &job_id1.1));
		// Job KeyID got removed
		assert_eq!(None, AcurastMarketplace::job_key_ids(&job_id1));

		// the full budget got refunded
		assert_eq!(0, AcurastMarketplace::reserved(&job_id1));

		assert_eq!(
			events(),
			[
				RuntimeEvent::Balances(pallet_balances::Event::BalanceSet {
					who: alice_account_id(),
					free: 100_000_000
				}),
				RuntimeEvent::AcurastMarketplace(crate::Event::AdvertisementStoredV2(
					processor_account_id()
				)),
				RuntimeEvent::AcurastMarketplace(crate::Event::AdvertisementStoredV2(
					processor_2_account_id()
				)),
				RuntimeEvent::AcurastMarketplace(crate::Event::JobRegistrationMatchedV2(
					job_id1.clone()
				)),
				RuntimeEvent::Balances(pallet_balances::Event::Transfer {
					from: alice_account_id(),
					to: pallet_acurast_acount(),
					amount: total_reward
				}),
				RuntimeEvent::Acurast(pallet_acurast::Event::JobRegistrationStoredV2(
					job_id1.clone(),
				)),
				RuntimeEvent::AcurastMarketplace(crate::Event::JobRegistrationAssignedV2(
					job_id1.clone(),
					processor_account_id(),
				)),
				RuntimeEvent::Balances(pallet_balances::Event::Withdraw {
					who: pallet_acurast_acount(),
					amount: assignment.fee_per_execution
				}),
				RuntimeEvent::Balances(pallet_balances::Event::Transfer {
					from: pallet_acurast_acount(),
					to: alice_account_id(),
					amount: total_reward - assignment.fee_per_execution
				}),
				RuntimeEvent::Acurast(pallet_acurast::Event::JobRegistrationRemoved(
					job_id1.clone()
				)),
			]
		);
	});
}

#[test]
fn test_deregister_on_assigned_job_for_competing() {
	let now: u64 = 1_671_800_400_000 - <Test as Config>::MatchingCompetingDueDelta::get();

	// 1000 is the smallest amount accepted by T::AssetTransactor::lock_asset for the asset used
	let ad = advertisement(1000, 1, 100_000, 50_000, 8);
	let registration1 = JobRegistrationFor::<Test> {
		script: script(),
		allowed_sources: None,
		allow_only_verified_sources: false,
		schedule: Schedule {
			duration: 5000,
			start_time: 1_671_800_400_000, // 23.12.2022 13:00
			end_time: 1_671_804_000_000,   // 23.12.2022 14:00 (one hour later)
			interval: 1_800_000,           // 30min
			max_start_delay: 0,
		},
		memory: 5_000u32,
		network_requests: 5,
		storage: 20_000u32,
		required_modules: JobModules::default(),
		extra: RegistrationExtra {
			requirements: JobRequirements {
				assignment_strategy: AssignmentStrategy::Competing,
				slots: 2,
				reward: 3_000_000 * 2,
				min_reputation: None,
				processor_version: None,
				runtime: Runtime::NodeJS,
			},
		},
	};

	ExtBuilder.build().execute_with(|| {
		let initial_job_id = Acurast::job_id_sequence();

		// pretend current time
		later(now);

		let _ = Balances::force_set_balance(RuntimeOrigin::root(), alice_account_id(), 100_000_000);

		let consumer_initial_balance = 100_000_000u128;
		let processor_initial_balance = 10_000_000u128;
		let pallet_initial_balance = 10_000_000u128;

		assert_eq!(Balances::free_balance(alice_account_id()), consumer_initial_balance);
		assert_eq!(Balances::free_balance(processor_2_account_id()), processor_initial_balance);
		assert_eq!(Balances::free_balance(processor_account_id()), processor_initial_balance);
		assert_eq!(Balances::free_balance(pallet_acurast_acount()), pallet_initial_balance);
		assert_eq!(Balances::free_balance(pallet_fees_account()), pallet_initial_balance);

		assert_ok!(AcurastMarketplace::advertise(
			RuntimeOrigin::signed(processor_account_id()),
			ad.clone(),
		));
		assert_ok!(AcurastMarketplace::advertise(
			RuntimeOrigin::signed(processor_2_account_id()),
			ad.clone(),
		));
		assert_eq!(
			Some(AdvertisementRestriction {
				max_memory: 50_000,
				network_request_quota: 8,
				storage_capacity: 100_000,
				allowed_consumers: ad.allowed_consumers.clone(),
				available_modules: JobModules::default(),
			}),
			AcurastMarketplace::stored_advertisement(processor_account_id())
		);
		assert_eq!(
			Some(ad.pricing.clone()),
			AcurastMarketplace::stored_advertisement_pricing(processor_account_id())
		);

		let job_id1 = (MultiOrigin::Acurast(alice_account_id()), initial_job_id + 1);

		assert_ok!(Acurast::register(
			RuntimeOrigin::signed(alice_account_id()),
			registration1.clone(),
		));

		assert_eq!(24_000_000, AcurastMarketplace::reserved(&job_id1));
		assert_ok!(AcurastMarketplace::propose_execution_matching(
			RuntimeOrigin::signed(bob_account_id()),
			vec![ExecutionMatch {
				job_id: job_id1.clone(),
				execution_index: 0,
				sources: vec![
					PlannedExecution { source: processor_account_id(), start_delay: 0 },
					PlannedExecution { source: processor_2_account_id(), start_delay: 0 }
				]
				.try_into()
				.unwrap(),
			}]
			.try_into()
			.unwrap(),
		));
		assert_eq!(
			Some(JobStatus::Matched),
			AcurastMarketplace::stored_job_status(&job_id1.0, &job_id1.1)
		);

		assert_ok!(AcurastMarketplace::acknowledge_execution_match(
			RuntimeOrigin::signed(processor_account_id()),
			job_id1.clone(),
			0,
			PubKeys::default(),
		));
		assert_ok!(AcurastMarketplace::acknowledge_execution_match(
			RuntimeOrigin::signed(processor_2_account_id()),
			job_id1.clone(),
			0,
			PubKeys::default(),
		));
		let assignment1 =
			AcurastMarketplace::stored_matches(processor_account_id(), job_id1.clone()).unwrap();
		let assignment2 =
			AcurastMarketplace::stored_matches(processor_2_account_id(), job_id1.clone()).unwrap();
		let total_reward = registration1.extra.requirements.reward
			* (registration1.extra.requirements.slots as u128)
			* (registration1.schedule.execution_count() as u128);
		assert_eq!(
			Balances::free_balance(alice_account_id()),
			consumer_initial_balance - total_reward
		);
		assert_eq!(Balances::free_balance(processor_account_id()), processor_initial_balance);
		assert_eq!(Balances::free_balance(processor_2_account_id()), processor_initial_balance);

		assert_ok!(Acurast::deregister(RuntimeOrigin::signed(alice_account_id()), job_id1.1));

		let fee_percentage = FeeManagerImpl::get_fee_percentage();

		let matcher_percentage = FeeManagerImpl::get_matcher_percentage();
		let matcher_payout = matcher_percentage.mul_floor(
			(registration1.extra.requirements.reward
				* registration1.extra.requirements.slots as u128)
				- (assignment1.fee_per_execution + assignment2.fee_per_execution),
		);

		let matcher_payout_fee = fee_percentage.mul_floor(matcher_payout);
		let matcher_pauout_after_fee = matcher_payout - matcher_payout_fee;

		assert_eq!(
			Balances::free_balance(alice_account_id()),
			consumer_initial_balance
				- (assignment1.fee_per_execution + assignment2.fee_per_execution + matcher_payout)
		);

		// Job got removed after the deregister call
		assert_eq!(None, AcurastMarketplace::stored_job_status(&job_id1.0, &job_id1.1));
		// Job KeyID got removed
		assert_eq!(None, AcurastMarketplace::job_key_ids(&job_id1));

		// the full budget got refunded
		assert_eq!(0, AcurastMarketplace::reserved(&job_id1));

		assert_eq!(
			events(),
			[
				RuntimeEvent::Balances(pallet_balances::Event::BalanceSet {
					who: alice_account_id(),
					free: consumer_initial_balance
				}),
				RuntimeEvent::AcurastMarketplace(crate::Event::AdvertisementStoredV2(
					processor_account_id()
				)),
				RuntimeEvent::AcurastMarketplace(crate::Event::AdvertisementStoredV2(
					processor_2_account_id()
				)),
				RuntimeEvent::Balances(pallet_balances::Event::Transfer {
					from: alice_account_id(),
					to: pallet_acurast_acount(),
					amount: total_reward
				}),
				RuntimeEvent::Acurast(pallet_acurast::Event::JobRegistrationStoredV2(
					job_id1.clone(),
				)),
				RuntimeEvent::AcurastMarketplace(crate::Event::JobExecutionMatchedV2(
					job_id1.clone()
				)),
				RuntimeEvent::Balances(pallet_balances::Event::Transfer {
					from: pallet_acurast_acount(),
					to: pallet_fees_account(),
					amount: matcher_payout_fee,
				}),
				RuntimeEvent::Balances(pallet_balances::Event::Transfer {
					from: pallet_acurast_acount(),
					to: bob_account_id(),
					amount: matcher_pauout_after_fee,
				}),
				RuntimeEvent::AcurastMarketplace(crate::Event::JobRegistrationAssignedV2(
					job_id1.clone(),
					processor_account_id(),
				)),
				RuntimeEvent::AcurastMarketplace(crate::Event::JobRegistrationAssignedV2(
					job_id1.clone(),
					processor_2_account_id(),
				)),
				RuntimeEvent::Balances(pallet_balances::Event::Withdraw {
					who: pallet_acurast_acount(),
					amount: assignment1.fee_per_execution
				}),
				RuntimeEvent::Balances(pallet_balances::Event::Withdraw {
					who: pallet_acurast_acount(),
					amount: assignment2.fee_per_execution
				}),
				RuntimeEvent::Balances(pallet_balances::Event::Transfer {
					from: pallet_acurast_acount(),
					to: alice_account_id(),
					amount: total_reward
						- (assignment1.fee_per_execution
							+ assignment2.fee_per_execution
							+ matcher_payout)
				}),
				RuntimeEvent::Acurast(pallet_acurast::Event::JobRegistrationRemoved(
					job_id1.clone()
				)),
			]
		);
	});
}

#[test]
fn test_deregister_on_assigned_job_for_competing_2() {
	let now: u64 = 1_671_800_400_000 - <Test as Config>::MatchingCompetingDueDelta::get();

	// 1000 is the smallest amount accepted by T::AssetTransactor::lock_asset for the asset used
	let ad = advertisement(1000, 1, 100_000, 50_000, 8);
	let registration1 = JobRegistrationFor::<Test> {
		script: script(),
		allowed_sources: None,
		allow_only_verified_sources: false,
		schedule: Schedule {
			duration: 5000,
			start_time: 1_671_800_400_000, // 23.12.2022 13:00
			end_time: 1_671_804_000_000,   // 23.12.2022 14:00 (one hour later)
			interval: 1_800_000,           // 30min
			max_start_delay: 0,
		},
		memory: 5_000u32,
		network_requests: 5,
		storage: 20_000u32,
		required_modules: JobModules::default(),
		extra: RegistrationExtra {
			requirements: JobRequirements {
				assignment_strategy: AssignmentStrategy::Competing,
				slots: 1,
				reward: 3_000_000 * 2,
				min_reputation: None,
				processor_version: None,
				runtime: Runtime::NodeJS,
			},
		},
	};

	ExtBuilder.build().execute_with(|| {
		let initial_job_id = Acurast::job_id_sequence();

		// pretend current time
		later(now);

		let _ = Balances::force_set_balance(RuntimeOrigin::root(), alice_account_id(), 100_000_000);

		let consumer_initial_balance = 100_000_000u128;
		let processor_initial_balance = 10_000_000u128;
		let pallet_initial_balance = 10_000_000u128;

		assert_eq!(Balances::free_balance(alice_account_id()), consumer_initial_balance);
		assert_eq!(Balances::free_balance(processor_2_account_id()), processor_initial_balance);
		assert_eq!(Balances::free_balance(processor_account_id()), processor_initial_balance);
		assert_eq!(Balances::free_balance(pallet_acurast_acount()), pallet_initial_balance);
		assert_eq!(Balances::free_balance(pallet_fees_account()), pallet_initial_balance);

		assert_ok!(AcurastMarketplace::advertise(
			RuntimeOrigin::signed(processor_account_id()),
			ad.clone(),
		));
		assert_ok!(AcurastMarketplace::advertise(
			RuntimeOrigin::signed(processor_2_account_id()),
			ad.clone(),
		));
		assert_eq!(
			Some(AdvertisementRestriction {
				max_memory: 50_000,
				network_request_quota: 8,
				storage_capacity: 100_000,
				allowed_consumers: ad.allowed_consumers.clone(),
				available_modules: JobModules::default(),
			}),
			AcurastMarketplace::stored_advertisement(processor_account_id())
		);
		assert_eq!(
			Some(ad.pricing.clone()),
			AcurastMarketplace::stored_advertisement_pricing(processor_account_id())
		);

		let job_id1 = (MultiOrigin::Acurast(alice_account_id()), initial_job_id + 1);

		assert_ok!(Acurast::register(
			RuntimeOrigin::signed(alice_account_id()),
			registration1.clone(),
		));

		assert_eq!(12_000_000, AcurastMarketplace::reserved(&job_id1));
		assert_ok!(AcurastMarketplace::propose_execution_matching(
			RuntimeOrigin::signed(bob_account_id()),
			vec![ExecutionMatch {
				job_id: job_id1.clone(),
				execution_index: 0,
				sources: vec![PlannedExecution { source: processor_account_id(), start_delay: 0 },]
					.try_into()
					.unwrap(),
			}]
			.try_into()
			.unwrap(),
		));
		assert_eq!(
			Some(JobStatus::Matched),
			AcurastMarketplace::stored_job_status(&job_id1.0, &job_id1.1)
		);

		assert_ok!(AcurastMarketplace::acknowledge_execution_match(
			RuntimeOrigin::signed(processor_account_id()),
			job_id1.clone(),
			0,
			PubKeys::default(),
		));

		later(registration1.schedule.start_time);

		let mut assignment1 =
			AcurastMarketplace::stored_matches(processor_account_id(), job_id1.clone()).unwrap();

		assert_ok!(AcurastMarketplace::report(
			RuntimeOrigin::signed(processor_account_id()),
			job_id1.clone(),
			ExecutionResult::Success(b"JOB_EXECUTED".to_vec().try_into().unwrap()),
		));

		assignment1.sla.met = 1;

		later(now + registration1.schedule.interval);

		assert_ok!(AcurastMarketplace::propose_execution_matching(
			RuntimeOrigin::signed(bob_account_id()),
			vec![ExecutionMatch {
				job_id: job_id1.clone(),
				execution_index: 1,
				sources: vec![PlannedExecution {
					source: processor_2_account_id(),
					start_delay: 0,
				},]
				.try_into()
				.unwrap(),
			}]
			.try_into()
			.unwrap(),
		));

		assert_ok!(AcurastMarketplace::acknowledge_execution_match(
			RuntimeOrigin::signed(processor_2_account_id()),
			job_id1.clone(),
			1,
			PubKeys::default(),
		));

		let assignment2 =
			AcurastMarketplace::stored_matches(processor_2_account_id(), job_id1.clone()).unwrap();

		let total_reward = registration1.extra.requirements.reward
			* (registration1.extra.requirements.slots as u128)
			* (registration1.schedule.execution_count() as u128);
		let fee_percentage = FeeManagerImpl::get_fee_percentage();
		let fee1 = fee_percentage.mul_floor(assignment1.fee_per_execution);
		let reward1_after_fee = assignment1.fee_per_execution - fee1;
		let matcher_percentage = FeeManagerImpl::get_matcher_percentage();
		let matcher_payout = matcher_percentage
			.mul_floor(registration1.extra.requirements.reward - assignment1.fee_per_execution)
			+ matcher_percentage
				.mul_floor(registration1.extra.requirements.reward - assignment2.fee_per_execution);
		let matcher_payout_fee = fee_percentage.mul_floor(matcher_payout);
		let matcher_payout_afer_fee = matcher_payout - matcher_payout_fee;

		assert_eq!(
			Balances::free_balance(alice_account_id()),
			consumer_initial_balance - total_reward
		);

		assert_eq!(Balances::free_balance(processor_2_account_id()), processor_initial_balance);

		assert_ok!(Acurast::deregister(RuntimeOrigin::signed(alice_account_id()), job_id1.1));

		let fee2 = fee_percentage.mul_floor(assignment2.fee_per_execution);
		let reward2_after_fee = assignment2.fee_per_execution - fee2;

		assert_eq!(
			Balances::free_balance(alice_account_id()),
			consumer_initial_balance
				- (assignment1.fee_per_execution + assignment2.fee_per_execution + matcher_payout)
		);

		// Job got removed after the deregister call
		assert_eq!(None, AcurastMarketplace::stored_job_status(&job_id1.0, &job_id1.1));
		// Job KeyID got removed
		assert_eq!(None, AcurastMarketplace::job_key_ids(&job_id1));

		// the full budget got refunded
		assert_eq!(0, AcurastMarketplace::reserved(&job_id1));

		assert_eq!(
			events(),
			[
				RuntimeEvent::Balances(pallet_balances::Event::BalanceSet {
					who: alice_account_id(),
					free: consumer_initial_balance
				}),
				RuntimeEvent::AcurastMarketplace(crate::Event::AdvertisementStoredV2(
					processor_account_id()
				)),
				RuntimeEvent::AcurastMarketplace(crate::Event::AdvertisementStoredV2(
					processor_2_account_id()
				)),
				RuntimeEvent::Balances(pallet_balances::Event::Transfer {
					from: alice_account_id(),
					to: pallet_acurast_acount(),
					amount: total_reward
				}),
				RuntimeEvent::Acurast(pallet_acurast::Event::JobRegistrationStoredV2(
					job_id1.clone(),
				)),
				RuntimeEvent::AcurastMarketplace(crate::Event::JobExecutionMatchedV2(
					job_id1.clone()
				)),
				RuntimeEvent::Balances(pallet_balances::Event::Transfer {
					from: pallet_acurast_acount(),
					to: pallet_fees_account(),
					amount: matcher_payout_fee / 2
				}),
				RuntimeEvent::Balances(pallet_balances::Event::Transfer {
					from: pallet_acurast_acount(),
					to: bob_account_id(),
					amount: matcher_payout_afer_fee / 2
				}),
				RuntimeEvent::AcurastMarketplace(crate::Event::JobRegistrationAssignedV2(
					job_id1.clone(),
					processor_account_id(),
				)),
				RuntimeEvent::Balances(pallet_balances::Event::Withdraw {
					who: pallet_acurast_acount(),
					amount: assignment1.fee_per_execution
				}),
				RuntimeEvent::AcurastMarketplace(crate::Event::ExecutionSuccess(
					job_id1.clone(),
					b"JOB_EXECUTED".to_vec().try_into().unwrap()
				)),
				RuntimeEvent::AcurastMarketplace(crate::Event::ReportedV2(
					job_id1.clone(),
					processor_account_id(),
				)),
				RuntimeEvent::AcurastMarketplace(crate::Event::JobExecutionMatchedV2(
					job_id1.clone()
				)),
				RuntimeEvent::Balances(pallet_balances::Event::Transfer {
					from: pallet_acurast_acount(),
					to: pallet_fees_account(),
					amount: matcher_payout_fee / 2
				}),
				RuntimeEvent::Balances(pallet_balances::Event::Transfer {
					from: pallet_acurast_acount(),
					to: bob_account_id(),
					amount: matcher_payout_afer_fee / 2
				}),
				RuntimeEvent::AcurastMarketplace(crate::Event::JobRegistrationAssignedV2(
					job_id1.clone(),
					processor_2_account_id(),
				)),
				RuntimeEvent::Balances(pallet_balances::Event::Withdraw {
					who: pallet_acurast_acount(),
					amount: assignment2.fee_per_execution
				}),
				RuntimeEvent::Balances(pallet_balances::Event::Transfer {
					from: pallet_acurast_acount(),
					to: alice_account_id(),
					amount: total_reward
						- (assignment1.fee_per_execution
							+ assignment2.fee_per_execution
							+ matcher_payout)
				}),
				RuntimeEvent::Acurast(pallet_acurast::Event::JobRegistrationRemoved(
					job_id1.clone()
				)),
			]
		);
	});
}

#[test]
fn test_match() {
	let now: u64 = 1_671_800_100_000; // 23.12.2022 12:55;

	// 1000 is the smallest amount accepted by T::AssetTransactor::lock_asset for the asset used
	let ad = advertisement(1000, 1, 100_000, 50_000, 8);
	let registration1 = JobRegistrationFor::<Test> {
		script: script(),
		allowed_sources: None,
		allow_only_verified_sources: false,
		schedule: Schedule {
			duration: 5000,
			start_time: 1_671_800_400_000, // 23.12.2022 13:00
			end_time: 1_671_804_000_000,   // 23.12.2022 14:00 (one hour later)
			interval: 1_800_000,           // 30min
			max_start_delay: 5000,
		},
		memory: 5_000u32,
		network_requests: 5,
		storage: 20_000u32,
		required_modules: JobModules::default(),
		extra: RegistrationExtra {
			requirements: JobRequirements {
				assignment_strategy: AssignmentStrategy::Single(None),
				slots: 1,
				reward: 3_000_000 * 2,
				min_reputation: None,
				processor_version: None,
				runtime: Runtime::NodeJS,
			},
		},
	};
	let registration2 = JobRegistrationFor::<Test> {
		script: script(),
		allowed_sources: None,
		allow_only_verified_sources: false,
		schedule: Schedule {
			duration: 5000,
			start_time: 1_671_800_400_000, // 23.12.2022 13:00
			end_time: 1_671_804_000_000,   // 23.12.2022 14:00 (one hour later)
			interval: 1_800_000,           // 30min
			max_start_delay: 10_000,
		},
		memory: 5_000u32,
		network_requests: 5,
		storage: 20_000u32,
		required_modules: JobModules::default(),
		extra: RegistrationExtra {
			requirements: JobRequirements {
				assignment_strategy: AssignmentStrategy::Single(None),
				slots: 1,
				reward: 3_000_000 * 2,
				min_reputation: None,
				processor_version: None,
				runtime: Runtime::NodeJS,
			},
		},
	};

	ExtBuilder.build().execute_with(|| {
		let initial_job_id = Acurast::job_id_sequence();

		// pretend current time
		later(now);

		let chain = attestation_chain();
		assert_ok!(Acurast::submit_attestation(
			RuntimeOrigin::signed(processor_account_id()),
			chain.clone()
		));
		assert_ok!(validate_and_extract_attestation::<Test>(&processor_account_id(), &chain));

		assert_ok!(AcurastMarketplace::advertise(
			RuntimeOrigin::signed(processor_account_id()),
			ad.clone(),
		));
		assert_eq!(
			Some(AdvertisementRestriction {
				max_memory: 50_000,
				network_request_quota: 8,
				storage_capacity: 100_000,
				allowed_consumers: ad.allowed_consumers.clone(),
				available_modules: JobModules::default(),
			}),
			AcurastMarketplace::stored_advertisement(processor_account_id())
		);
		assert_eq!(
			Some(ad.pricing.clone()),
			AcurastMarketplace::stored_advertisement_pricing(processor_account_id())
		);

		assert_ok!(AcurastCompute::create_pool(
			RuntimeOrigin::root(),
			*b"cpu-ops-per-second______",
			Perquintill::from_percent(25),
			bounded_vec![],
		));
		let pool_id = AcurastCompute::last_metric_pool_id();
		let manager =
			<Test as pallet_acurast_compute::Config>::ManagerProviderForEligibleProcessor::lookup(
				&processor_account_id(),
			)
			.unwrap();
		let _ = AcurastCompute::commit(&processor_account_id(), &manager, &[(pool_id, 1, 2)]);

		let job_id1 = (MultiOrigin::Acurast(alice_account_id()), initial_job_id + 1);
		let job_id2 = (MultiOrigin::Acurast(alice_account_id()), initial_job_id + 2);

		assert_ok!(Acurast::register_with_min_metrics(
			RuntimeOrigin::signed(alice_account_id()),
			registration1.clone(),
			bounded_vec![(pool_id, 1, 4)],
		));
		assert_eq!(12_000_000, AcurastMarketplace::reserved(&job_id1));
		assert_ok!(Acurast::register(
			RuntimeOrigin::signed(alice_account_id()),
			registration2.clone(),
		));
		assert_eq!(12_000_000, AcurastMarketplace::reserved(&job_id2));
		assert_eq!(
			Some(JobStatus::Open),
			AcurastMarketplace::stored_job_status(
				MultiOrigin::Acurast(alice_account_id()),
				initial_job_id + 1
			)
		);

		let job_match1 = Match {
			job_id: job_id1.clone(),
			sources: bounded_vec![PlannedExecution {
				source: processor_account_id(),
				start_delay: 0,
			}],
		};
		let job_match2 = Match {
			job_id: job_id2.clone(),
			sources: bounded_vec![PlannedExecution {
				source: processor_account_id(),
				start_delay: 5_000,
			}],
		};

		assert_ok!(AcurastMarketplace::propose_matching(
			RuntimeOrigin::signed(charlie_account_id()),
			vec![job_match1.clone(), job_match2.clone()].try_into().unwrap(),
		));
		assert_eq!(
			Some(JobStatus::Matched),
			AcurastMarketplace::stored_job_status(&job_id1.0, &job_id1.1)
		);
		// matcher got paid out already so job budget decreased
		assert_eq!(11804000, AcurastMarketplace::reserved(&job_id1));
		assert_eq!(11804000, AcurastMarketplace::reserved(&job_id2));

		assert_ok!(AcurastMarketplace::acknowledge_match(
			RuntimeOrigin::signed(processor_account_id()),
			job_id1.clone(),
			PubKeys::default(),
		));
		assert_eq!(
			Some(JobStatus::Assigned(1)),
			AcurastMarketplace::stored_job_status(&job_id1.0, &job_id1.1)
		);

		// pretend time moved on
		assert_eq!(1, System::block_number());
		later(registration1.schedule.start_time + 3000); // pretend actual execution until report call took 3 seconds
		assert_eq!(2, System::block_number());

		assert_ok!(AcurastMarketplace::report(
			RuntimeOrigin::signed(processor_account_id()),
			job_id1.clone(),
			ExecutionResult::Success(operation_hash())
		));
		// job budget decreased by reward worth one execution
		assert_eq!(6784000, AcurastMarketplace::reserved(&job_id1));
		let assignment1 =
			AcurastMarketplace::stored_matches(processor_account_id(), job_id1.clone()).unwrap();
		let assignment2 =
			AcurastMarketplace::stored_matches(processor_account_id(), job_id2.clone()).unwrap();
		// average reward updated on acknowledged
		assert_eq!(
			Some((assignment1.fee_per_execution + assignment2.fee_per_execution) / 2),
			AcurastMarketplace::average_reward()
		);
		// reputation updated to around ~60%
		assert_eq!(
			Permill::from_parts(611_764),
			BetaReputation::<u128>::normalize(
				AcurastMarketplace::stored_reputation(processor_account_id()).unwrap()
			)
			.unwrap()
		);
		assert_eq!(
			Some(Assignment {
				execution: ExecutionSpecifier::All,
				slot: 0,
				start_delay: 0,
				fee_per_execution: 5_020_000,
				acknowledged: true,
				sla: SLA { total: 2, met: 1 },
				pub_keys: PubKeys::default(),
			}),
			AcurastMarketplace::stored_matches(processor_account_id(), job_id1.clone()),
		);
		// Job still assigned after one execution
		assert_eq!(
			Some(JobStatus::Assigned(1)),
			AcurastMarketplace::stored_job_status(&job_id1.0, &job_id1.1),
		);

		// pretend time moved on
		later(registration1.schedule.range(0).1);
		assert_eq!(3, System::block_number());

		assert_ok!(AcurastMarketplace::report(
			RuntimeOrigin::signed(processor_account_id()),
			job_id1.clone(),
			ExecutionResult::Success(operation_hash())
		));
		// job budget decreased by reward worth one execution
		assert_eq!(1764000, AcurastMarketplace::reserved(&job_id1));

		later(registration1.schedule.end_time + 1);
		assert_eq!(4, System::block_number());
		assert_eq!(Some(2), AcurastMarketplace::total_assigned());
		// reputation increased
		assert_eq!(
			Permill::from_parts(725_789),
			BetaReputation::<u128>::normalize(
				AcurastMarketplace::stored_reputation(processor_account_id()).unwrap()
			)
			.unwrap()
		);
		// Job still assigned after last execution
		assert_eq!(
			Some(JobStatus::Assigned(1)),
			AcurastMarketplace::stored_job_status(&job_id1.0, &job_id1.1),
		);

		assert_ok!(Acurast::deregister(RuntimeOrigin::signed(alice_account_id()), job_id1.1,));

		assert_eq!(
			None,
			AcurastMarketplace::stored_matches(processor_account_id(), job_id1.clone()),
		);

		// Job no longer assigned after finalization
		assert_eq!(None, AcurastMarketplace::stored_job_status(&job_id1.0, &job_id1.1));
		// Job KeyID got removed
		assert_eq!(None, AcurastMarketplace::job_key_ids(&job_id1));
		// the remaining budget got refunded
		assert_eq!(0, AcurastMarketplace::reserved(&job_id1));
		// but job2 still have full budget
		assert_eq!(11804000, AcurastMarketplace::reserved(&job_id2));

		assert_eq!(
			events(),
			[
				RuntimeEvent::Acurast(pallet_acurast::Event::AttestationStoredV2(
					processor_account_id()
				)),
				RuntimeEvent::AcurastMarketplace(crate::Event::AdvertisementStoredV2(
					processor_account_id()
				)),
				RuntimeEvent::AcurastCompute(pallet_acurast_compute::Event::PoolCreated(
					1,
					MetricPool {
						config: bounded_vec![],
						name: *b"cpu-ops-per-second______",
						reward: ProvisionalBuffer::new(Perquintill::from_percent(25)),
						total: SlidingBuffer::new(0),
						total_with_bonus: SlidingBuffer::new(0),
					}
				)),
				RuntimeEvent::Balances(pallet_balances::Event::Transfer {
					from: alice_account_id(),
					to: pallet_acurast_acount(),
					amount: 12_000_000
				}),
				RuntimeEvent::Acurast(pallet_acurast::Event::JobRegistrationStoredV2(
					job_id1.clone(),
				)),
				RuntimeEvent::Balances(pallet_balances::Event::Transfer {
					from: alice_account_id(),
					to: pallet_acurast_acount(),
					amount: 12_000_000
				}),
				RuntimeEvent::Acurast(pallet_acurast::Event::JobRegistrationStoredV2(
					job_id2.clone(),
				)),
				RuntimeEvent::AcurastMarketplace(crate::Event::JobRegistrationMatchedV2(
					job_id1.clone()
				)),
				RuntimeEvent::AcurastMarketplace(crate::Event::JobRegistrationMatchedV2(
					job_id2.clone()
				)),
				RuntimeEvent::Balances(pallet_balances::Event::Transfer {
					from: pallet_acurast_acount(),
					to: pallet_fees_account(),
					amount: 117_600
				}),
				RuntimeEvent::Balances(pallet_balances::Event::Transfer {
					from: pallet_acurast_acount(),
					to: charlie_account_id(),
					amount: 274_400
				}),
				RuntimeEvent::AcurastMarketplace(crate::Event::JobRegistrationAssignedV2(
					job_id1.clone(),
					processor_account_id(),
				)),
				RuntimeEvent::Balances(pallet_balances::Event::Withdraw {
					who: pallet_acurast_acount(),
					amount: 5_020_000
				}),
				RuntimeEvent::AcurastMarketplace(crate::Event::ExecutionSuccess(
					job_id1.clone(),
					operation_hash()
				)),
				RuntimeEvent::AcurastMarketplace(crate::Event::ReportedV2(
					job_id1.clone(),
					processor_account_id(),
				)),
				RuntimeEvent::Balances(pallet_balances::Event::Withdraw {
					who: pallet_acurast_acount(),
					amount: 5_020_000
				}),
				RuntimeEvent::AcurastMarketplace(crate::Event::ExecutionSuccess(
					job_id1.clone(),
					operation_hash()
				)),
				RuntimeEvent::AcurastMarketplace(crate::Event::ReportedV2(
					job_id1.clone(),
					processor_account_id(),
				)),
				RuntimeEvent::Balances(pallet_balances::Event::Transfer {
					from: pallet_acurast_acount(),
					to: alice_account_id(),
					amount: 1_764_000
				}),
				RuntimeEvent::Acurast(pallet_acurast::Event::JobRegistrationRemoved(
					job_id1.clone()
				))
			]
		);
	});
}

#[test]
fn test_multi_assignments() {
	let now = 1_694_795_700_000; // 15.09.2023 17:35

	// 1000 is the smallest amount accepted by T::AssetTransactor::lock_asset for the asset used
	let ad = advertisement(1000, 1, 100_000, 50_000, 8);
	let registration = JobRegistrationFor::<Test> {
		script: script(),
		allowed_sources: None,
		allow_only_verified_sources: false,
		schedule: Schedule {
			duration: 1000,
			start_time: 1_694_796_000_000, // 15.09.2023 17:40
			end_time: 1_694_796_120_000,   // 15.09.2023 17:42 (2 minutes later)
			interval: 10000,               // 10 seconds
			max_start_delay: 0,
		},
		memory: 5_000u32,
		network_requests: 5,
		storage: 20_000u32,
		required_modules: JobModules::default(),
		extra: RegistrationExtra {
			requirements: JobRequirements {
				assignment_strategy: AssignmentStrategy::Single(None),
				slots: 4,
				reward: 3_000_000 * 2,
				min_reputation: None,
				processor_version: None,
				runtime: Runtime::NodeJS,
			},
		},
	};

	ExtBuilder.build().execute_with(|| {
		let _ =
			Balances::force_set_balance(RuntimeOrigin::root(), alice_account_id(), 1_000_000_000);

		let initial_job_id = Acurast::job_id_sequence();

		// pretend current time
		later(now);

		let processors = vec![
			(processor_account_id(), attestation_chain()),
			(processor_2_account_id(), attestation_chain_processor_2()),
			(processor_3_account_id(), attestation_chain_processor_3()),
			(processor_4_account_id(), attestation_chain_processor_4()),
		];

		let _attestations: Vec<Attestation> = processors
			.iter()
			.map(|(processor, attestation_chain)| {
				assert_ok!(Acurast::submit_attestation(
					RuntimeOrigin::signed(processor.clone()),
					attestation_chain.clone()
				));
				let attestation =
					validate_and_extract_attestation::<Test>(processor, attestation_chain).unwrap();

				assert_ok!(AcurastMarketplace::advertise(
					RuntimeOrigin::signed(processor.clone()),
					ad.clone(),
				));
				assert_eq!(
					Some(AdvertisementRestriction {
						max_memory: 50_000,
						network_request_quota: 8,
						storage_capacity: 100_000,
						allowed_consumers: ad.allowed_consumers.clone(),
						available_modules: JobModules::default(),
					}),
					AcurastMarketplace::stored_advertisement(processor)
				);
				assert_eq!(
					Some(ad.pricing.clone()),
					AcurastMarketplace::stored_advertisement_pricing(processor)
				);

				attestation
			})
			.collect();

		let job_id1 = (MultiOrigin::Acurast(alice_account_id()), initial_job_id + 1);

		assert_ok!(Acurast::register(
			RuntimeOrigin::signed(alice_account_id()),
			registration.clone(),
		));
		assert_eq!(
			Some(JobStatus::Open),
			AcurastMarketplace::stored_job_status(
				MultiOrigin::Acurast(alice_account_id()),
				initial_job_id + 1
			)
		);

		let job_sources: PlannedExecutions<AccountId, <Test as pallet_acurast::Config>::MaxSlots> =
			processors
				.iter()
				.map(|(processor, _)| PlannedExecution {
					source: processor.clone(),
					start_delay: 0,
				})
				.collect::<Vec<PlannedExecution<AccountId>>>()
				.try_into()
				.unwrap();

		let job_match = Match { job_id: job_id1.clone(), sources: job_sources };

		assert_ok!(AcurastMarketplace::propose_matching(
			RuntimeOrigin::signed(charlie_account_id()),
			vec![job_match.clone()].try_into().unwrap(),
		));

		assert_eq!(
			Some(JobStatus::Matched),
			AcurastMarketplace::stored_job_status(&job_id1.0, &job_id1.1)
		);
		// matcher got rewarded already so job budget decreased
		assert_eq!(264096000, AcurastMarketplace::reserved(&job_id1));

		// pretend current time
		let start_time = registration.schedule.start_time;
		processors.iter().for_each(|(processor, _)| {
			//later(start_time);
			Timestamp::set_timestamp(start_time);
			assert_ok!(AcurastMarketplace::acknowledge_match(
				RuntimeOrigin::signed(processor.clone()),
				job_id1.clone(),
				PubKeys::default(),
			));
		});

		assert_eq!(
			Some(JobStatus::Assigned(processors.len() as u8)),
			AcurastMarketplace::stored_job_status(&job_id1.0, &job_id1.1)
		);

		// job budget decreased by reward worth one execution
		assert_eq!(264096000, AcurastMarketplace::reserved(&job_id1));
		// average reward only updated at end of job
		assert_eq!(
			Some(
				processors
					.iter()
					.fold(0u128, |acc, (p, _)| {
						let asssignment =
							AcurastMarketplace::stored_matches(p, job_id1.clone()).unwrap();
						acc + asssignment.fee_per_execution
					})
					.div(processors.len() as u128)
			),
			AcurastMarketplace::average_reward()
		);
		// reputation still ~50%
		assert_eq!(
			Permill::from_parts(509_803),
			BetaReputation::<u128>::normalize(
				AcurastMarketplace::stored_reputation(processor_account_id()).unwrap()
			)
			.unwrap()
		);
		processors.iter().enumerate().for_each(|(slot, (processor, _))| {
			assert_eq!(
				Some(Assignment {
					execution: ExecutionSpecifier::All,
					slot: slot as u8,
					start_delay: 0,
					fee_per_execution: 1_020_000,
					acknowledged: true,
					sla: SLA { total: 12, met: 0 },
					pub_keys: PubKeys::default(),
				}),
				AcurastMarketplace::stored_matches(processor, job_id1.clone()),
			);
			assert_ok!(AcurastMarketplace::report(
				RuntimeOrigin::signed(processor.clone()),
				job_id1.clone(),
				ExecutionResult::Success(operation_hash())
			));
			assert_eq!(
				Some(Assignment {
					execution: ExecutionSpecifier::All,
					slot: slot as u8,
					start_delay: 0,
					fee_per_execution: 1_020_000,
					acknowledged: true,
					sla: SLA { total: 12, met: 1 },
					pub_keys: PubKeys::default(),
				}),
				AcurastMarketplace::stored_matches(processor, job_id1.clone()),
			);
		});

		// Processors are still assigned after one execution
		assert_eq!(
			Some(JobStatus::Assigned(processors.len() as u8)),
			AcurastMarketplace::stored_job_status(&job_id1.0, &job_id1.1)
		);

		let next_timestamp = registration.schedule.start_time + registration.schedule.interval;
		Timestamp::set_timestamp(next_timestamp);

		assert_ok!(AcurastMarketplace::report(
			RuntimeOrigin::signed(processor_account_id()),
			job_id1.clone(),
			ExecutionResult::Success(operation_hash())
		));
		// job budget decreased by reward worth one execution
		assert_eq!(258996000, AcurastMarketplace::reserved(&job_id1));

		// pretend time moved on
		later(registration.schedule.end_time);

		assert_eq!(258996000, AcurastMarketplace::reserved(&job_id1));

		processors.iter().for_each(|(processor, _)| {
			assert_ok!(AcurastMarketplace::report(
				RuntimeOrigin::signed(processor.clone()),
				job_id1.clone(),
				ExecutionResult::Success(operation_hash())
			));
			assert_eq!(None, AcurastMarketplace::stored_matches(processor, job_id1.clone()),)
		});

		assert_eq!(Some(1), AcurastMarketplace::total_assigned());
	});
}

#[test]
fn test_no_match_schedule_overlap() {
	let now: u64 = 1_671_800_100_000; // 23.12.2022 12:55;

	// 1000 is the smallest amount accepted by T::AssetTransactor::lock_asset for the asset used
	let ad = advertisement(1000, 1, 100_000, 50_000, 8);
	let registration1 = JobRegistrationFor::<Test> {
		script: script(),
		allowed_sources: None,
		allow_only_verified_sources: false,
		schedule: Schedule {
			duration: 5000,
			start_time: 1_671_800_400_000, // 23.12.2022 13:00
			end_time: 1_671_804_000_000,   // 23.12.2022 14:00 (one hour later)
			interval: 1_800_000,           // 30min -> 2 executions fit
			max_start_delay: 5000,
		},
		memory: 5_000u32,
		network_requests: 5,
		storage: 20_000u32,
		required_modules: JobModules::default(),
		extra: RegistrationExtra {
			requirements: JobRequirements {
				assignment_strategy: AssignmentStrategy::Single(None),
				slots: 1,
				reward: 3_000_000 * 2,
				min_reputation: None,
				processor_version: None,
				runtime: Runtime::NodeJS,
			},
		},
	};

	let registration2 = JobRegistrationFor::<Test> {
		script: script_random_value(),
		allowed_sources: None,
		allow_only_verified_sources: false,
		schedule: Schedule {
			duration: 5000,
			start_time: 1_671_802_200_000, // 23.12.2022 13:30
			end_time: 1_671_805_800_000,   // 23.12.2022 14:30 (one hour later)
			interval: 1_200_000,           // 20min -> 3 executions fit
			max_start_delay: 5000,
		},
		memory: 5_000u32,
		network_requests: 5,
		storage: 20_000u32,
		required_modules: JobModules::default(),
		extra: RegistrationExtra {
			requirements: JobRequirements {
				assignment_strategy: AssignmentStrategy::Single(None),
				slots: 1,
				reward: 3_000_000 * 2,
				min_reputation: None,
				processor_version: None,
				runtime: Runtime::NodeJS,
			},
		},
	};

	ExtBuilder.build().execute_with(|| {
		let initial_job_id = Acurast::job_id_sequence();
		let job_id1 = (MultiOrigin::Acurast(alice_account_id()), initial_job_id + 1);
		let job_id2 = (MultiOrigin::Acurast(alice_account_id()), initial_job_id + 2);

		// pretend current time
		assert_ok!(Timestamp::set(RuntimeOrigin::none(), now));
		assert_ok!(AcurastMarketplace::advertise(
			RuntimeOrigin::signed(processor_account_id()),
			ad.clone(),
		));

		// register first job
		assert_ok!(Acurast::register(
			RuntimeOrigin::signed(alice_account_id()),
			registration1.clone(),
		));
		assert_eq!(
			Some(JobStatus::Open),
			AcurastMarketplace::stored_job_status(&job_id1.0, &job_id1.1)
		);

		// register second job
		assert_ok!(Acurast::register(
			RuntimeOrigin::signed(alice_account_id()),
			registration2.clone(),
		));
		assert_eq!(
			Some(JobStatus::Open),
			AcurastMarketplace::stored_job_status(&job_id1.0, job_id1.1 + 1)
		);

		// the first job matches because capacity left
		let m = Match {
			job_id: job_id1.clone(),
			sources: bounded_vec![PlannedExecution {
				source: processor_account_id(),
				start_delay: 0,
			}],
		};
		assert_ok!(AcurastMarketplace::propose_matching(
			RuntimeOrigin::signed(charlie_account_id()),
			vec![m.clone()].try_into().unwrap(),
		));

		// this one does not match anymore
		let m2 = Match {
			job_id: job_id2.clone(),
			sources: bounded_vec![PlannedExecution {
				source: processor_account_id(),
				start_delay: 0,
			}],
		};
		assert_err!(
			AcurastMarketplace::propose_matching(
				RuntimeOrigin::signed(charlie_account_id()),
				vec![m2.clone()].try_into().unwrap(),
			),
			Error::<Test>::ScheduleOverlapInMatch
		);

		assert_eq!(
			events(),
			[
				RuntimeEvent::AcurastMarketplace(crate::Event::AdvertisementStoredV2(
					processor_account_id()
				)),
				RuntimeEvent::Balances(pallet_balances::Event::Transfer {
					from: alice_account_id(),
					to: pallet_acurast_acount(),
					amount: 12_000_000
				}),
				RuntimeEvent::Acurast(pallet_acurast::Event::JobRegistrationStoredV2(
					job_id1.clone()
				)),
				RuntimeEvent::Balances(pallet_balances::Event::Transfer {
					from: alice_account_id(),
					to: pallet_acurast_acount(),
					amount: 18_000_000
				}),
				RuntimeEvent::Acurast(pallet_acurast::Event::JobRegistrationStoredV2((
					job_id2.0.clone(),
					job_id2.1
				))),
				RuntimeEvent::AcurastMarketplace(crate::Event::JobRegistrationMatchedV2(
					job_id1.clone()
				)),
				RuntimeEvent::Balances(pallet_balances::Event::Transfer {
					from: pallet_acurast_acount(),
					to: pallet_fees_account(),
					amount: 58800
				}),
				RuntimeEvent::Balances(pallet_balances::Event::Transfer {
					from: pallet_acurast_acount(),
					to: charlie_account_id(),
					amount: 137200
				}),
				// no match event for second
			]
		);
	});
}

#[test]
fn test_no_match_insufficient_reputation() {
	let now: u64 = 1_671_800_100_000; // 23.12.2022 12:55;

	// 1000 is the smallest amount accepted by T::AssetTransactor::lock_asset for the asset used
	let ad = advertisement(1000, 1, 100_000, 50_000, 8);
	let registration1 = JobRegistrationFor::<Test> {
		script: script(),
		allowed_sources: None,
		allow_only_verified_sources: false,
		schedule: Schedule {
			duration: 5000,
			start_time: 1_671_800_400_000, // 23.12.2022 13:00
			end_time: 1_671_804_000_000,   // 23.12.2022 14:00 (one hour later)
			interval: 1_800_000,           // 30min -> 2 executions fit
			max_start_delay: 5000,
		},
		memory: 5_000u32,
		network_requests: 5,
		storage: 20_000u32,
		required_modules: JobModules::default(),
		extra: RegistrationExtra {
			requirements: JobRequirements {
				assignment_strategy: AssignmentStrategy::Single(None),
				slots: 1,
				reward: 3_000_000 * 2,
				min_reputation: Some(1_000_000),
				processor_version: None,
				runtime: Runtime::NodeJS,
			},
		},
	};

	ExtBuilder.build().execute_with(|| {
		let initial_job_id = Acurast::job_id_sequence();
		let job_id = (MultiOrigin::Acurast(alice_account_id()), initial_job_id + 1);

		// pretend current time
		assert_ok!(Timestamp::set(RuntimeOrigin::none(), now));
		assert_ok!(AcurastMarketplace::advertise(
			RuntimeOrigin::signed(processor_account_id()),
			ad.clone(),
		));

		// register job
		assert_ok!(Acurast::register(
			RuntimeOrigin::signed(alice_account_id()),
			registration1.clone(),
		));
		assert_eq!(
			Some(JobStatus::Open),
			AcurastMarketplace::stored_job_status(&job_id.0, job_id.1)
		);

		// the job matches except insufficient reputation
		let m = Match {
			job_id: job_id.clone(),
			sources: bounded_vec![PlannedExecution {
				source: processor_account_id(),
				start_delay: 0,
			}],
		};
		assert_err!(
			AcurastMarketplace::propose_matching(
				RuntimeOrigin::signed(charlie_account_id()),
				vec![m.clone()].try_into().unwrap(),
			),
			Error::<Test>::InsufficientReputationInMatch
		);

		assert_eq!(
			events(),
			[
				RuntimeEvent::AcurastMarketplace(crate::Event::AdvertisementStoredV2(
					processor_account_id()
				)),
				RuntimeEvent::Balances(pallet_balances::Event::Transfer {
					from: alice_account_id(),
					to: pallet_acurast_acount(),
					amount: 12_000_000
				}),
				RuntimeEvent::Acurast(pallet_acurast::Event::JobRegistrationStoredV2(
					job_id.clone()
				)),
				// no match event for job
			]
		);
	});
}

#[test]
fn test_report_afer_last_report() {
	let now: u64 = 1_671_800_100_000; // 23.12.2022 12:55;

	// 1000 is the smallest amount accepted by T::AssetTransactor::lock_asset for the asset used
	let ad = advertisement(1000, 1, 100_000, 50_000, 8);
	let registration = JobRegistrationFor::<Test> {
		script: script(),
		allowed_sources: None,
		allow_only_verified_sources: false,
		schedule: Schedule {
			duration: 5000,
			start_time: 1_671_800_400_000, // 23.12.2022 13:00
			end_time: 1_671_804_000_000,   // 23.12.2022 14:00 (one hour later)
			interval: 1_800_000,           // 30min
			max_start_delay: 5000,
		},
		memory: 5_000u32,
		network_requests: 5,
		storage: 20_000u32,
		required_modules: JobModules::default(),
		extra: RegistrationExtra {
			requirements: JobRequirements {
				assignment_strategy: AssignmentStrategy::Single(None),
				slots: 1,
				reward: 3_000_000 * 2,
				min_reputation: None,
				processor_version: None,
				runtime: Runtime::NodeJS,
			},
		},
	};

	ExtBuilder.build().execute_with(|| {
		let initial_job_id = Acurast::job_id_sequence();
		let job_id = (MultiOrigin::Acurast(alice_account_id()), initial_job_id + 1);

		// pretend current time
		assert_ok!(Timestamp::set(RuntimeOrigin::none(), now));
		assert_ok!(AcurastMarketplace::advertise(
			RuntimeOrigin::signed(processor_account_id()),
			ad.clone(),
		));
		assert_eq!(
			Some(AdvertisementRestriction {
				max_memory: 50_000,
				network_request_quota: 8,
				storage_capacity: 100_000,
				allowed_consumers: ad.allowed_consumers.clone(),
				available_modules: JobModules::default(),
			}),
			AcurastMarketplace::stored_advertisement(processor_account_id())
		);

		assert_ok!(Acurast::register(
			RuntimeOrigin::signed(alice_account_id()),
			registration.clone(),
		));

		let m = Match {
			job_id: job_id.clone(),
			sources: bounded_vec![PlannedExecution {
				source: processor_account_id(),
				start_delay: 0,
			}],
		};
		assert_ok!(AcurastMarketplace::propose_matching(
			RuntimeOrigin::signed(charlie_account_id()),
			vec![m.clone()].try_into().unwrap(),
		));

		assert_ok!(AcurastMarketplace::acknowledge_match(
			RuntimeOrigin::signed(processor_account_id()),
			job_id.clone(),
			PubKeys::default(),
		));

		// report twice with success
		// -------------------------

		// pretend time moved on
		let mut iter = registration.schedule.iter(0).unwrap();
		later(iter.next().unwrap() + 1000);
		assert_ok!(AcurastMarketplace::report(
			RuntimeOrigin::signed(processor_account_id()),
			job_id.clone(),
			ExecutionResult::Success(operation_hash())
		));

		// pretend time moved on
		later(iter.next().unwrap() + 1000);
		assert_ok!(AcurastMarketplace::report(
			RuntimeOrigin::signed(processor_account_id()),
			job_id.clone(),
			ExecutionResult::Success(operation_hash())
		));

		// third report is illegal!
		later(registration.schedule.range(0).1 + 1000);
		assert_err!(
			AcurastMarketplace::report(
				RuntimeOrigin::signed(processor_account_id()),
				job_id.clone(),
				ExecutionResult::Success(operation_hash())
			),
			Error::<Test>::ReportFromUnassignedSource
		);

		assert_eq!(
			events(),
			[
				RuntimeEvent::AcurastMarketplace(crate::Event::AdvertisementStoredV2(
					processor_account_id()
				)),
				RuntimeEvent::Balances(pallet_balances::Event::Transfer {
					from: alice_account_id(),
					to: pallet_acurast_acount(),
					amount: 12_000_000
				}),
				RuntimeEvent::Acurast(pallet_acurast::Event::JobRegistrationStoredV2(
					job_id.clone()
				)),
				RuntimeEvent::AcurastMarketplace(crate::Event::JobRegistrationMatchedV2(
					job_id.clone()
				)),
				RuntimeEvent::Balances(pallet_balances::Event::Transfer {
					from: pallet_acurast_acount(),
					to: pallet_fees_account(),
					amount: 58_800
				}),
				RuntimeEvent::Balances(pallet_balances::Event::Transfer {
					from: pallet_acurast_acount(),
					to: charlie_account_id(),
					amount: 137_200
				}),
				RuntimeEvent::AcurastMarketplace(crate::Event::JobRegistrationAssignedV2(
					job_id.clone(),
					processor_account_id(),
				)),
				RuntimeEvent::Balances(pallet_balances::Event::Withdraw {
					who: pallet_acurast_acount(),
					amount: 5_020_000
				}),
				RuntimeEvent::AcurastMarketplace(crate::Event::ExecutionSuccess(
					job_id.clone(),
					operation_hash()
				)),
				RuntimeEvent::AcurastMarketplace(crate::Event::ReportedV2(
					job_id.clone(),
					processor_account_id(),
				)),
				RuntimeEvent::Balances(pallet_balances::Event::Withdraw {
					who: pallet_acurast_acount(),
					amount: 5_020_000
				}),
				RuntimeEvent::AcurastMarketplace(crate::Event::ExecutionSuccess(
					job_id.clone(),
					operation_hash()
				)),
				RuntimeEvent::AcurastMarketplace(crate::Event::ReportedV2(
					job_id.clone(),
					processor_account_id(),
				)),
			]
		);
	});
}

#[test]
fn test_deploy_reuse_keys_same_editor() {
	let now: u64 = 1_671_800_100_000; // 23.12.2022 12:55;

	// 1000 is the smallest amount accepted by T::AssetTransactor::lock_asset for the asset used
	let ad = advertisement(1000, 1, 100_000, 50_000, 8);
	let registration1 = JobRegistrationFor::<Test> {
		script: script(),
		allowed_sources: None,
		allow_only_verified_sources: false,
		schedule: Schedule {
			duration: 5000,
			start_time: 1_671_800_400_000, // 23.12.2022 13:00
			end_time: 1_671_804_000_000,   // 23.12.2022 14:00 (one hour later)
			interval: 1_800_000,           // 30min
			max_start_delay: 5000,
		},
		memory: 5_000u32,
		network_requests: 5,
		storage: 20_000u32,
		required_modules: JobModules::default(),
		extra: RegistrationExtra {
			requirements: JobRequirements {
				assignment_strategy: AssignmentStrategy::Single(Some(bounded_vec![
					PlannedExecution { source: processor_account_id(), start_delay: 0 },
					PlannedExecution { source: processor_2_account_id(), start_delay: 0 }
				])),
				slots: 2,
				reward: 3_000_000 * 2,
				min_reputation: None,
				processor_version: None,
				runtime: Runtime::NodeJS,
			},
		},
	};
	let registration2 = JobRegistrationFor::<Test> {
		script: script_random_value(),
		allowed_sources: None,
		allow_only_verified_sources: false,
		schedule: Schedule {
			duration: 5000,
			start_time: 1_671_800_400_000, // 23.12.2022 13:00
			end_time: 1_671_804_000_000,   // 23.12.2022 14:00 (one hour later)
			interval: 1_800_000,           // 30min
			max_start_delay: 10_000,
		},
		memory: 5_000u32,
		network_requests: 5,
		storage: 20_000u32,
		required_modules: JobModules::default(),
		extra: RegistrationExtra {
			requirements: JobRequirements {
				assignment_strategy: AssignmentStrategy::Single(None),
				slots: 1,
				reward: 3_000_000 * 2,
				min_reputation: None,
				processor_version: None,
				runtime: Runtime::NodeJS,
			},
		},
	};

	let key_id =
		H256::from_slice(&hex!("e2259508cea453c056f02d233c2c94b5aae27b401baabd1bbccaacfb230075a5"));

	ExtBuilder.build().execute_with(|| {
		let initial_job_id = Acurast::job_id_sequence();

		// pretend current time
		later(now);

		assert_ok!(AcurastMarketplace::advertise(
			RuntimeOrigin::signed(processor_account_id()),
			ad.clone(),
		));
		assert_ok!(AcurastMarketplace::advertise(
			RuntimeOrigin::signed(processor_2_account_id()),
			ad.clone(),
		));

		let job_id1: (MultiOrigin<sp_core::crypto::AccountId32>, u128) =
			(MultiOrigin::Acurast(alice_account_id()), initial_job_id + 1);

		let deployhemt_hash1 = <Test as Config>::DeploymentHashing::hash(
			&(job_id1.0.clone(), registration1.script.clone()).encode(),
		);

		assert_ok!(AcurastMarketplace::deploy(
			RuntimeOrigin::signed(alice_account_id()),
			registration1.clone(),
			pallet_acurast::ScriptMutability::Mutable(None),
			None,
			None
		));

		assert_eq!(Some(key_id), AcurastMarketplace::job_key_ids(job_id1.clone()));
		// assert_eq!(vec![key_id], <crate::pallet::DeploymentKeyIds::<Test>>::iter_keys().collect::<Vec<H256>>());
		assert_eq!(Some(key_id), AcurastMarketplace::deployment_key_ids(deployhemt_hash1));

		// deregister job1 to demonstrate that even then we can extend job1 with job2
		assert_ok!(Acurast::deregister(RuntimeOrigin::signed(alice_account_id()), job_id1.1));

		// register different script as extension
		let job_id2: (MultiOrigin<sp_core::crypto::AccountId32>, u128) =
			(MultiOrigin::Acurast(alice_account_id()), initial_job_id + 2);

		let deployhemt_hash2 = <Test as Config>::DeploymentHashing::hash(
			&(job_id2.0.clone(), registration2.script.clone()).encode(),
		);

		assert_ok!(AcurastMarketplace::deploy(
			RuntimeOrigin::signed(alice_account_id()),
			registration2,
			pallet_acurast::ScriptMutability::Mutable(None),
			Some(job_id1.clone()),
			None
		));

		// job key is same for job1 and job2
		assert_eq!(Some(key_id), AcurastMarketplace::job_key_ids(job_id2.clone()));
		// assert_eq!(vec![key_id], <crate::pallet::DeploymentKeyIds::<Test>>::iter_keys().collect::<Vec<H256>>());
		// job1's deployment key no longer points to any KeyId
		assert_eq!(None, AcurastMarketplace::deployment_key_ids(deployhemt_hash1));
		// the new deployment_hash of job2 now points to the key_id previously "owned" by the predecessor deployment
		assert_eq!(Some(key_id), AcurastMarketplace::deployment_key_ids(deployhemt_hash2));
	});
}

#[test]
fn test_deploy_reuse_keys_different_editor() {
	let now: u64 = 1_671_800_100_000; // 23.12.2022 12:55;

	// 1000 is the smallest amount accepted by T::AssetTransactor::lock_asset for the asset used
	let ad = advertisement(1000, 1, 100_000, 50_000, 8);
	let registration1 = JobRegistrationFor::<Test> {
		script: script(),
		allowed_sources: None,
		allow_only_verified_sources: false,
		schedule: Schedule {
			duration: 5000,
			start_time: 1_671_800_400_000, // 23.12.2022 13:00
			end_time: 1_671_804_000_000,   // 23.12.2022 14:00 (one hour later)
			interval: 1_800_000,           // 30min
			max_start_delay: 5000,
		},
		memory: 5_000u32,
		network_requests: 5,
		storage: 20_000u32,
		required_modules: JobModules::default(),
		extra: RegistrationExtra {
			requirements: JobRequirements {
				assignment_strategy: AssignmentStrategy::Single(Some(bounded_vec![
					PlannedExecution { source: processor_account_id(), start_delay: 0 },
					PlannedExecution { source: processor_2_account_id(), start_delay: 0 }
				])),
				slots: 2,
				reward: 3_000_000 * 2,
				min_reputation: None,
				processor_version: None,
				runtime: Runtime::NodeJS,
			},
		},
	};
	let registration2 = JobRegistrationFor::<Test> {
		script: script(),
		allowed_sources: None,
		allow_only_verified_sources: false,
		schedule: Schedule {
			duration: 5000,
			start_time: 1_671_800_400_000, // 23.12.2022 13:00
			end_time: 1_671_804_000_000,   // 23.12.2022 14:00 (one hour later)
			interval: 1_800_000,           // 30min
			max_start_delay: 10_000,
		},
		memory: 5_000u32,
		network_requests: 5,
		storage: 20_000u32,
		required_modules: JobModules::default(),
		extra: RegistrationExtra {
			requirements: JobRequirements {
				assignment_strategy: AssignmentStrategy::Single(None),
				slots: 1,
				reward: 3_000_000 * 2,
				min_reputation: None,
				processor_version: None,
				runtime: Runtime::NodeJS,
			},
		},
	};

	let key_id =
		H256::from_slice(&hex!("e2259508cea453c056f02d233c2c94b5aae27b401baabd1bbccaacfb230075a5"));

	ExtBuilder.build().execute_with(|| {
		let initial_job_id = Acurast::job_id_sequence();

		// pretend current time
		later(now);

		assert_ok!(AcurastMarketplace::advertise(
			RuntimeOrigin::signed(processor_account_id()),
			ad.clone(),
		));
		assert_ok!(AcurastMarketplace::advertise(
			RuntimeOrigin::signed(processor_2_account_id()),
			ad.clone(),
		));

		let job_id1: (MultiOrigin<sp_core::crypto::AccountId32>, u128) =
			(MultiOrigin::Acurast(alice_account_id()), initial_job_id + 1);

		let deployhemt_hash = <Test as Config>::DeploymentHashing::hash(
			&(job_id1.0.clone(), registration1.script.clone()).encode(),
		);

		assert_ok!(AcurastMarketplace::deploy(
			RuntimeOrigin::signed(alice_account_id()),
			registration1.clone(),
			pallet_acurast::ScriptMutability::Mutable(Some(bob_account_id())),
			None,
			None
		));

		assert_eq!(Some(key_id), AcurastMarketplace::job_key_ids(job_id1.clone()));
		// assert_eq!(vec![key_id], <crate::pallet::DeploymentKeyIds::<Test>>::iter_keys().collect::<Vec<H256>>());
		assert_eq!(Some(key_id), AcurastMarketplace::deployment_key_ids(deployhemt_hash));

		// deregister job1 to demonstrate that even then we can extend job1 with job2
		assert_ok!(Acurast::deregister(RuntimeOrigin::signed(alice_account_id()), job_id1.1));

		// register same script as extension (is possible even job1's editor is not equals owner)
		let job_id2: (MultiOrigin<sp_core::crypto::AccountId32>, u128) =
			(MultiOrigin::Acurast(alice_account_id()), initial_job_id + 2);
		assert_ok!(AcurastMarketplace::deploy(
			RuntimeOrigin::signed(alice_account_id()),
			registration2,
			pallet_acurast::ScriptMutability::Mutable(Some(bob_account_id())),
			Some(job_id1.clone()),
			None
		));

		// job key is same for job1 and job2
		assert_eq!(Some(key_id), AcurastMarketplace::job_key_ids(job_id2.clone()));
		// assert_eq!(vec![key_id], <crate::pallet::DeploymentKeyIds::<Test>>::iter_keys().collect::<Vec<H256>>());
		// job1's equal job2's deployment key still point to same key_id
		assert_eq!(Some(key_id), AcurastMarketplace::deployment_key_ids(deployhemt_hash));
	});
}

#[test]
fn test_deploy_reuse_keys_different_editor_script_edit_fails() {
	let now: u64 = 1_671_800_100_000; // 23.12.2022 12:55;

	// 1000 is the smallest amount accepted by T::AssetTransactor::lock_asset for the asset used
	let ad = advertisement(1000, 1, 100_000, 50_000, 8);
	let registration1 = JobRegistrationFor::<Test> {
		script: script(),
		allowed_sources: None,
		allow_only_verified_sources: false,
		schedule: Schedule {
			duration: 5000,
			start_time: 1_671_800_400_000, // 23.12.2022 13:00
			end_time: 1_671_804_000_000,   // 23.12.2022 14:00 (one hour later)
			interval: 1_800_000,           // 30min
			max_start_delay: 5000,
		},
		memory: 5_000u32,
		network_requests: 5,
		storage: 20_000u32,
		required_modules: JobModules::default(),
		extra: RegistrationExtra {
			requirements: JobRequirements {
				assignment_strategy: AssignmentStrategy::Single(Some(bounded_vec![
					PlannedExecution { source: processor_account_id(), start_delay: 0 },
					PlannedExecution { source: processor_2_account_id(), start_delay: 0 }
				])),
				slots: 2,
				reward: 3_000_000 * 2,
				min_reputation: None,
				processor_version: None,
				runtime: Runtime::NodeJS,
			},
		},
	};
	let registration2 = JobRegistrationFor::<Test> {
		script: script_random_value(),
		allowed_sources: None,
		allow_only_verified_sources: false,
		schedule: Schedule {
			duration: 5000,
			start_time: 1_671_800_400_000, // 23.12.2022 13:00
			end_time: 1_671_804_000_000,   // 23.12.2022 14:00 (one hour later)
			interval: 1_800_000,           // 30min
			max_start_delay: 10_000,
		},
		memory: 5_000u32,
		network_requests: 5,
		storage: 20_000u32,
		required_modules: JobModules::default(),
		extra: RegistrationExtra {
			requirements: JobRequirements {
				assignment_strategy: AssignmentStrategy::Single(None),
				slots: 1,
				reward: 3_000_000 * 2,
				min_reputation: None,
				processor_version: None,
				runtime: Runtime::NodeJS,
			},
		},
	};

	let key_id =
		H256::from_slice(&hex!("e2259508cea453c056f02d233c2c94b5aae27b401baabd1bbccaacfb230075a5"));

	ExtBuilder.build().execute_with(|| {
		let initial_job_id = Acurast::job_id_sequence();

		// pretend current time
		later(now);

		assert_ok!(AcurastMarketplace::advertise(
			RuntimeOrigin::signed(processor_account_id()),
			ad.clone(),
		));
		assert_ok!(AcurastMarketplace::advertise(
			RuntimeOrigin::signed(processor_2_account_id()),
			ad.clone(),
		));

		let job_id1: (MultiOrigin<sp_core::crypto::AccountId32>, u128) =
			(MultiOrigin::Acurast(alice_account_id()), initial_job_id + 1);

		let deployhemt_hash1 = <Test as Config>::DeploymentHashing::hash(
			&(job_id1.0.clone(), registration1.script.clone()).encode(),
		);

		assert_ok!(AcurastMarketplace::deploy(
			RuntimeOrigin::signed(alice_account_id()),
			registration1.clone(),
			pallet_acurast::ScriptMutability::Mutable(Some(bob_account_id())),
			None,
			None
		));

		assert_eq!(Some(key_id), AcurastMarketplace::job_key_ids(job_id1.clone()));
		// assert_eq!(vec![key_id], <crate::pallet::DeploymentKeyIds::<Test>>::iter_keys().collect::<Vec<H256>>());
		assert_eq!(Some(key_id), AcurastMarketplace::deployment_key_ids(deployhemt_hash1));

		// deregister job1 to demonstrate that even then we can extend job1 with job2
		assert_ok!(Acurast::deregister(RuntimeOrigin::signed(alice_account_id()), job_id1.1));

		// register different script fails because editor of job1 is not equals owner
		assert_err!(
			AcurastMarketplace::deploy(
				RuntimeOrigin::signed(alice_account_id()),
				registration2,
				pallet_acurast::ScriptMutability::Mutable(None),
				Some(job_id1),
				None
			),
			Error::<Test>::OnlyEditorCanEditScript
		);
	});
}

fn next_block() {
	if System::block_number() >= 1 {
		// pallet_acurast_marketplace::on_finalize(System::block_number());
		Timestamp::on_finalize(System::block_number());
	}
	System::set_block_number(System::block_number() + 1);
	Timestamp::on_initialize(System::block_number());
}

/// A helper function to move time on in tests. It ensures `Timestamp::set` is only called once per block by advancing the block otherwise.
fn later(now: u64) {
	// If this is not the very first timestamp ever set, we always advance the block before setting new time
	// this is because setting it twice in a block is not legal
	if Timestamp::get() > 0 {
		// pretend block was finalized
		let b = System::block_number();
		next_block(); // we cannot set time twice in same block
		assert_eq!(b + 1, System::block_number());
	}
	// pretend time moved on
	assert_ok!(Timestamp::set(RuntimeOrigin::none(), now));
}
