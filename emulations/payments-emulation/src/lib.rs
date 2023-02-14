#![allow(dead_code)]

extern crate core;

// parent re-exports
use emulations::{
	emulators::xcm_emulator,
	runtimes::{acurast_runtime, polkadot_runtime, proxy_parachain_runtime, statemint_runtime},
};

// needed libs
use crate::acurast_runtime::pallet_acurast;
use cumulus_primitives_core::ParaId;
use frame_support::{dispatch::Dispatchable, traits::GenesisBuild, weights::Weight};
use hex_literal::hex;
use polkadot_parachain::primitives::Sibling;
use sp_runtime::{
	bounded_vec,
	traits::{
		AccountIdConversion, AccountIdLookup, BlakeTwo256, ConstU128, ConstU32, StaticLookup,
	},
	AccountId32, BoundedVec,
};
use xcm::latest::prelude::*;
use xcm_emulator::{decl_test_network, decl_test_parachain, decl_test_relay_chain};

use acurast_runtime::{
	pallet_acurast::{JobRegistration, Schedule},
	pallet_acurast_marketplace::{
		types::MAX_PRICING_VARIANTS, Advertisement, FeeManager, JobRequirements, PricingVariant,
		SchedulingWindow,
	},
	AccountId, AcurastAsset, AcurastAssetId, AcurastBalance, FeeManagement, InternalAssetId,
	Runtime as AcurastRuntime,
};

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
		RuntimeOrigin = acurast_runtime::RuntimeOrigin,
		XcmpMessageHandler = acurast_runtime::XcmpQueue,
		DmpMessageHandler = acurast_runtime::DmpQueue,
		new_ext = acurast_ext(ACURAST_CHAIN_ID),
	}
}

decl_test_parachain! {
	pub struct ProxyParachain {
		Runtime = proxy_parachain_runtime::Runtime,
		RuntimeOrigin = proxy_parachain_runtime::RuntimeOrigin,
		XcmpMessageHandler = proxy_parachain_runtime::XcmpQueue,
		DmpMessageHandler = proxy_parachain_runtime::DmpQueue,
		new_ext = proxy_ext(PROXY_CHAIN_ID),
	}
}

decl_test_parachain! {
	pub struct StatemintParachain {
		Runtime = statemint_runtime::Runtime,
		RuntimeOrigin = statemint_runtime::RuntimeOrigin,
		XcmpMessageHandler = statemint_runtime::XcmpQueue,
		DmpMessageHandler = statemint_runtime::DmpQueue,
		new_ext = statemint_ext(STATEMINT_CHAIN_ID),
	}
}

decl_test_network! {
	pub struct Network {
		relay_chain = PolkadotRelay,
		parachains = vec![
			(2000, ProxyParachain),
			(2001, AcurastParachain),
			(1000, StatemintParachain),
		],
	}
}

pub const PROXY_CHAIN_ID: u32 = 2000; // make this match parachains in decl_test_network!
pub const ACURAST_CHAIN_ID: u32 = 2001; // make this match parachains in decl_test_network!
pub const STATEMINT_CHAIN_ID: u32 = 1000; // make this match parachains in decl_test_network!
pub const STATEMINT_ASSETS_PALLET_INDEX: u8 = 50; // make this match pallet index

pub const ALICE: AccountId32 = AccountId32::new([4u8; 32]);
pub const BOB: AccountId32 = AccountId32::new([8u8; 32]);
pub const FERDIE: AccountId32 = AccountId32::new([5u8; 32]);
pub const BURN_ACCOUNT: AccountId32 = AccountId32::new([0u8; 32]);

pub const INITIAL_BALANCE: u128 = 1_000_000_000_000;

const STATEMINT_NATIVE_ID: u32 = 100;
const STATEMINT_NATIVE_IS_SUFFICIENT: bool = true;
const STATEMINT_NATIVE_MIN_BALANCE: u128 = 1;
const STATEMINT_NATIVE_INITIAL_BALANCE: u128 = INITIAL_BALANCE * 100;
const STATEMINT_NATIVE_TOKEN_NAME: &str = "reserved_native_asset";
const STATEMINT_NATIVE_TOKEN_SYMBOL: &str = "RNA";
const STATEMINT_NATIVE_TOKEN_DECIMALS: u8 = 12;

