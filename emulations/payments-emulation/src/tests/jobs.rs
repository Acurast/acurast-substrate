// use pallet_acurast_marketplace::FeeManager;
use emulations::runtimes::acurast_runtime::{
	pallet_acurast_marketplace::{ExecutionResult, PlannedExecution},
	RegistrationExtra,
};

use crate::tests::{acurast_runtime::pallet_acurast, pallet_acurast::JobRegistration, *};

#[test]
fn send_native_and_token() {
	Network::reset();
	// create acurast native token in statemint to pay for execution of xcm
	StatemintParachain::execute_with(|| {
		let result = StatemintAssets::create(
			statemint_runtime::RuntimeOrigin::signed(ALICE),
			codec::Compact(STATEMINT_NATIVE_ID),
			sp_runtime::MultiAddress::Id(ALICE),
			10,
		);
		assert_ok!(result);

		let result = StatemintAssets::mint(
			statemint_runtime::RuntimeOrigin::signed(ALICE),
			codec::Compact(STATEMINT_NATIVE_ID),
			sp_runtime::MultiAddress::Id(ALICE),
			INITIAL_BALANCE,
		);
		assert_ok!(result);

		let alice_balance = StatemintAssets::balance(STATEMINT_NATIVE_ID, &ALICE);

		assert_eq!(alice_balance, INITIAL_BALANCE);
	});

	// create another token in statemint to pay for job
	StatemintParachain::execute_with(|| {
		let result = StatemintAssets::create(
			statemint_runtime::RuntimeOrigin::signed(ALICE),
			codec::Compact(TEST_TOKEN_ID),
			sp_runtime::MultiAddress::Id(ALICE),
			10,
		);
		assert_ok!(result);

		let result = StatemintAssets::mint(
			statemint_runtime::RuntimeOrigin::signed(ALICE),
			codec::Compact(TEST_TOKEN_ID),
			sp_runtime::MultiAddress::Id(ALICE),
			INITIAL_BALANCE,
		);
		assert_ok!(result);

		let alice_balance = StatemintAssets::balance(TEST_TOKEN_ID, &ALICE);

		assert_eq!(alice_balance, INITIAL_BALANCE);
	});

	// transfer both tokens to alice's account in acurast
	StatemintParachain::execute_with(|| {
		let xcm = StatemintXcmPallet::limited_reserve_transfer_assets(
			statemint_runtime::RuntimeOrigin::signed(ALICE),
			Box::new(
				MultiLocation { parents: 1, interior: X1(Parachain(ACURAST_CHAIN_ID)) }.into(),
			),
			Box::new(
				X1(Junction::AccountId32 { network: NetworkId::Any, id: ALICE.into() })
					.into()
					.into(),
			),
			Box::new(
				vec![
					MultiAsset {
						id: Concrete(
							X2(
								PalletInstance(STATEMINT_ASSETS_PALLET_INDEX),
								GeneralIndex(TEST_TOKEN_ID as u128),
							)
							.into(),
						),
						fun: Fungible(INITIAL_BALANCE / 2),
					},
					MultiAsset {
						id: Concrete(
							X2(
								PalletInstance(STATEMINT_ASSETS_PALLET_INDEX),
								GeneralIndex(STATEMINT_NATIVE_ID as u128),
							)
							.into(),
						),
						fun: Fungible(INITIAL_BALANCE / 2),
					},
				]
				.into(),
			),
			1,
			WeightLimit::Unlimited,
		);
		assert_ok!(xcm);
	});

	// check events in debug
	StatemintParachain::execute_with(|| {
		let _events: Vec<String> = statemint_runtime::System::events()
			.iter()
			.map(|e| format!("{:?}", e.event))
			.collect();
		let _x = 1; // put breakpoint here
	});

	// check that funds were minted correctly
	AcurastParachain::execute_with(|| {
		let _events: Vec<String> = acurast_runtime::System::events()
			.iter()
			.map(|e| format!("{:?}", e.event))
			.collect();
		let alice_balance_test_token = AcurastAssetsInternal::balance(TEST_TOKEN_ID, &ALICE);
		let alice_balance_false = AcurastAssetsInternal::balance(STATEMINT_NATIVE_ID, &ALICE);
		let _alice_balance_native = acurast_runtime::Balances::free_balance(&ALICE);
		assert_eq!(alice_balance_false, 0);
		// assert_eq!(alice_balance_native, 1453652000000);
		assert_eq!(alice_balance_test_token, INITIAL_BALANCE / 2);
	})
}

