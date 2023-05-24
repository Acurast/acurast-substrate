use sp_runtime::Permill;

use emulations::runtimes::acurast_runtime::{
	pallet_acurast::MultiOrigin,
	pallet_acurast_marketplace::{ExecutionResult, PlannedExecution, RegistrationExtra},
};
use reputation::{BetaReputation, ReputationEngine};

use crate::tests::{
	acurast_runtime::{pallet_acurast, pallet_acurast::JobRegistration},
	*,
};

#[test]
fn fund_register_job() {
	use acurast_runtime::Runtime as AcurastRuntime;

	let pallet_account: <AcurastRuntime as frame_system::Config>::AccountId =
		<AcurastRuntime as pallet_acurast::Config>::PalletId::get().into_account_truncating();

	let reward_per_execution = 20_000;
	let registration = JobRegistration::<AccountId32, _> {
		script: script(),
		allowed_sources: None,
		allow_only_verified_sources: true,
		schedule: Schedule {
			duration: 5000,
			start_time: 1_671_800_400_000, // 23.12.2022 13:00
			end_time: 1_671_804_000_000,   // 23.12.2022 14:00 (one hour later)
			interval: 1_800_000,           // 30min
			max_start_delay: 5000,
		},
		memory: 5_000u32,
		network_requests: 5,
		required_modules: vec![].try_into().unwrap(),
		storage: 20_000u32,
		extra: RegistrationExtra {
			requirements: JobRequirements {
				slots: 1,
				reward: test_asset(reward_per_execution),
				min_reputation: Some(500_000),
				instant_match: None,
			},
			expected_fulfillment_fee: 10000,
		},
	};

	AcurastParachain::execute_with(|| {
		// register job
		{
			let balance_test_token = AcurastAssetsInternal::balance(TEST_TOKEN_ID, &ALICE);
			assert_eq!(balance_test_token, TEST_TOKEN_INITIAL_BALANCE);

			assert_ok!(Acurast::register(
				acurast_runtime::RuntimeOrigin::signed(ALICE), // ALICE's account should now be funded
				registration,
			));

			let balance_test_token = AcurastAssetsInternal::balance(TEST_TOKEN_ID, &ALICE);
			assert_eq!(balance_test_token, TEST_TOKEN_INITIAL_BALANCE - 2 * reward_per_execution);
			// reward worth 2 executions
		}
		// check job event and balances
		{
			let _events: Vec<String> = acurast_runtime::System::events()
				.iter()
				.map(|e| format!("{:?}", e.event))
				.collect();
			let _bob_balance_test_token = AcurastAssetsInternal::balance(TEST_TOKEN_ID, &ALICE);
			let _ferdie_balance_test_token = AcurastAssetsInternal::balance(TEST_TOKEN_ID, &FERDIE);
			let _ferdie_balance_false =
				AcurastAssetsInternal::balance(STATEMINT_NATIVE_ID, &FERDIE);
			let _ferdie_balance_native = acurast_runtime::Balances::free_balance(&FERDIE);
			let _pallet_balance_test_token =
				AcurastAssetsInternal::balance(TEST_TOKEN_ID, pallet_account.clone());
		}
	});
}