const TEST_TOKEN_ID: u32 = 22;
const TEST_TOKEN_NAME: &str = "acurast_test_asset";
const TEST_TOKEN_SYMBOL: &str = "ACRST_TEST";
const TEST_TOKEN_DECIMALS: u8 = 12;
const TEST_TOKEN_IS_SUFFICIENT: bool = false;
const TEST_TOKEN_MIN_BALANCE: u128 = 1_000;
const TEST_TOKEN_INITIAL_BALANCE: u128 = 1_000_000_000_000_000;

pub fn acurast_ext(para_id: u32) -> sp_io::TestExternalities {
	use acurast_runtime::{Runtime, System};

	let mut t = frame_system::GenesisConfig::default().build_storage::<Runtime>().unwrap();

	let parachain_info_config =
		parachain_info::GenesisConfig { parachain_id: ParaId::from(para_id) };

	<parachain_info::GenesisConfig as GenesisBuild<Runtime, _>>::assimilate_storage(
		&parachain_info_config,
		&mut t,
	)
	.unwrap();

	let pallet_assets_account: <Runtime as frame_system::Config>::AccountId =
		<Runtime as pallet_acurast::Config>::PalletId::get().into_account_truncating();

	let fee_manager_account: <Runtime as frame_system::Config>::AccountId =
		acurast_runtime::FeeManagerPalletId::get().into_account_truncating();

	pallet_balances::GenesisConfig::<Runtime> {
		balances: vec![
			(ALICE, INITIAL_BALANCE),
			(BOB, INITIAL_BALANCE),
			(FERDIE, INITIAL_BALANCE),
			(pallet_assets_account, INITIAL_BALANCE),
			(fee_manager_account, INITIAL_BALANCE),
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
		assets: vec![
			(
				STATEMINT_NATIVE_ID,
				BURN_ACCOUNT,
				STATEMINT_NATIVE_IS_SUFFICIENT,
				STATEMINT_NATIVE_MIN_BALANCE,
			),
			(TEST_TOKEN_ID, ALICE, TEST_TOKEN_IS_SUFFICIENT, TEST_TOKEN_MIN_BALANCE),
		],
		metadata: vec![
			(
				STATEMINT_NATIVE_ID,
				STATEMINT_NATIVE_TOKEN_NAME.as_bytes().to_vec(),
				STATEMINT_NATIVE_TOKEN_SYMBOL.as_bytes().to_vec(),
				STATEMINT_NATIVE_TOKEN_DECIMALS,
			),
			(
				TEST_TOKEN_ID,
				TEST_TOKEN_NAME.as_bytes().to_vec(),
				TEST_TOKEN_SYMBOL.as_bytes().to_vec(),
				TEST_TOKEN_DECIMALS,
			),
		],
		accounts: vec![
			(STATEMINT_NATIVE_ID, BURN_ACCOUNT, STATEMINT_NATIVE_INITIAL_BALANCE),
			(TEST_TOKEN_ID, FERDIE, TEST_TOKEN_INITIAL_BALANCE),
		],
	}
	.assimilate_storage(&mut t)
	.unwrap();

	acurast_runtime::pallet_acurast_assets::GenesisConfig::<Runtime> {
		assets: vec![
			(
				STATEMINT_NATIVE_ID,
				STATEMINT_CHAIN_ID,
				STATEMINT_ASSETS_PALLET_INDEX,
				STATEMINT_NATIVE_ID as u128,
			),
			(
				TEST_TOKEN_ID,
				STATEMINT_CHAIN_ID,
				STATEMINT_ASSETS_PALLET_INDEX,
				TEST_TOKEN_ID as u128,
			),
		],
	}
	.assimilate_storage(&mut t)
	.unwrap();

	let mut ext = sp_io::TestExternalities::new(t);
	ext.execute_with(|| System::set_block_number(1));
	ext
}

pub fn proxy_ext(para_id: u32) -> sp_io::TestExternalities {
	use proxy_parachain_runtime::{Runtime, System};

	let mut t = frame_system::GenesisConfig::default().build_storage::<Runtime>().unwrap();

	let parachain_info_config =
		parachain_info::GenesisConfig { parachain_id: ParaId::from(para_id) };

	<parachain_info::GenesisConfig as GenesisBuild<Runtime, _>>::assimilate_storage(
		&parachain_info_config,
		&mut t,
	)
	.unwrap();

	pallet_balances::GenesisConfig::<Runtime> { balances: vec![(ALICE, INITIAL_BALANCE)] }
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

	let mut t = frame_system::GenesisConfig::default().build_storage::<Runtime>().unwrap();

	let parachain_info_config =
		parachain_info::GenesisConfig { parachain_id: ParaId::from(para_id) };

	<parachain_info::GenesisConfig as GenesisBuild<Runtime, _>>::assimilate_storage(
		&parachain_info_config,
		&mut t,
	)
	.unwrap();

	pallet_balances::GenesisConfig::<Runtime> {
		balances: vec![
			(BURN_ACCOUNT, INITIAL_BALANCE),
			(ALICE, INITIAL_BALANCE),
			(sibling_para_account_id(ACURAST_CHAIN_ID), INITIAL_BALANCE),
			(sibling_para_account_id(PROXY_CHAIN_ID), INITIAL_BALANCE),
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
		ump_service_total_weight: Weight::from_ref_time(4 * 1_000_000_000),
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

	let mut t = frame_system::GenesisConfig::default().build_storage::<Runtime>().unwrap();

	pallet_balances::GenesisConfig::<Runtime> {
		// IMPORTANT: to do reserve transfers, the sovereign account of the parachains needs to hold
		// a minimum amount of DOT called "existential deposit". Otherwise transfers will fail at
		// the point of internal_transfer_asset method of the AssetTransactor in xcm_executor::Config
		balances: vec![
			(ALICE, INITIAL_BALANCE),
			(child_para_account_id(ACURAST_CHAIN_ID), INITIAL_BALANCE),
			(child_para_account_id(PROXY_CHAIN_ID), INITIAL_BALANCE),
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

pub fn sibling_para_account_id(id: u32) -> polkadot_core_primitives::AccountId {
	// ParaId::from(id).into_account_truncating()
	Sibling::from(id).into_account_truncating()
}

pub fn child_para_account_id(id: u32) -> polkadot_core_primitives::AccountId {
	// ParaId::from(id).into_account_truncating()
	ParaId::from(id).into_account_truncating()
}

// Helper function for forming buy execution message
fn buy_execution<C>(fees: impl Into<MultiAsset>) -> Instruction<C> {
	BuyExecution { fees: fees.into(), weight_limit: Unlimited }
}

type AcurastXcmPallet = pallet_xcm::Pallet<acurast_runtime::Runtime>;
type PolkadotXcmPallet = pallet_xcm::Pallet<polkadot_runtime::Runtime>;
type StatemintXcmPallet = pallet_xcm::Pallet<statemint_runtime::Runtime>;

type StatemintAssets = pallet_assets::Pallet<statemint_runtime::Runtime>;
type AcurastAssetsInternal = pallet_assets::Pallet<acurast_runtime::Runtime>;
type AcurastAssets = acurast_runtime::pallet_acurast_assets::Pallet<acurast_runtime::Runtime>;

pub fn para_account_id(id: u32) -> AccountId32 {
	ParaId::from(id).into_account_truncating()
}
pub fn processor_account_id() -> AccountId32 {
	hex!("b8bc25a2b4c0386b8892b43e435b71fe11fa50533935f027949caf04bcce4694").into()
}
pub fn pallet_assets_account() -> <AcurastRuntime as frame_system::Config>::AccountId {
	<AcurastRuntime as pallet_acurast::Config>::PalletId::get().into_account_truncating()
}
pub fn pallet_fees_account() -> <AcurastRuntime as frame_system::Config>::AccountId {
	FeeManagement::pallet_id().into_account_truncating()
}
pub fn alice_account_id() -> AccountId32 {
	[0; 32].into()
}
pub fn bob_account_id() -> AccountId32 {
	[1; 32].into()
}
const SCRIPT_BYTES: [u8; 53] = hex!("697066733A2F2F00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000");

pub fn owned_asset(amount: u128) -> AcurastAsset {
	AcurastAsset(MultiAsset {
		id: Concrete(MultiLocation {
			parents: 1,
			interior: X3(
				Parachain(STATEMINT_CHAIN_ID),
				PalletInstance(STATEMINT_ASSETS_PALLET_INDEX),
				GeneralIndex(TEST_TOKEN_ID as u128),
			),
		}),
		fun: Fungible(amount),
	})
}

pub fn registration() -> JobRegistration<AccountId32, JobRequirements<AcurastAsset, AccountId32>> {
	JobRegistration {
		script: SCRIPT_BYTES.to_vec().try_into().unwrap(),
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
		extra: JobRequirements { slots: 1, reward: owned_asset(20000), instant_match: None },
	}
}
pub fn asset(id: u32) -> AssetId {
	AssetId::Concrete(MultiLocation::new(
		1,
		X3(
			Parachain(STATEMINT_CHAIN_ID),
			PalletInstance(STATEMINT_ASSETS_PALLET_INDEX),
			GeneralIndex(id as u128),
		),
	))
}
pub fn advertisement(
	fee_per_millisecond: u128,
) -> Advertisement<AccountId, AcurastAssetId, AcurastBalance> {
	let pricing: BoundedVec<
		PricingVariant<AcurastAssetId, AcurastBalance>,
		ConstU32<MAX_PRICING_VARIANTS>,
	> = bounded_vec![PricingVariant {
		reward_asset: asset(TEST_TOKEN_ID),
		fee_per_millisecond,
		fee_per_storage_byte: 0,
		base_fee_per_execution: 0,
		scheduling_window: SchedulingWindow::Delta(2_628_000_000), // 1 month
	}];
	Advertisement {
		pricing,
		allowed_consumers: None,
		storage_capacity: 5,
		max_memory: 5000,
		network_request_quota: 8,
	}
}

#[cfg(test)]
mod network_tests {
	use super::*;
	use codec::Encode;
	use frame_support::{assert_ok, traits::Currency};
	use xcm_emulator::TestExt;

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
				Box::new(
					X1(Junction::AccountId32 { network: Any, id: ALICE.into() }).into().into()
				),
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
}

#[cfg(test)]
mod statemint_backed_native_assets {
	use super::*;
	use frame_support::assert_ok;
	use xcm_emulator::TestExt;

	#[test]
	fn can_recreate() {
		Network::reset();
		AcurastParachain::execute_with(|| {
			let result = AcurastAssets::create(
				acurast_runtime::RuntimeOrigin::signed(ALICE),
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
}

#[cfg(test)]
mod jobs {
	use frame_support::{assert_ok, dispatch::RawOrigin};
	use hex_literal::hex;

	use super::*;
	use crate::{
		acurast_runtime::pallet_acurast,
		pallet_acurast::{JobRegistration, ListUpdateOperation},
	};

	// use pallet_acurast_marketplace::FeeManager;
	use emulations::runtimes::acurast_runtime::RegistrationExtra;
	use sp_runtime::BoundedVec;
	use xcm_emulator::TestExt;

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
			let alice_balance_native = acurast_runtime::Balances::free_balance(&ALICE);
			assert_eq!(alice_balance_false, 0);
			// assert_eq!(alice_balance_native, 1453652000000);
			assert_eq!(alice_balance_test_token, INITIAL_BALANCE / 2);
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

	// #[test]
	// fn create_match_report_job() {
	// 	use acurast_runtime::{RuntimeCall::AcurastMarketplace, Runtime as AcurastRuntime};
	// 	use pallet_acurast_marketplace::{
	// 		AdvertisementFor, Call::advertise, PricingVariant,
	// 	};
	// 	use mock::advertisement;
	//
	// 	let pallet_account: <AcurastRuntime as frame_system::Config>::AccountId =
	// 		<AcurastRuntime as pallet_acurast::Config>::PalletId::get().into_account_truncating();
	//
	// 	let reward_amount = INITIAL_BALANCE / 2;
	// 	let job_token = MultiAsset {
	// 		id: Concrete(MultiLocation {
	// 			parents: 1,
	// 			interior: X3(Parachain(STATEMINT_CHAIN_ID), PalletInstance(STATEMINT_ASSETS_PALLET_INDEX), GeneralIndex(TEST_ASSET_ID)),
	// 		}),
	// 		fun: Fungible(INITIAL_BALANCE / 2),
	// 	};
	// 	let alice_origin = acurast_runtime::Origin::signed(ALICE.clone());
	// 	let bob_origin = acurast_runtime::Origin::signed(BOB.clone());
	//
	// 	// fund alice's accounft with job payment tokens
	// 	send_native_and_token();
	//
	// 	// advertise resources
	// 	AcurastParachain::execute_with(|| {
	// 		let advertise_call = AcurastMarketplace(advertise {
	// 			advertisement: advertisement(10000u128),
	// 		});
	//
	// 		assert_ok!(advertise_call.dispatch(bob_origin.clone()));
	// 	});
	//
	// 	// register job
	// 	AcurastParachain::execute_with(|| {
	// 		use acurast_runtime::RuntimeCall::Acurast;
	// 		use acurast_runtime::pallet_acurast::Call::{register, update_job_assignments};
	//
	// 		let register_call = Acurast(register {
	// 			registration: registration(),
	// 		});
	//
	// 		let dispatch_status = register_call.dispatch(alice_origin.clone());
	// 		assert_ok!(dispatch_status);
	// 	});
	//
	// 	// check job event
	// 	AcurastParachain::execute_with(|| {
	// 		let _events: Vec<String> = acurast_runtime::System::events()
	// .iter()
	// .map(|e| format!("{:?}", e.event))
	// .collect();
	// 		let _alice_balance_test_token = AcurastAssetsInternal::balance(TEST_ASSET_ID, &ALICE);
	// 		let _bob_balance_test_token = AcurastAssetsInternal::balance(TEST_ASSET_ID, &BOB);
	//
	// 		let _pallet_balance_test_token = AcurastAssetsInternal::balance(TEST_ASSET_ID, pallet_account.clone());
	// 		let _alice_balance_false = AcurastAssetsInternal::balance(NATIVE_ASSET_ID, &ALICE);
	// 		let _alice_balance_native = acurast_runtime::Balances::free_balance(&ALICE);
	// 		let _x = 10;
	// 	});
	//
	// 	// fulfill job
	// 	AcurastParachain::execute_with(|| {
	// 		use acurast_runtime::Call::Acurast;
	// 		use acurast_runtime::pallet_acurast::Call::fulfill;
	// 		let payload: [u8; 32] = rand::random();
	// 		let fulfillment = Fulfillment {
	// 			script: SCRIPT_BYTES.to_vec().try_into().unwrap(),
	// 			payload: payload.to_vec(),
	// 		};
	//
	// 		let pallet_balance = pallet_assets::Pallet::<acurast_runtime::Runtime>::balance(
	// 			TEST_ASSET_ID,
	// 			pallet_account.clone(),
	// 		);
	// 		// 500_000_000_000
	// 		let fulfill_call = Acurast(fulfill {
	// 			fulfillment,
	// 			requester: sp_runtime::MultiAddress::Id(ALICE.clone()),
	// 		});
	// 		let bob_origin = acurast_runtime::Origin::signed(BOB);
	// 		let dispatch_status = fulfill_call.dispatch(bob_origin);
	// 		assert_ok!(dispatch_status);
	// 	});
	//
	// 	// check fulfill event
	// 	ProxyParachain::execute_with(|| {
	// 		use emulations::runtimes::proxy_parachain_runtime::{RuntimeEvent, System};
	// 		let events = System::events()
	// .iter()
	// .map(|e| format!("{:?}", e.event))
	// .collect();
	// 		assert!(events.iter().any(|r| matches!(r.event, RuntimeEvent::AcurastReceiver(..))));
	// 	});
	// }
	//
	//
	// fn next_block() {
	// 	if System::block_number() >= 1 {
	// 		// pallet_acurast_marketplace::on_finalize(System::block_number());
	// 		Timestamp::on_finalize(System::block_number());
	// 	}
	// 	System::set_block_number(System::block_number() + 1);
	// 	Timestamp::on_initialize(System::block_number());
	// }
	//
	// /// A helper function to move time on in tests. It ensures `Timestamp::set` is only called once per block by advancing the block otherwise.
	// fn later(now: u64) {
	// 	// If this is not the very first timestamp ever set, we always advance the block before setting new time
	// 	// this is because setting it twice in a block is not legal
	// 	if Timestamp::get() > 0 {
	// 		// pretend block was finalized
	// 		let b = System::block_number();
	// 		next_block(); // we cannot set time twice in same block
	// 		assert_eq!(b + 1, System::block_number());
	// 	}
	// 	// pretend time moved on
	// 	assert_ok!(Timestamp::set(RuntimeOrigin::none(), now));
	// }
}
