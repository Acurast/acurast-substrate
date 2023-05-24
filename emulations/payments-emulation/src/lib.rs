extern crate core;

#[cfg(test)]
mod tests {
	use cumulus_primitives_core::ParaId;
	use frame_support::{
		assert_ok,
		traits::{GenesisBuild, Hooks},
	};
	use hex_literal::hex;
	use sp_runtime::{
		bounded_vec,
		traits::{AccountIdConversion, ConstU32},
		AccountId32, BoundedVec,
	};
	use xcm::latest::prelude::*;
	use xcm_simulator::{decl_test_parachain, TestExt};

	use acurast_runtime::{
		pallet_acurast::Schedule,
		pallet_acurast_marketplace::{
			types::MAX_PRICING_VARIANTS, Advertisement, FeeManager, JobRequirements,
			PricingVariant, SchedulingWindow,
		},
		AccountId, AcurastAsset, AssetId, Balance, FeeManagement,
	};
	// parent re-exports
	use super::polkadot_ext;
	use emulations::{
		emulators::{
			xcm_simulator,
			xcm_simulator::{decl_test_network, decl_test_relay_chain},
		},
		runtimes::{
			acurast_runtime,
			acurast_runtime::{
				pallet_acurast, pallet_acurast_marketplace,
				pallet_acurast_marketplace::ExecutionOperationHash,
			},
			polkadot_runtime,
		},
	};

	mod jobs;

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
			XcmpMessageHandler = acurast_runtime::XcmpQueue,
			DmpMessageHandler = acurast_runtime::DmpQueue,
			new_ext = acurast_ext(ACURAST_CHAIN_ID),
		}
	}

	decl_test_network! {
		pub struct Network {
			relay_chain = PolkadotRelay,
			parachains = vec![
				(2001, AcurastParachain),
			],
		}
	}

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
				(TEST_TOKEN_ID, ALICE, TEST_TOKEN_INITIAL_BALANCE),
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

		acurast_runtime::pallet_acurast_processor_manager::GenesisConfig::<Runtime> {
			managers: vec![(ALICE, vec![BOB])],
		}
		.assimilate_storage(&mut t)
		.unwrap();

		let mut ext = sp_io::TestExternalities::new(t);
		ext.execute_with(|| System::set_block_number(1));
		ext
	}

	type Acurast = pallet_acurast::Pallet<acurast_runtime::Runtime>;
	type AcurastMarketplace = pallet_acurast_marketplace::Pallet<acurast_runtime::Runtime>;
	type AcurastAssetsInternal = pallet_assets::Pallet<acurast_runtime::Runtime>;

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
	) -> Advertisement<AccountId, AssetId, Balance, pallet_acurast::CU32<100>> {
		let pricing: BoundedVec<PricingVariant<AssetId, Balance>, ConstU32<MAX_PRICING_VARIANTS>> =
			bounded_vec![PricingVariant {
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

// add arg paras: Vec<u32>
pub fn polkadot_ext() -> sp_io::TestExternalities {
	use emulations::runtimes::polkadot_runtime::{Runtime, System};

	let t = frame_system::GenesisConfig::default().build_storage::<Runtime>().unwrap();

	let mut ext = sp_io::TestExternalities::new(t);
	ext.execute_with(|| {
		System::set_block_number(1);
	});
	ext
}
