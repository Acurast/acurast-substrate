extern crate core;

// parent re-exports
use emulations::emulators::xcm_emulator;
use emulations::runtimes::{acurast_runtime, polkadot_runtime, proxy_runtime, statemint_runtime};

// needed libs
use crate::acurast_runtime::pallet_acurast;
use cumulus_primitives_core::ParaId;
use frame_support::traits::GenesisBuild;
use polkadot_parachain::primitives::Sibling;
use sp_runtime::traits::AccountIdConversion;
use sp_runtime::traits::{Dispatchable, StaticLookup};
use sp_runtime::AccountId32;
use xcm::prelude::*;
use xcm_emulator::TestExt;
use xcm_emulator::{decl_test_network, decl_test_parachain, decl_test_relay_chain};

decl_test_relay_chain! {
    pub struct PolkadotRelay {
        Runtime = polkadot_runtime::Runtime,
        XcmConfig = polkadot_runtime::xcm_config::XcmConfig,
        new_ext = polkadot_ext(),
    }
}

decl_test_parachain! {
    pub struct AcurastParachain {
        Runtime = acurast_runtime::Runtime,
        Origin = acurast_runtime::Origin,
        XcmpMessageHandler = acurast_runtime::XcmpQueue,
        DmpMessageHandler = acurast_runtime::DmpQueue,
        new_ext = acurast_ext(2000),
    }
}

decl_test_parachain! {
    pub struct ProxyParachain {
        Runtime = proxy_runtime::Runtime,
        Origin = proxy_runtime::Origin,
        XcmpMessageHandler = proxy_runtime::XcmpQueue,
        DmpMessageHandler = proxy_runtime::DmpQueue,
        new_ext = proxy_ext(2001),
    }
}

decl_test_parachain! {
    pub struct StatemintParachain {
        Runtime = statemint_runtime::Runtime,
        Origin = statemint_runtime::Origin,
        XcmpMessageHandler = statemint_runtime::XcmpQueue,
        DmpMessageHandler = statemint_runtime::DmpQueue,
        new_ext = statemint_ext(1000),
    }
}

decl_test_network! {
    pub struct Network {
        relay_chain = PolkadotRelay,
        parachains = vec![
            (2000, AcurastParachain),
            (2001, ProxyParachain),
            (1000, StatemintParachain),
        ],
    }
}

pub const ALICE: AccountId32 = AccountId32::new([4u8; 32]);
pub const BOB: AccountId32 = AccountId32::new([8u8; 32]);
pub const BURN_ACCOUNT: AccountId32 = AccountId32::new([0u8; 32]);

pub const INITIAL_BALANCE: u128 = 1_000_000_000_000;

pub fn acurast_ext(para_id: u32) -> sp_io::TestExternalities {
    use acurast_runtime::{Runtime, System};

    let mut t = frame_system::GenesisConfig::default()
        .build_storage::<Runtime>()
        .unwrap();

    let parachain_info_config = parachain_info::GenesisConfig {
        parachain_id: para_id.into(),
    };

    <parachain_info::GenesisConfig as GenesisBuild<Runtime, _>>::assimilate_storage(
        &parachain_info_config,
        &mut t,
    )
    .unwrap();

    let pallet_assets_account: <Runtime as frame_system::Config>::AccountId =
        <Runtime as pallet_acurast::Config>::PalletId::get().into_account_truncating();

    pallet_balances::GenesisConfig::<Runtime> {
        balances: vec![
            (ALICE, INITIAL_BALANCE),
            (pallet_assets_account, INITIAL_BALANCE),
            (BOB, INITIAL_BALANCE),
        ],
    }
    .assimilate_storage(&mut t)
    .unwrap();

    let pallet_xcm_config = pallet_xcm::GenesisConfig::default();
    <pallet_xcm::GenesisConfig as GenesisBuild<Runtime, _>>::assimilate_storage(
        &pallet_xcm_config,
        &mut t,
    )
    .unwrap();

    pallet_assets::GenesisConfig::<Runtime> {
        assets: vec![(
            NATIVE_ASSET_ID,
            BURN_ACCOUNT,
            NATIVE_IS_SUFFICIENT,
            NATIVE_MIN_BALANCE,
        )],
        metadata: vec![(
            NATIVE_ASSET_ID,
            NATIVE_TOKEN_NAME.into(),
            NATIVE_TOKEN_SYMBOL.into(),
            NATIVE_TOKEN_DECIMALS,
        )],
        accounts: vec![(NATIVE_ASSET_ID, BURN_ACCOUNT, NATIVE_INITIAL_BALANCE)],
    }
    .assimilate_storage(&mut t)
    .unwrap();

    const NATIVE_ASSET_ID: u32 = 42;
    const NATIVE_IS_SUFFICIENT: bool = true;
    const NATIVE_MIN_BALANCE: u128 = 1;
    const NATIVE_INITIAL_BALANCE: u128 = INITIAL_BALANCE * 100;
    const NATIVE_TOKEN_NAME: &str = "reserved_native_asset";
    const NATIVE_TOKEN_SYMBOL: &str = "RNA";
    const NATIVE_TOKEN_DECIMALS: u8 = 12;

    let mut ext = sp_io::TestExternalities::new(t);
    ext.execute_with(|| System::set_block_number(1));
    ext
}