#[test]
fn register_match_report_job() {
	use acurast_runtime::Runtime as AcurastRuntime;

	let pallet_account: <AcurastRuntime as frame_system::Config>::AccountId =
		<AcurastRuntime as pallet_acurast::Config>::PalletId::get().into_account_truncating();

	let now: u64 = 1_671_789_600_000; // 23.12.2022 10:00;

	let ad = advertisement(1000, 1, 100_000, 50_000, 8, SchedulingWindow::Delta(2_628_000_000)); // 1 month scheduling window
	let reward_per_execution = 10_000_000;
	let registration = JobRegistration {
		script: script(),
		allowed_sources: None,
		allow_only_verified_sources: true,
		schedule: Schedule {
			duration: 5000,
			start_time: 1_671_800_400_000, // 23.12.2022 13:00
			end_time: 1_671_804_000_000,   // 23.12.2022 14:00 (one hour later)
			interval: 1_800_000,           // 30min
			max_start_delay: 5000,
		},
		memory: 5_000u32,
		network_requests: 5,
		required_modules: vec![].try_into().unwrap(),
		storage: 20_000u32,
		extra: RegistrationExtra {
			requirements: JobRequirements {
				slots: 1,
				reward: test_asset(reward_per_execution),
				min_reputation: Some(500_000),
				instant_match: Some(vec![PlannedExecution { source: BOB, start_delay: 0 }]),
			},
			expected_fulfillment_fee: 10000,
		},
	};
	// base_fee_per_execution + duration * fee_per_millisecond + storage * fee_per_storage_byte
	let price_per_execution = 0 + 5000 * 1000 + 20_000 * 1;

	// advertise resources
	AcurastParachain::execute_with(|| {
		// advertise
		assert_ok!(AcurastMarketplace::advertise(
			acurast_runtime::RuntimeOrigin::signed(BOB),
			ad.clone(),
		));

		// pretend current time
		later(now);

		// register job
		let job_id = {
			let balance_ferdie_test_token = AcurastAssetsInternal::balance(TEST_TOKEN_ID, &FERDIE);
			assert_eq!(balance_ferdie_test_token, TEST_TOKEN_INITIAL_BALANCE);

			assert_ok!(Acurast::register(
				acurast_runtime::RuntimeOrigin::signed(FERDIE), // FERDIE is a pre-funded via Genesis
				registration.clone(),
			));

			let balance_test_token = AcurastAssetsInternal::balance(TEST_TOKEN_ID, &FERDIE);
			// check we now have lower balance corresponding reward worth 2 executions
			assert_eq!(balance_test_token, TEST_TOKEN_INITIAL_BALANCE - 2 * reward_per_execution);

			(MultiOrigin::Acurast(FERDIE), Acurast::job_id_sequence())
		};

		// check job event and balances
		{
			let _events: Vec<String> = acurast_runtime::System::events()
				.iter()
				.map(|e| format!("{:?}", e.event))
				.collect();
			let _bob_balance_test_token = AcurastAssetsInternal::balance(TEST_TOKEN_ID, &ALICE);
			let _ferdie_balance_test_token = AcurastAssetsInternal::balance(TEST_TOKEN_ID, &FERDIE);
			let _ferdie_balance_false =
				AcurastAssetsInternal::balance(STATEMINT_NATIVE_ID, &FERDIE);
			let _ferdie_balance_native = acurast_runtime::Balances::free_balance(&FERDIE);
			let _pallet_balance_test_token =
				AcurastAssetsInternal::balance(TEST_TOKEN_ID, pallet_account.clone());
		}

		// acknowledge
		assert_ok!(AcurastMarketplace::acknowledge_match(
			acurast_runtime::RuntimeOrigin::signed(BOB).into(),
			job_id.clone(),
			pallet_acurast_marketplace::PubKeys::default()
		));

		// reports
		{
			let balance_test_token_0 = AcurastAssetsInternal::balance(TEST_TOKEN_ID, &ALICE);
			assert_eq!(balance_test_token_0, TEST_TOKEN_INITIAL_BALANCE);

			let mut iter = registration.schedule.iter(0).unwrap();

			later(iter.next().unwrap() + 1000);
			assert_ok!(AcurastMarketplace::report(
				acurast_runtime::RuntimeOrigin::signed(BOB).into(),
				job_id.clone(),
				ExecutionResult::Success(operation_hash())
			));
			// reputation still ~50%
			assert_eq!(
				BetaReputation::<u128>::normalize(
					AcurastMarketplace::stored_reputation(BOB, test_token_asset_id()).unwrap()
				)
				.unwrap(),
				Permill::from_parts(509_803)
			);

			let balance_test_token_1 = AcurastAssetsInternal::balance(TEST_TOKEN_ID, &ALICE);
			// check BOB's manager now has higher balance corresponding reward gained
			assert_eq!(
				balance_test_token_1,
				TEST_TOKEN_INITIAL_BALANCE + price_per_execution -
					FeeManagement::get_fee_percentage().mul_floor(price_per_execution)
			);

			later(iter.next().unwrap() + 1000);
			assert_ok!(AcurastMarketplace::report(
				acurast_runtime::RuntimeOrigin::signed(BOB).into(),
				job_id.clone(),
				ExecutionResult::Success(operation_hash())
			));
			assert_ok!(AcurastMarketplace::finalize_job(
				acurast_runtime::RuntimeOrigin::signed(BOB).into(),
				job_id.clone()
			));
			// reputation increased
			assert_eq!(
				BetaReputation::<u128>::normalize(
					AcurastMarketplace::stored_reputation(BOB, test_token_asset_id()).unwrap()
				)
				.unwrap(),
				Permill::from_parts(763_424)
			);

			let balance_test_token_2 = AcurastAssetsInternal::balance(TEST_TOKEN_ID, &ALICE);
			// check we now have higher balance corresponding reward gained
			assert_eq!(
				balance_test_token_2,
				balance_test_token_1 + price_per_execution -
					FeeManagement::get_fee_percentage().mul_floor(price_per_execution)
			);
		}
	});
}

