use parity_scale_codec::Encode;
use frame_support::{assert_ok, traits::Currency};
use xcm_emulator::TestExt;

#[cfg(test)]
use crate::tests::*;

#[test]
fn dmp() {
	Network::reset();

	let remark = acurast_runtime::RuntimeCall::System(frame_system::Call::<
		acurast_runtime::Runtime,
	>::remark_with_event {
		remark: "Hello from Atera".as_bytes().to_vec(),
	});
	PolkadotRelay::execute_with(|| {
		assert_ok!(polkadot_runtime::XcmPallet::force_default_xcm_version(
			polkadot_runtime::RuntimeOrigin::root(),
			Some(0)
		));
		assert_ok!(polkadot_runtime::XcmPallet::send_xcm(
			Here,
			Parachain(ACURAST_CHAIN_ID),
			Xcm(vec![Transact {
				origin_type: OriginKind::SovereignAccount,
				require_weight_at_most: INITIAL_BALANCE as u64,
				call: remark.encode().into(),
			}]),
		));
	});

	AcurastParachain::execute_with(|| {
		use acurast_runtime::{RuntimeEvent, System};
		System::events().iter().for_each(|r| println!(">>> {:?}", r.event));

		assert!(System::events().iter().any(|r| matches!(
			r.event,
			RuntimeEvent::System(frame_system::Event::Remarked { sender: _, hash: _ })
		)));
	});
}

#[test]
fn ump() {
	Network::reset();

	PolkadotRelay::execute_with(|| {
		let _ = polkadot_runtime::Balances::deposit_creating(
			&ParaId::from(ACURAST_CHAIN_ID).into_account_truncating(),
			1_000_000_000_000,
		);
	});

	let remark = polkadot_runtime::RuntimeCall::System(frame_system::Call::<
		polkadot_runtime::Runtime,
	>::remark_with_event {
		remark: "Hello from Acurast!".as_bytes().to_vec(),
	});

	let send_amount = 1_000_000_000_000;
	AcurastParachain::execute_with(|| {
		assert_ok!(acurast_runtime::PolkadotXcm::send_xcm(
			Here,
			Parent,
			Xcm(vec![
				WithdrawAsset((Here, send_amount).into()),
				buy_execution((Here, send_amount)),
				Transact {
					origin_type: OriginKind::SovereignAccount,
					require_weight_at_most: INITIAL_BALANCE as u64,
					call: remark.encode().into(),
				}
			]),
		));
	});

	PolkadotRelay::execute_with(|| {
		use polkadot_runtime::{RuntimeEvent, System};
		let _events: Vec<String> =
			System::events().iter().map(|e| format!("{:?}", e.event)).collect();
		assert!(System::events().iter().any(|r| matches!(
			r.event,
			RuntimeEvent::System(frame_system::Event::Remarked { sender: _, hash: _ })
		)));
	});
}

#[test]
fn xcmp() {
	Network::reset();

	let remark = proxy_parachain_runtime::RuntimeCall::System(frame_system::Call::<
		proxy_parachain_runtime::Runtime,
	>::remark_with_event {
		remark: "Hello from acurast!".as_bytes().to_vec(),
	});
	AcurastParachain::execute_with(|| {
		assert_ok!(acurast_runtime::PolkadotXcm::send_xcm(
			Here,
			MultiLocation::new(1, X1(Parachain(PROXY_CHAIN_ID))),
			Xcm(vec![Transact {
				origin_type: OriginKind::SovereignAccount,
				require_weight_at_most: 100_000_000,
				call: remark.encode().into(),
			}]),
		));
	});

	ProxyParachain::execute_with(|| {
		use proxy_parachain_runtime::{RuntimeEvent, System};
		let _events: Vec<String> =
			System::events().iter().map(|e| format!("{:?}", e.event)).collect();
		System::events().iter().for_each(|r| println!(">>> {:?}", r.event));

		assert!(System::events().iter().any(|r| matches!(
			r.event,
			RuntimeEvent::System(frame_system::Event::Remarked { sender: _, hash: _ })
		)));
	});
}