pub fn proxy_ext(para_id: u32) -> sp_io::TestExternalities {
    use proxy_runtime::{Runtime, System};

    let mut t = frame_system::GenesisConfig::default()
        .build_storage::<Runtime>()
        .unwrap();

    let parachain_info_config = parachain_info::GenesisConfig {
        parachain_id: para_id.into(),
    };

    <parachain_info::GenesisConfig as GenesisBuild<Runtime, _>>::assimilate_storage(
        &parachain_info_config,
        &mut t,
    )
    .unwrap();

    pallet_balances::GenesisConfig::<Runtime> {
        balances: vec![(ALICE, INITIAL_BALANCE)],
    }
    .assimilate_storage(&mut t)
    .unwrap();

    let pallet_xcm_config = pallet_xcm::GenesisConfig::default();
    <pallet_xcm::GenesisConfig as GenesisBuild<Runtime, _>>::assimilate_storage(
        &pallet_xcm_config,
        &mut t,
    )
    .unwrap();

    let mut ext = sp_io::TestExternalities::new(t);
    ext.execute_with(|| System::set_block_number(1));
    ext
}

pub fn statemint_ext(para_id: u32) -> sp_io::TestExternalities {
    use statemint_runtime::{Runtime, System};

    let mut t = frame_system::GenesisConfig::default()
        .build_storage::<Runtime>()
        .unwrap();

    let parachain_info_config = parachain_info::GenesisConfig {
        parachain_id: para_id.into(),
    };

    <parachain_info::GenesisConfig as GenesisBuild<Runtime, _>>::assimilate_storage(
        &parachain_info_config,
        &mut t,
    )
    .unwrap();

    pallet_balances::GenesisConfig::<Runtime> {
        balances: vec![
            (BURN_ACCOUNT, INITIAL_BALANCE),
            (ALICE, INITIAL_BALANCE),
            (sibling_para_account_id(2000), INITIAL_BALANCE),
            (sibling_para_account_id(2001), INITIAL_BALANCE),
        ],
    }
    .assimilate_storage(&mut t)
    .unwrap();

    let pallet_xcm_config = pallet_xcm::GenesisConfig::default();
    <pallet_xcm::GenesisConfig as GenesisBuild<Runtime, _>>::assimilate_storage(
        &pallet_xcm_config,
        &mut t,
    )
    .unwrap();

    let mut ext = sp_io::TestExternalities::new(t);
    ext.execute_with(|| System::set_block_number(1));
    ext
}

fn default_parachains_host_configuration(
) -> polkadot_runtime_parachains::configuration::HostConfiguration<
    polkadot_primitives::v2::BlockNumber,
