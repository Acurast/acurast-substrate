extern crate core;

#[cfg(test)]
mod tests {
	use cumulus_primitives_core::ParaId;
	use frame_support::{
		assert_ok,
		dispatch::RawOrigin,
		traits::{GenesisBuild, Hooks},
		weights::Weight,
	};
	use hex_literal::hex;
	use polkadot_parachain::primitives::Sibling;
	use sp_runtime::{
		bounded_vec,
		traits::{AccountIdConversion, ConstU32, StaticLookup},
		AccountId32, BoundedVec,
	};
	use xcm::latest::prelude::*;
	use xcm_emulator::{decl_test_network, decl_test_parachain, decl_test_relay_chain, TestExt};

	use acurast_runtime::{
		pallet_acurast::Schedule,
		pallet_acurast_marketplace::{
			types::MAX_PRICING_VARIANTS, Advertisement, FeeManager, JobRequirements,
			PricingVariant, SchedulingWindow,
		},
		AccountId, AcurastAsset, AcurastAssetId, AcurastBalance, FeeManagement,
		Runtime as AcurastRuntime,
	};
	// parent re-exports
	use emulations::{
		emulators::xcm_emulator,
		runtimes::{
			acurast_runtime,
			acurast_runtime::{
				pallet_acurast, pallet_acurast_marketplace,
				pallet_acurast_marketplace::ExecutionOperationHash,
			},
			polkadot_runtime, proxy_parachain_runtime, statemint_runtime,
		},
	};

	mod jobs;
	mod network_tests;
	mod statemint_backed_native_assets;

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

	pub const PROXY_CHAIN_ID: u32 = 2000;
	// make this match parachains in decl_test_network!
	pub const ACURAST_CHAIN_ID: u32 = 2001;
	// make this match parachains in decl_test_network!
	pub const STATEMINT_CHAIN_ID: u32 = 1000;
	// make this match parachains in decl_test_network!
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
			<Runtime as acurast_runtime::pallet_acurast::Config>::PalletId::get()
				.into_account_truncating();

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
					acurast_runtime::AcurastPalletId::get().into_account_truncating(),
					STATEMINT_NATIVE_IS_SUFFICIENT,
					STATEMINT_NATIVE_MIN_BALANCE,
				),
				(
					TEST_TOKEN_ID,
					acurast_runtime::AcurastPalletId::get().into_account_truncating(),
					TEST_TOKEN_IS_SUFFICIENT,
					TEST_TOKEN_MIN_BALANCE,
				),
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

		acurast_runtime::pallet_acurast_assets_manager::GenesisConfig::<Runtime> {
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

		acurast_runtime::pallet_acurast::GenesisConfig::<Runtime> {
			attestations: vec![(BOB, None)],
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
	type Acurast = pallet_acurast::Pallet<acurast_runtime::Runtime>;
	type AcurastMarketplace = pallet_acurast_marketplace::Pallet<acurast_runtime::Runtime>;
	type AcurastAssetsInternal = pallet_assets::Pallet<acurast_runtime::Runtime>;
	type AcurastAssets =
		acurast_runtime::pallet_acurast_assets_manager::Pallet<acurast_runtime::Runtime>;

	/// Type representing the utf8 bytes of a string containing the value of an ipfs url.
	/// The ipfs url is expected to point to a script.
	pub type Script = BoundedVec<u8, ConstU32<53>>;

	const SCRIPT_BYTES: [u8; 53] = hex!("697066733A2F2F00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000");
	pub const OPERATION_HASH: [u8; 32] =
		hex!("a3f18e4c6f0cdd0d8666f407610351cacb9a263678cf058294be9977b69f2cb3");

	pub fn script() -> Script {
		SCRIPT_BYTES.to_vec().try_into().unwrap()
	}

	pub fn operation_hash() -> ExecutionOperationHash {
		OPERATION_HASH.to_vec().try_into().unwrap()
	}

	pub fn test_token_asset_id() -> AssetId {
		Concrete(MultiLocation {
			parents: 1,
			interior: X3(
				Parachain(STATEMINT_CHAIN_ID),
				PalletInstance(STATEMINT_ASSETS_PALLET_INDEX),
				GeneralIndex(TEST_TOKEN_ID as u128),
			),
		})
	}

	pub fn test_asset(amount: u128) -> AcurastAsset {
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

	pub fn advertisement(
		fee_per_millisecond: u128,
		fee_per_storage_byte: u128,
		storage_capacity: u32,
		max_memory: u32,
		network_request_quota: u8,
		scheduling_window: SchedulingWindow,
	) -> Advertisement<AccountId, AcurastAssetId, AcurastBalance, pallet_acurast::CU32<100>> {
		let pricing: BoundedVec<
			PricingVariant<AcurastAssetId, AcurastBalance>,
			ConstU32<MAX_PRICING_VARIANTS>,
		> = bounded_vec![PricingVariant {
			reward_asset: test_token_asset_id(),
			fee_per_millisecond,
			fee_per_storage_byte,
			base_fee_per_execution: 0,
			scheduling_window,
		}];
		Advertisement {
			pricing,
			allowed_consumers: None,
			storage_capacity,
			max_memory,
			network_request_quota,
			available_modules: vec![].try_into().unwrap(),
		}
	}
}