#[test]
fn reserve_transfer() {
	Network::reset();

	let withdraw_amount = INITIAL_BALANCE / 4;

	PolkadotRelay::execute_with(|| {
		assert_ok!(PolkadotXcmPallet::reserve_transfer_assets(
			polkadot_runtime::RuntimeOrigin::signed(ALICE),
			Box::new(X1(Parachain(ACURAST_CHAIN_ID)).into().into()),
			Box::new(X1(Junction::AccountId32 { network: Any, id: ALICE.into() }).into().into()),
			Box::new(
				MultiAsset {
					id: Concrete(MultiLocation { parents: 0, interior: Here }),
					fun: Fungible(withdraw_amount)
				}
				.into()
			),
			0,
		));
		assert_eq!(
			polkadot_runtime::Balances::free_balance(&child_para_account_id(ACURAST_CHAIN_ID)),
			INITIAL_BALANCE + withdraw_amount
		);
	});

	PolkadotRelay::execute_with(|| {
		let _events: Vec<String> = polkadot_runtime::System::events()
			.iter()
			.map(|e| format!("{:?}", e.event))
			.collect();
		let _x = 1;
	});

	AcurastParachain::execute_with(|| {
		let _events: Vec<String> = acurast_runtime::System::events()
			.iter()
			.map(|e| format!("{:?}", e.event))
			.collect();

		// TODO: are fees deterministic? if so then remove the margins and hardcode the results
		let fee_margin = ((INITIAL_BALANCE as f32 + withdraw_amount as f32) * 0.15) as u128;
		let full_deposit = INITIAL_BALANCE + withdraw_amount;
		let pallet_balance =
			pallet_balances::Pallet::<acurast_runtime::Runtime>::free_balance(&ALICE);
		assert!(
			pallet_balance < (&full_deposit + &fee_margin) &&
				pallet_balance > (full_deposit - fee_margin)
		);
	});
}

/// Scenario:
/// A parachain transfers funds on the relay chain to another parachain account.
///
/// Asserts that the parachain accounts are updated as expected.
#[test]
fn withdraw_and_deposit() {
	Network::reset();

	let send_amount = INITIAL_BALANCE / 2;

	AcurastParachain::execute_with(|| {
		let message = Xcm(vec![
			WithdrawAsset((Here, send_amount).into()),
			buy_execution((Here, send_amount)),
			DepositAsset {
				assets: All.into(),
				max_assets: 1,
				beneficiary: Parachain(PROXY_CHAIN_ID).into(),
			},
		]);
		// Send withdraw and deposit
		assert_ok!(AcurastXcmPallet::send_xcm(Here, Parent, message.clone()));
	});

	PolkadotRelay::execute_with(|| {
		let acurast_balance =
			polkadot_runtime::Balances::free_balance(child_para_account_id(ACURAST_CHAIN_ID));
		let proxy_balance =
			polkadot_runtime::Balances::free_balance(child_para_account_id(PROXY_CHAIN_ID));
		assert_eq!(acurast_balance, INITIAL_BALANCE - send_amount);
		assert_eq!(proxy_balance, 1499693514774); // initial + amount - fees
	});
}

/// Scenario:
/// A parachain wants to be notified that a transfer worked correctly.
/// It sends a `QueryHolding` after the deposit to get notified on success.
///
/// Asserts that the balances are updated correctly and the expected XCM is sent.
#[test]
fn query_holding() {
	Network::reset();

	let send_amount = INITIAL_BALANCE / 2;
	let query_id_set = 1234;

	// Send a message which fully succeeds on the relay chain
	AcurastParachain::execute_with(|| {
		let message = Xcm(vec![
			WithdrawAsset((Here, send_amount).into()),
			buy_execution((Here, send_amount)),
			DepositAsset {
				assets: All.into(),
				max_assets: 1,
				beneficiary: Parachain(PROXY_CHAIN_ID).into(),
			},
			QueryHolding {
				query_id: query_id_set,
				dest: Parachain(ACURAST_CHAIN_ID).into(),
				assets: All.into(),
				max_response_weight: 1_000_000_000,
			},
		]);
		// Send withdraw and deposit with query holding
		assert_ok!(AcurastXcmPallet::send_xcm(Here, Parent, message.clone()));
	});

	// Check that transfer was executed
	PolkadotRelay::execute_with(|| {
		let acurast_balance =
			polkadot_runtime::Balances::free_balance(child_para_account_id(ACURAST_CHAIN_ID));
		let proxy_balance =
			polkadot_runtime::Balances::free_balance(child_para_account_id(PROXY_CHAIN_ID));
		// Withdraw executed
		assert_eq!(acurast_balance, INITIAL_BALANCE - send_amount);

		// Deposit executed
		assert_eq!(proxy_balance, 1499591353032);
	});

	// Check that QueryResponse message was received
	AcurastParachain::execute_with(|| {
		use acurast_runtime::{RuntimeEvent, System};
		let events = System::events();
		let _events: Vec<String> = events.iter().map(|e| format!("{:?}", e.event)).collect();

		match events[0].event {
			RuntimeEvent::PolkadotXcm(pallet_xcm::Event::UnexpectedResponse(_, 1234)) =>
				assert!(true),
			_ => panic!("Correct event not found"),
		}
	});
}