> {
    use polkadot_primitives::v2::{MAX_CODE_SIZE, MAX_POV_SIZE};

    polkadot_runtime_parachains::configuration::HostConfiguration {
        minimum_validation_upgrade_delay: 5,
        validation_upgrade_cooldown: 10u32,
        validation_upgrade_delay: 10,
        code_retention_period: 1200,
        max_code_size: MAX_CODE_SIZE,
        max_pov_size: MAX_POV_SIZE,
        max_head_data_size: 32 * 1024,
        group_rotation_frequency: 20,
        chain_availability_period: 4,
        thread_availability_period: 4,
        max_upward_queue_count: 8,
        max_upward_queue_size: 1024 * 1024,
        max_downward_message_size: 1024,
        ump_service_total_weight: 4 * 1_000_000_000,
        max_upward_message_size: 50 * 1024,
        max_upward_message_num_per_candidate: 5,
        hrmp_sender_deposit: 0,
        hrmp_recipient_deposit: 0,
        hrmp_channel_max_capacity: 8,
        hrmp_channel_max_total_size: 8 * 1024,
        hrmp_max_parachain_inbound_channels: 4,
        hrmp_max_parathread_inbound_channels: 4,
        hrmp_channel_max_message_size: 1024 * 1024,
        hrmp_max_parachain_outbound_channels: 4,
        hrmp_max_parathread_outbound_channels: 4,
        hrmp_max_message_num_per_candidate: 5,
        dispute_period: 6,
        no_show_slots: 2,
        n_delay_tranches: 25,
        needed_approvals: 2,
        relay_vrf_modulo_samples: 2,
        zeroth_delay_tranche_width: 0,
        ..Default::default()
    }
}

pub fn polkadot_ext() -> sp_io::TestExternalities {
    use polkadot_runtime::{Runtime, System};

    let mut t = frame_system::GenesisConfig::default()
        .build_storage::<Runtime>()
        .unwrap();

    pallet_balances::GenesisConfig::<Runtime> {
        // IMPORTANT: to do reserve transfers, the sovereign account of the parachains needs to hold
        // a minimum amount of DOT called "existential deposit". Otherwise transfers will fail at
        // the point of internal_transfer_asset method of the AssetTransactor in xcm_executor::Config
        balances: vec![
            (ALICE, INITIAL_BALANCE),
            (child_para_account_id(2000), INITIAL_BALANCE),
            (child_para_account_id(2001), INITIAL_BALANCE),
        ],
    }
    .assimilate_storage(&mut t)
    .unwrap();

    polkadot_runtime_parachains::configuration::GenesisConfig::<Runtime> {
        config: default_parachains_host_configuration(),
    }
    .assimilate_storage(&mut t)
    .unwrap();

    let pallet_xcm_config = pallet_xcm::GenesisConfig::default();
    <pallet_xcm::GenesisConfig as GenesisBuild<Runtime, _>>::assimilate_storage(
        &pallet_xcm_config,
        &mut t,
    )
    .unwrap();

    let mut ext = sp_io::TestExternalities::new(t);
    ext.execute_with(|| System::set_block_number(1));
    ext
}

// Helper function for forming buy execution message
fn buy_execution<C>(fees: impl Into<MultiAsset>) -> Instruction<C> {
    BuyExecution {
        fees: fees.into(),
        weight_limit: Unlimited,
    }
}

pub fn sibling_para_account_id(id: u32) -> polkadot_core_primitives::AccountId {
    // ParaId::from(id).into_account_truncating()
    Sibling::from(id).into_account_truncating()
}

pub fn child_para_account_id(id: u32) -> polkadot_core_primitives::AccountId {
    // ParaId::from(id).into_account_truncating()
    ParaId::from(id).into_account_truncating()
}

type AcurastXcmPallet = pallet_xcm::Pallet<acurast_runtime::Runtime>;
type PolkadotXcmPallet = pallet_xcm::Pallet<polkadot_runtime::Runtime>;
type StatemintXcmPallet = pallet_xcm::Pallet<statemint_runtime::Runtime>;

type StatemintMinter = pallet_assets::Pallet<statemint_runtime::Runtime>;
type AcurastMinter = pallet_assets::Pallet<statemint_runtime::Runtime>;

#[cfg(test)]
mod network_tests {
    use super::*;
    use codec::Encode;
    use frame_support::{assert_ok, traits::Currency};
    use sp_runtime::traits::AccountIdConversion;