#[test]
fn pallet_assets_is_callable_in_runtime() {
	Network::reset();

	AcurastParachain::execute_with(|| {
		let pallet_account: <AcurastRuntime as frame_system::Config>::AccountId =
			<AcurastRuntime as pallet_acurast::Config>::PalletId::get().into_account_truncating();

		let raw_origin = RawOrigin::<<AcurastRuntime as frame_system::Config>::AccountId>::Signed(
			pallet_account.clone(),
		);
		let pallet_origin: <AcurastRuntime as frame_system::Config>::RuntimeOrigin =
			raw_origin.into();

		let _result = pallet_assets::Pallet::<AcurastRuntime>::create(
			pallet_origin,
			codec::Compact(STATEMINT_NATIVE_ID),
			<AcurastRuntime as frame_system::Config>::Lookup::unlookup(ALICE.clone()),
			1,
		);
		let _x = 10;
	});

	AcurastParachain::execute_with(|| {
		let _events: Vec<String> = acurast_runtime::System::events()
			.iter()
			.map(|e| format!("{:?}", e.event))
			.collect();
		let _x = 1; // put breakpoint here
	});
}

#[test]
fn fund_register_job() {
	use acurast_runtime::Runtime as AcurastRuntime;

	let pallet_account: <AcurastRuntime as frame_system::Config>::AccountId =
		<AcurastRuntime as pallet_acurast::Config>::PalletId::get().into_account_truncating();

	// fund alice's account with job payment tokens
	send_native_and_token();

	let reward_per_execution = 20_000;
	let registration = JobRegistration {
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
		extra: RegistrationExtra {
			destination: MultiLocation { parents: 1, interior: X1(Parachain(PROXY_CHAIN_ID)) },
			parameters: None,
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
			assert_eq!(balance_test_token, INITIAL_BALANCE / 2);

			assert_ok!(acurast_runtime::pallet_acurast::Pallet::<AcurastRuntime>::register(
				acurast_runtime::RuntimeOrigin::signed(ALICE), // ALICE's account should now be funded
				registration,
			));

			let balance_test_token = AcurastAssetsInternal::balance(TEST_TOKEN_ID, &ALICE);
			assert_eq!(balance_test_token, INITIAL_BALANCE / 2 - 2 * reward_per_execution); // reward worth 2 executions
		}
		// check job event and balances
		{
			let _events: Vec<String> = acurast_runtime::System::events()
				.iter()
				.map(|e| format!("{:?}", e.event))
				.collect();
			let _bob_balance_test_token = AcurastAssetsInternal::balance(TEST_TOKEN_ID, &BOB);
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

	let ad = advertisement(1000, 1, 100_000, 50_000, 8);
	let reward_per_execution = 10_000_000;
	let job_id = (FERDIE, script());
	let registration = JobRegistration {
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
		extra: RegistrationExtra {
			destination: MultiLocation { parents: 1, interior: X1(Parachain(PROXY_CHAIN_ID)) },
			parameters: None,
			requirements: JobRequirements {
				slots: 1,
				reward: test_asset(reward_per_execution),
				min_reputation: Some(500_000),
				instant_match: Some(vec![PlannedExecution { source: BOB, start_delay: 0 }]),
			},
			expected_fulfillment_fee: 10000,
		},
	};
	let price_per_execution = 5000 * 1000 + 20_000 + 5 * 8;

	// advertise resources
	AcurastParachain::execute_with(|| {
		// advertise
		assert_ok!(
			acurast_runtime::pallet_acurast_marketplace::Pallet::<AcurastRuntime>::advertise(
				acurast_runtime::RuntimeOrigin::signed(BOB),
				ad.clone(),
			)
		);

		// pretend current time
		later(now);

		// register job
		{
			let balance_ferdie_test_token = AcurastAssetsInternal::balance(TEST_TOKEN_ID, &FERDIE);
			assert_eq!(balance_ferdie_test_token, TEST_TOKEN_INITIAL_BALANCE);

			assert_ok!(acurast_runtime::pallet_acurast::Pallet::<AcurastRuntime>::register(
				acurast_runtime::RuntimeOrigin::signed(FERDIE), // FERDIE is a pre-funded via Genesis
				registration.clone(),
			));

			let balance_test_token = AcurastAssetsInternal::balance(TEST_TOKEN_ID, &FERDIE);
			// check we now have lower balance corresponding reward worth 2 executions
			assert_eq!(balance_test_token, TEST_TOKEN_INITIAL_BALANCE - 2 * reward_per_execution);
		}

		// check job event and balances
		{
			let _events: Vec<String> = acurast_runtime::System::events()
				.iter()
				.map(|e| format!("{:?}", e.event))
				.collect();
			let _bob_balance_test_token = AcurastAssetsInternal::balance(TEST_TOKEN_ID, &BOB);
			let _ferdie_balance_test_token = AcurastAssetsInternal::balance(TEST_TOKEN_ID, &FERDIE);
			let _ferdie_balance_false =
				AcurastAssetsInternal::balance(STATEMINT_NATIVE_ID, &FERDIE);
			let _ferdie_balance_native = acurast_runtime::Balances::free_balance(&FERDIE);
			let _pallet_balance_test_token =
				AcurastAssetsInternal::balance(TEST_TOKEN_ID, pallet_account.clone());
		}

		// acknowledge
		assert_ok!(acurast_runtime::pallet_acurast_marketplace::Pallet::<AcurastRuntime>::acknowledge_match(
            acurast_runtime::RuntimeOrigin::signed(BOB).into(),
            job_id.clone(),
        ));

		// reports
		{
			let balance_bob_test_token_0 = AcurastAssetsInternal::balance(TEST_TOKEN_ID, &BOB);
			assert_eq!(balance_bob_test_token_0, 0);

			let mut iter = registration.schedule.iter(0).unwrap();

			later(iter.next().unwrap() + 1000);
			assert_ok!(
				acurast_runtime::pallet_acurast_marketplace::Pallet::<AcurastRuntime>::report(
					acurast_runtime::RuntimeOrigin::signed(BOB).into(),
					job_id.clone(),
					false,
					ExecutionResult::Success(operation_hash())
				)
			);

			let balance_bob_test_token_1 = AcurastAssetsInternal::balance(TEST_TOKEN_ID, &BOB);
			// check we now have higher balance corresponding reward gained
			assert_eq!(
				balance_bob_test_token_1,
				price_per_execution -
					FeeManagement::get_fee_percentage().mul_floor(price_per_execution) -
					28
			);

			later(iter.next().unwrap() + 1000);
			assert_ok!(
				acurast_runtime::pallet_acurast_marketplace::Pallet::<AcurastRuntime>::report(
					acurast_runtime::RuntimeOrigin::signed(BOB).into(),
					job_id.clone(),
					true,
					ExecutionResult::Success(operation_hash())
				)
			);

			let balance_test_token_2 = AcurastAssetsInternal::balance(TEST_TOKEN_ID, &BOB);
			// check we now have higher balance corresponding reward gained
			assert_eq!(
				balance_test_token_2,
				balance_bob_test_token_1 + price_per_execution -
					FeeManagement::get_fee_percentage().mul_floor(price_per_execution) -
					28
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
