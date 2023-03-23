use crate::tests::*;

#[test]
fn can_recreate() {
	Network::reset();
	AcurastParachain::execute_with(|| {
		let result = AcurastAssets::create(
			acurast_runtime::RuntimeOrigin::root(),
			codec::Compact(STATEMINT_NATIVE_ID),
			Concrete(MultiLocation {
				parents: 1,
				interior: X3(
					Parachain(STATEMINT_CHAIN_ID),
					PalletInstance(STATEMINT_ASSETS_PALLET_INDEX),
					GeneralIndex(STATEMINT_NATIVE_ID as u128),
				),
			}),
			sp_runtime::MultiAddress::Id(ALICE),
			10,
		);
		assert_ok!(result);
	})
}

#[test]
#[should_panic]
fn cannot_mint() {
	Network::reset();
	AcurastParachain::execute_with(|| {
		let result = StatemintAssets::mint(
			statemint_runtime::RuntimeOrigin::signed(ALICE),
			codec::Compact(STATEMINT_NATIVE_ID),
			sp_runtime::MultiAddress::Id(ALICE),
			1500,
		);
		assert_ok!(result);
	})
}

#[test]
fn reserve_transfer_mint_native() {
	Network::reset();
	// crate same token (id) in statemint so we use the default statemint fungibles adapter defined
	// in the xcm_config of acurast
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

	// do a reserve transfer. This should not mint anything since that id is reserved for local
	// asset translation with the Balances pallet and not the Assets pallet
	// reserve backed transfer of token 1 from statemint to acurast
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
				vec![MultiAsset {
					id: Concrete(
						X2(
							PalletInstance(STATEMINT_ASSETS_PALLET_INDEX),
							GeneralIndex(STATEMINT_NATIVE_ID as u128),
						)
						.into(),
					),
					fun: Fungible(INITIAL_BALANCE / 2),
				}]
				.into(),
			),
			0,
			WeightLimit::Unlimited,
		);
		assert_ok!(xcm);
	});

	StatemintParachain::execute_with(|| {
		let _events: Vec<String> = statemint_runtime::System::events()
			.iter()
			.map(|e| format!("{:?}", e.event))
			.collect();
		println!("stop");
	});

	AcurastParachain::execute_with(|| {
		let _events: Vec<String> = acurast_runtime::System::events()
			.iter()
			.map(|e| format!("{:?}", e.event))
			.collect();
		let alice_balance_fung = AcurastAssetsInternal::balance(STATEMINT_NATIVE_ID, &ALICE);
		let alice_balance_native = acurast_runtime::Balances::free_balance(&ALICE);
		assert_eq!(alice_balance_fung, 0);
		assert_eq!(alice_balance_native, 1495958800000);
	})
}

#[test]
// TODO: transfers from acurast to a user in statemint of native assets. It should burn the native
fn transfer_statemint_burn_native() {
	assert!(true);
}