    #[test]
    fn dmp() {
        Network::reset();

        let remark = acurast_runtime::Call::System(
            frame_system::Call::<acurast_runtime::Runtime>::remark_with_event {
                remark: "Hello from Atera".as_bytes().to_vec(),
            },
        );
        PolkadotRelay::execute_with(|| {
            assert_ok!(polkadot_runtime::XcmPallet::force_default_xcm_version(
                polkadot_runtime::Origin::root(),
                Some(0)
            ));
            assert_ok!(polkadot_runtime::XcmPallet::send_xcm(
                Here,
                Parachain(2000),
                Xcm(vec![Transact {
                    origin_type: OriginKind::SovereignAccount,
                    require_weight_at_most: INITIAL_BALANCE as u64,
                    call: remark.encode().into(),
                }]),
            ));
        });

        AcurastParachain::execute_with(|| {
            use acurast_runtime::{Event, System};
            System::events()
                .iter()
                .for_each(|r| println!(">>> {:?}", r.event));

            assert!(System::events().iter().any(|r| matches!(
                r.event,
                Event::System(frame_system::Event::Remarked { sender: _, hash: _ })
            )));
        });
    }

    #[test]
    fn ump() {
        Network::reset();

        PolkadotRelay::execute_with(|| {
            let _ = polkadot_runtime::Balances::deposit_creating(
                &ParaId::from(2000).into_account_truncating(),
                1_000_000_000_000,
            );
        });

        let remark = polkadot_runtime::Call::System(frame_system::Call::<
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
            use polkadot_runtime::{Event, System};
            let _event_list = System::events();
            assert!(System::events().iter().any(|r| matches!(
                r.event,
                Event::System(frame_system::Event::Remarked { sender: _, hash: _ })
            )));
        });
    }

    #[test]
    fn xcmp() {
        Network::reset();

        let remark = proxy_runtime::Call::System(
            frame_system::Call::<proxy_runtime::Runtime>::remark_with_event {
                remark: "Hello from acurast!".as_bytes().to_vec(),
            },
        );
        AcurastParachain::execute_with(|| {
            assert_ok!(acurast_runtime::PolkadotXcm::send_xcm(
                Here,
                MultiLocation::new(1, X1(Parachain(2001))),
                Xcm(vec![Transact {
                    origin_type: OriginKind::SovereignAccount,
                    require_weight_at_most: 10_000_000,
                    call: remark.encode().into(),
                }]),
            ));
        });

        ProxyParachain::execute_with(|| {
            use proxy_runtime::{Event, System};
            System::events()
                .iter()
                .for_each(|r| println!(">>> {:?}", r.event));

            assert!(System::events().iter().any(|r| matches!(
                r.event,
                Event::System(frame_system::Event::Remarked { sender: _, hash: _ })
            )));
        });
    }

    #[test]
    fn reserve_transfer() {
        Network::reset();

        let withdraw_amount = INITIAL_BALANCE / 4;

        PolkadotRelay::execute_with(|| {
            assert_ok!(PolkadotXcmPallet::reserve_transfer_assets(
                polkadot_runtime::Origin::signed(ALICE),
                Box::new(X1(Parachain(2000)).into().into()),
                Box::new(
                    X1(Junction::AccountId32 {
                        network: Any,
                        id: ALICE.into()
                    })
                    .into()
                    .into()
                ),
                Box::new(
                    MultiAsset {
                        id: Concrete(MultiLocation {
                            parents: 0,
                            interior: Here
                        }),
                        fun: Fungible(withdraw_amount)
                    }
                    .into()
                ),
                0,
            ));
            assert_eq!(
                polkadot_runtime::Balances::free_balance(&child_para_account_id(2000)),
                INITIAL_BALANCE + withdraw_amount
            );
        });

        PolkadotRelay::execute_with(|| {
            let _events = polkadot_runtime::System::events();
            let _x = 1;
        });

        AcurastParachain::execute_with(|| {
            let _events = acurast_runtime::System::events();

            // TODO: are fees deterministic? if so then remove the margins and hardcode the results
            let fee_margin = ((INITIAL_BALANCE as f32 + withdraw_amount as f32) * 0.15) as u128;
            let full_deposit = INITIAL_BALANCE + withdraw_amount;
            let pallet_balance =
                pallet_balances::Pallet::<acurast_runtime::Runtime>::free_balance(&ALICE);
            assert!(
                pallet_balance < (&full_deposit + &fee_margin)
                    && pallet_balance > (full_deposit - fee_margin)
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
                    beneficiary: Parachain(2001).into(),
                },
            ]);
            // Send withdraw and deposit
            assert_ok!(AcurastXcmPallet::send_xcm(Here, Parent, message.clone()));
        });

        PolkadotRelay::execute_with(|| {
            let acurast_balance =
                polkadot_runtime::Balances::free_balance(child_para_account_id(2000));
            let proxy_balance =
                polkadot_runtime::Balances::free_balance(child_para_account_id(2001));
            assert_eq!(acurast_balance, INITIAL_BALANCE - send_amount);
            assert_eq!(proxy_balance, 1499647936911); // initial + amount - fees
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
                    beneficiary: Parachain(2001).into(),
                },
                QueryHolding {
                    query_id: query_id_set,
                    dest: Parachain(2000).into(),
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
                polkadot_runtime::Balances::free_balance(child_para_account_id(2000));
            let proxy_balance =
                polkadot_runtime::Balances::free_balance(child_para_account_id(2001));
            // Withdraw executed
            assert_eq!(acurast_balance, INITIAL_BALANCE - send_amount);

            // Deposit executed
            assert_eq!(proxy_balance, 1499530582548);
        });

        // Check that QueryResponse message was received
        AcurastParachain::execute_with(|| {
            use acurast_runtime::{Event, System};
            let events = System::events();

            match events[0].event {
                Event::PolkadotXcm(pallet_xcm::Event::UnexpectedResponse(_, 1234)) => assert!(true),
                _ => panic!("Correct event not found"),
            }
        });
    }
}