#[test]
fn register_match_report_job2() {
	use acurast_runtime::Runtime as AcurastRuntime;

	let pallet_account: <AcurastRuntime as frame_system::Config>::AccountId =
		<AcurastRuntime as pallet_acurast::Config>::PalletId::get().into_account_truncating();

	let ad = advertisement(1000, 1, 100_000, 50_000, 8, SchedulingWindow::End(1_680_448_761_934));
	// base_fee_per_execution + duration * fee_per_millisecond + storage * fee_per_storage_byte
	let price_per_execution = 0 + 1000 * 1000 + 1 * 0;
	let schedule = Schedule {
		duration: 1000,
		start_time: 1_677_752_518_599,
		end_time: 1_677_752_523_600,
		interval: 1001,
		max_start_delay: 0,
	};
	let count: u128 = schedule.execution_count() as u128;
	assert_eq!(count, 5);
	let reward_per_execution = price_per_execution + 10;
	let registration = JobRegistration {
		script: script(),
		allowed_sources: None,
		allow_only_verified_sources: true,
		schedule: schedule.clone(),
		memory: 0u32,
		network_requests: 0,
		required_modules: vec![].try_into().unwrap(),
		storage: 0u32,
		extra: RegistrationExtra {
			requirements: JobRequirements {
				slots: 1,
				reward: test_asset(reward_per_execution),
				min_reputation: Some(500_000),
				instant_match: Some(vec![PlannedExecution { source: BOB, start_delay: 0 }]),
			},
			expected_fulfillment_fee: 0,
		},
	};

	let now: u64 = schedule.start_time - 100_000;

	// advertise resources
	AcurastParachain::execute_with(|| {
		// advertise
		assert_ok!(AcurastMarketplace::advertise(
			acurast_runtime::RuntimeOrigin::signed(BOB),
			ad.clone(),
		));

		// pretend current time
		later(now);

		// register job
		let job_id = {
			let balance_ferdie_test_token = AcurastAssetsInternal::balance(TEST_TOKEN_ID, &FERDIE);
			assert_eq!(balance_ferdie_test_token, TEST_TOKEN_INITIAL_BALANCE);

			assert_ok!(Acurast::register(
				acurast_runtime::RuntimeOrigin::signed(FERDIE), // FERDIE is a pre-funded via Genesis
				registration.clone(),
			));

			let balance_test_token = AcurastAssetsInternal::balance(TEST_TOKEN_ID, &FERDIE);
			// check we now have lower balance corresponding reward worth 2 executions
			assert_eq!(
				balance_test_token,
				TEST_TOKEN_INITIAL_BALANCE - count * reward_per_execution
			);

			(MultiOrigin::Acurast(FERDIE), Acurast::job_id_sequence())
		};

		// check job event and balances
		{
			let _events: Vec<String> = acurast_runtime::System::events()
				.iter()
				.map(|e| format!("{:?}", e.event))
				.collect();
			let _bob_balance_test_token = AcurastAssetsInternal::balance(TEST_TOKEN_ID, &ALICE);
			let _ferdie_balance_test_token = AcurastAssetsInternal::balance(TEST_TOKEN_ID, &FERDIE);
			let _ferdie_balance_false =
				AcurastAssetsInternal::balance(STATEMINT_NATIVE_ID, &FERDIE);
			let _ferdie_balance_native = acurast_runtime::Balances::free_balance(&FERDIE);
			let _pallet_balance_test_token =
				AcurastAssetsInternal::balance(TEST_TOKEN_ID, pallet_account.clone());
		}

		// acknowledge
		assert_ok!(AcurastMarketplace::acknowledge_match(
			acurast_runtime::RuntimeOrigin::signed(BOB).into(),
			job_id.clone(),
			pallet_acurast_marketplace::PubKeys::default()
		));

		// reports
		{
			let balance_test_token_0 = AcurastAssetsInternal::balance(TEST_TOKEN_ID, &ALICE);
			assert_eq!(balance_test_token_0, TEST_TOKEN_INITIAL_BALANCE);

			let mut iter = registration.schedule.iter(0).unwrap();

			later(iter.next().unwrap() + 1000);
			assert_ok!(AcurastMarketplace::report(
				acurast_runtime::RuntimeOrigin::signed(BOB).into(),
				job_id.clone(),
				ExecutionResult::Success(operation_hash())
			));
			// reputation still ~50%
			assert_eq!(
				BetaReputation::<u128>::normalize(
					AcurastMarketplace::stored_reputation(BOB, test_token_asset_id()).unwrap()
				)
				.unwrap(),
				Permill::from_parts(509_803)
			);

			let balance_test_token_1 = AcurastAssetsInternal::balance(TEST_TOKEN_ID, &ALICE);
			// check BOB's manager now has higher balance corresponding reward gained
			assert_eq!(
				balance_test_token_1,
				TEST_TOKEN_INITIAL_BALANCE + price_per_execution -
					FeeManagement::get_fee_percentage().mul_floor(price_per_execution)
			);

			// DO NOT move time forward since we report again in same block
			iter.next().unwrap();
			assert_ok!(AcurastMarketplace::report(
				acurast_runtime::RuntimeOrigin::signed(BOB).into(),
				job_id.clone(),
				ExecutionResult::Success(operation_hash())
			));
			// reputation still ~50%
			assert_eq!(
				BetaReputation::<u128>::normalize(
					AcurastMarketplace::stored_reputation(BOB, test_token_asset_id()).unwrap()
				)
				.unwrap(),
				Permill::from_parts(509_803)
			);

			let balance_test_token_2 = AcurastAssetsInternal::balance(TEST_TOKEN_ID, &ALICE);
			// check we now have higher balance corresponding reward gained
			assert_eq!(
				balance_test_token_2,
				balance_test_token_1 + price_per_execution -
					FeeManagement::get_fee_percentage().mul_floor(price_per_execution)
			);

			// MISS OUT on 2 submissions
			iter.next().unwrap();
			iter.next().unwrap();

			iter.next().unwrap();
			assert_ok!(AcurastMarketplace::report(
				acurast_runtime::RuntimeOrigin::signed(BOB).into(),
				job_id.clone(),
				ExecutionResult::Success(operation_hash())
			));
			assert_ok!(AcurastMarketplace::finalize_job(
				acurast_runtime::RuntimeOrigin::signed(BOB).into(),
				job_id.clone()
			));
			// reputation increased, but less than in previous test
			assert_eq!(
				BetaReputation::<u128>::normalize(
					AcurastMarketplace::stored_reputation(BOB, test_token_asset_id()).unwrap()
				)
				.unwrap(),
				Permill::from_parts(573_039)
			);

			let balance_test_token_3 = AcurastAssetsInternal::balance(TEST_TOKEN_ID, &ALICE);
			// check we now have higher balance corresponding reward gained
			assert_eq!(
				balance_test_token_3,
				balance_test_token_2 + price_per_execution -
					FeeManagement::get_fee_percentage().mul_floor(price_per_execution)
			);
		}
	});
}

fn next_block() {
	if acurast_runtime::System::block_number() >= 1 {
		acurast_runtime::Timestamp::on_finalize(acurast_runtime::System::block_number());
	}
	acurast_runtime::System::set_block_number(acurast_runtime::System::block_number() + 1);
	acurast_runtime::Timestamp::on_initialize(acurast_runtime::System::block_number());
}

/// A helper function to move time on in tests. It ensures `Timestamp::set` is only called once per block by advancing the block otherwise.
fn later(now: u64) {
	// If this is not the very first timestamp ever set, we always advance the block before setting new time
	// this is because setting it twice in a block is not legal
	if acurast_runtime::Timestamp::get() > 0 {
		// pretend block was finalized
		let b = acurast_runtime::System::block_number();
		next_block(); // we cannot set time twice in same block
		assert_eq!(b + 1, acurast_runtime::System::block_number());
	}
	// pretend time moved on
	assert_ok!(acurast_runtime::Timestamp::set(acurast_runtime::RuntimeOrigin::none(), now));
}