#[cfg(test)]
mod statemint_backed_native_assets {
    use super::*;
    use frame_support::assert_ok;

    #[test]
    #[should_panic]
    fn cannot_create() {
        Network::reset();
        AcurastParachain::execute_with(|| {
            let result = AcurastMinter::create(
                statemint_runtime::Origin::signed(ALICE),
                42,
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
            let result = StatemintMinter::mint(
                statemint_runtime::Origin::signed(ALICE),
                42,
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
            let result = StatemintMinter::create(
                statemint_runtime::Origin::signed(ALICE),
                42,
                sp_runtime::MultiAddress::Id(ALICE),
                10,
            );
            assert_ok!(result);

            let result = StatemintMinter::mint(
                statemint_runtime::Origin::signed(ALICE),
                42,
                sp_runtime::MultiAddress::Id(ALICE),
                INITIAL_BALANCE,
            );
            assert_ok!(result);

            let alice_balance = StatemintMinter::balance(42, &ALICE);

            assert_eq!(alice_balance, INITIAL_BALANCE);
        });

        // do a reserve transfer. This should not mint anything since that id is reserved for local
        // asset translation with the Balances pallet and not the Assets pallet
        // reserve backed transfer of token 1 from statemint to acurast
        StatemintParachain::execute_with(|| {
            let xcm = StatemintXcmPallet::reserve_transfer_assets(
                statemint_runtime::Origin::signed(ALICE),
                Box::new(
                    MultiLocation {
                        parents: 1,
                        interior: X1(Parachain(2000)),
                    }
                    .into(),
                ),
                Box::new(
                    X1(Junction::AccountId32 {
                        network: NetworkId::Any,
                        id: ALICE.into(),
                    })
                    .into()
                    .into(),
                ),
                Box::new(
                    vec![MultiAsset {
                        id: Concrete(X2(PalletInstance(50), GeneralIndex(42)).into()),
                        fun: Fungible(INITIAL_BALANCE / 2),
                    }]
                    .into(),
                ),
                0,
            );
            assert_ok!(xcm);
        });

        StatemintParachain::execute_with(|| {
            let _events = statemint_runtime::System::events();
            println!("stop");
        });

        AcurastParachain::execute_with(|| {
            let _events = acurast_runtime::System::events();
            let alice_balance_fung = AcurastMinter::balance(42, &ALICE);
            let alice_balance_native = acurast_runtime::Balances::free_balance(&ALICE);
            assert_eq!(alice_balance_fung, 0);
            assert_eq!(alice_balance_native, 1453652000000);
        })
    }

    #[test]
    // TODO: transfers from acurast to a user in statemint of native assets. It should burn the native
    fn transfer_statemint_burn_native() {
        assert!(true);
    }
}

#[cfg(test)]
mod job_payments {
    use frame_support::assert_ok;
    use frame_support::dispatch::RawOrigin;

    use super::*;
    use crate::acurast_runtime::pallet_acurast;
    use crate::pallet_acurast::{
        Fulfillment, JobAssignmentUpdate, JobRegistration, ListUpdateOperation,
    };
    use acurast_runtime::Runtime as AcurastRuntime;

    const SCRIPT_BYTES: [u8; 53] = hex_literal::hex!("697066733A2F2F00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000");

    #[test]
    fn send_native_and_token() {
        Network::reset();
        // create acurast native token in statemint to pay for execution of xcm
        StatemintParachain::execute_with(|| {
            let result = StatemintMinter::create(
                statemint_runtime::Origin::signed(ALICE),
                42,
                sp_runtime::MultiAddress::Id(ALICE),
                10,
            );
            assert_ok!(result);

            let result = StatemintMinter::mint(
                statemint_runtime::Origin::signed(ALICE),
                42,
                sp_runtime::MultiAddress::Id(ALICE),
                INITIAL_BALANCE,
            );
            assert_ok!(result);

            let alice_balance = StatemintMinter::balance(42, &ALICE);

            assert_eq!(alice_balance, INITIAL_BALANCE);
        });

        // create another token in statemint to pay for job
        StatemintParachain::execute_with(|| {
            let result = StatemintMinter::create(
                statemint_runtime::Origin::signed(ALICE),
                69,
                sp_runtime::MultiAddress::Id(ALICE),
                10,
            );
            assert_ok!(result);

            let result = StatemintMinter::mint(
                statemint_runtime::Origin::signed(ALICE),
                69,
                sp_runtime::MultiAddress::Id(ALICE),
                INITIAL_BALANCE,
            );
            assert_ok!(result);

            let alice_balance = StatemintMinter::balance(69, &ALICE);

            assert_eq!(alice_balance, INITIAL_BALANCE);
        });

        // transfer both tokens to alice's account in acurast
        StatemintParachain::execute_with(|| {
            let xcm = StatemintXcmPallet::reserve_transfer_assets(
                statemint_runtime::Origin::signed(ALICE),
                Box::new(
                    MultiLocation {
                        parents: 1,
                        interior: X1(Parachain(2000)),
                    }
                    .into(),
                ),
                Box::new(
                    X1(Junction::AccountId32 {
                        network: NetworkId::Any,
                        id: ALICE.into(),
                    })
                    .into()
                    .into(),
                ),
                Box::new(
                    vec![
                        MultiAsset {
                            id: Concrete(X2(PalletInstance(50), GeneralIndex(42)).into()),
                            fun: Fungible(INITIAL_BALANCE / 2),
                        },
                        // MultiAsset {
                        //     id: Concrete(MultiLocation {
                        //         parents: 1,
                        //         interior: Here,
                        //     }),
                        //     fun: Fungible(INITIAL_BALANCE / 4),
                        // },
                        MultiAsset {
                            id: Concrete(X2(PalletInstance(50), GeneralIndex(69)).into()),
                            fun: Fungible(INITIAL_BALANCE / 2),
                        },
                    ]
                    .into(),
                ),
                0,
            );
            assert_ok!(xcm);
        });

        // check events in debug
        StatemintParachain::execute_with(|| {
            let _events = statemint_runtime::System::events();
            let _x = 1; // put breakpoint here
        });

        // check that funds were minted correctly
        AcurastParachain::execute_with(|| {
            let _events = acurast_runtime::System::events();
            let alice_balance_69 = AcurastMinter::balance(69, &ALICE);
            let alice_balance_false = AcurastMinter::balance(42, &ALICE);
            let alice_balance_native = acurast_runtime::Balances::free_balance(&ALICE);
            assert_eq!(alice_balance_false, 0);
            // assert_eq!(alice_balance_native, 1453652000000);
            assert_eq!(alice_balance_69, INITIAL_BALANCE / 2);
        })
    }

    #[test]
    fn pallet_assets_is_callable_in_runtime() {
        Network::reset();

        AcurastParachain::execute_with(|| {
            let bstrin = <AcurastRuntime as pallet_acurast::Config>::PalletId::get();
            let sstrin = String::from_utf8_lossy(&bstrin.0);
            let pallet_account: <AcurastRuntime as frame_system::Config>::AccountId =
                <AcurastRuntime as pallet_acurast::Config>::PalletId::get()
                    .into_account_truncating();

            let hex_acc = format!("{:x?}", pallet_account);
            let raw_origin =
                RawOrigin::<<AcurastRuntime as frame_system::Config>::AccountId>::Signed(
                    pallet_account.clone(),
                );
            let pallet_origin: <AcurastRuntime as frame_system::Config>::Origin = raw_origin.into();

            let _result = pallet_assets::Pallet::<AcurastRuntime>::create(
                pallet_origin,
                420,
                <AcurastRuntime as frame_system::Config>::Lookup::unlookup(ALICE.clone()),
                1,
            );
            let _x = 10;
        });

        AcurastParachain::execute_with(|| {
            let _events = acurast_runtime::System::events();
            let _x = 1; // put breakpoint here
        });
    }

    #[test]
    fn create_job_and_fulfill_local() {
        let job_token = MultiAsset {
            id: Concrete(MultiLocation {
                parents: 1,
                interior: X3(Parachain(1000), PalletInstance(50), GeneralIndex(69)),
            }),
            fun: Fungible(INITIAL_BALANCE / 2),
        };
        let alice_origin = acurast_runtime::Origin::signed(ALICE.clone());

        // fund alice's account with job payment tokens
        send_native_and_token();

        // register job and assign processor
        AcurastParachain::execute_with(|| {
            use acurast_runtime::Call::Acurast;
            use pallet_acurast::Call::{register, update_job_assignments};

            let register_call = Acurast(register {
                registration: JobRegistration {
                    script: SCRIPT_BYTES.to_vec().try_into().unwrap(),
                    allowed_sources: None,
                    allow_only_verified_sources: false,
                    payment: job_token.clone(),
                    extra: (),
                },
            });

            let dispatch_status = register_call.dispatch(alice_origin.clone());
            assert_ok!(dispatch_status);

            let updates = vec![JobAssignmentUpdate {
                operation: ListUpdateOperation::Add,
                assignee: BOB.clone(),
                job_id: (ALICE.clone(), SCRIPT_BYTES.to_vec().try_into().unwrap()),
            }];
            let assign_call = Acurast(update_job_assignments { updates });
            let _dispatch_status = assign_call.dispatch(alice_origin.clone());
            let _x = 10;
        });

        // check job event
        AcurastParachain::execute_with(|| {
            let _events = acurast_runtime::System::events();
            let _alice_balance_69 = AcurastMinter::balance(69, &ALICE);
            let _bob_balance_69 = AcurastMinter::balance(69, &BOB);
            let pallet_account: <AcurastRuntime as frame_system::Config>::AccountId =
                <AcurastRuntime as pallet_acurast::Config>::PalletId::get()
                    .into_account_truncating();
            let _pallet_balance_69 = AcurastMinter::balance(69, pallet_account);
            let _alice_balance_false = AcurastMinter::balance(42, &ALICE);
            let _alice_balance_native = acurast_runtime::Balances::free_balance(&ALICE);
            let _x = 10;
        });

        // fulfill job
        AcurastParachain::execute_with(|| {
            use acurast_runtime::Call::Acurast;
            use pallet_acurast::Call::fulfill;
            let payload: [u8; 32] = rand::random();
            let fulfillment = Fulfillment {
                script: SCRIPT_BYTES.to_vec().try_into().unwrap(),
                payload: payload.to_vec(),
            };
            let fulfill_call = Acurast(fulfill {
                fulfillment,
                requester: sp_runtime::MultiAddress::Id(ALICE.clone()),
            });
            let bob_origin = acurast_runtime::Origin::signed(BOB);
            let dispatch_status = fulfill_call.dispatch(bob_origin);
            assert_ok!(dispatch_status);
        });

        // check fulfill event
        AcurastParachain::execute_with(|| {
            let _events = acurast_runtime::System::events();
            let _alice_balance_69 = AcurastMinter::balance(69, &ALICE);
            let _alice_balance_native = acurast_runtime::Balances::free_balance(&ALICE);
            let bob_balance_69 = AcurastMinter::balance(69, &BOB);
            let _bob_balance_native = acurast_runtime::Balances::free_balance(&BOB);
            let pallet_account: <AcurastRuntime as frame_system::Config>::AccountId =
                <AcurastRuntime as pallet_acurast::Config>::PalletId::get()
                    .into_account_truncating();
            let _pallet_balance_69 = AcurastMinter::balance(69, pallet_account);
            assert_eq!(bob_balance_69, INITIAL_BALANCE / 2)
        })
    }
}

#[cfg(test)]
mod xcmp_panic {
    use super::*;
    use frame_support::assert_ok;

    use crate::{
        acurast_runtime, statemint_runtime, AcurastMinter, AcurastParachain, Network,
        StatemintMinter, StatemintParachain, StatemintXcmPallet, ALICE, INITIAL_BALANCE,
    };

    #[test]
    fn panics() {
        Network::reset();
        // create acurast native token in statemint to pay for execution of xcm
        StatemintParachain::execute_with(|| {
            let result = StatemintMinter::create(
                statemint_runtime::Origin::signed(ALICE),
                42,
                sp_runtime::MultiAddress::Id(ALICE),
                10,
            );
            assert_ok!(result);

            let result = StatemintMinter::mint(
                statemint_runtime::Origin::signed(ALICE),
                42,
                sp_runtime::MultiAddress::Id(ALICE),
                INITIAL_BALANCE,
            );
            assert_ok!(result);

            let alice_balance = StatemintMinter::balance(42, &ALICE);

            assert_eq!(alice_balance, INITIAL_BALANCE);
        });

        // create another token in statemint to pay for job
        StatemintParachain::execute_with(|| {
            let result = StatemintMinter::create(
                statemint_runtime::Origin::signed(ALICE),
                69,
                sp_runtime::MultiAddress::Id(ALICE),
                10,
            );
            assert_ok!(result);

            let result = StatemintMinter::mint(
                statemint_runtime::Origin::signed(ALICE),
                69,
                sp_runtime::MultiAddress::Id(ALICE),
                INITIAL_BALANCE,
            );
            assert_ok!(result);

            let alice_balance = StatemintMinter::balance(69, &ALICE);

            assert_eq!(alice_balance, INITIAL_BALANCE);
        });

        // transfer both tokens to alice's account in acurast
        StatemintParachain::execute_with(|| {
            let xcm = StatemintXcmPallet::reserve_transfer_assets(
                statemint_runtime::Origin::signed(ALICE),
                Box::new(
                    MultiLocation {
                        parents: 1,
                        interior: X1(Parachain(2000)),
                    }
                    .into(),
                ),
                Box::new(
                    X1(Junction::AccountId32 {
                        network: NetworkId::Any,
                        id: ALICE.into(),
                    })
                    .into()
                    .into(),
                ),
                Box::new(
                    vec![
                        // MultiAsset {
                        // 	id: Concrete(X2(PalletInstance(50), GeneralIndex(42)).into()),
                        // 	fun: Fungible(INITIAL_BALANCE/2)
                        // },
                        MultiAsset {
                            id: Concrete(X2(PalletInstance(50), GeneralIndex(69)).into()),
                            fun: Fungible(INITIAL_BALANCE / 2),
                        },
                        MultiAsset {
                            id: Concrete(MultiLocation {
                                parents: 1,
                                interior: Here,
                            }),
                            fun: Fungible(INITIAL_BALANCE / 2),
                        },
                    ]
                    .into(),
                ),
                1,
            );
            assert_ok!(xcm);
        });

        // check events in debug
        StatemintParachain::execute_with(|| {
            let _events = statemint_runtime::System::events();
            let _x = 1; // put breakpoint here
        });

        // check that funds were minted correctly
        AcurastParachain::execute_with(|| {
            let _events = acurast_runtime::System::events();
            let alice_balance_69 = AcurastMinter::balance(69, &ALICE);
            let alice_balance_false = AcurastMinter::balance(42, &ALICE);
            let alice_balance_native = acurast_runtime::Balances::free_balance(&ALICE);
            assert_eq!(alice_balance_false, 0);
            assert_eq!(alice_balance_native, 1453652000000);
            assert_eq!(alice_balance_69, INITIAL_BALANCE / 2);
        })
    }
}
