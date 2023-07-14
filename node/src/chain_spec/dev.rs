use cumulus_primitives_core::ParaId;
use nimbus_primitives::NimbusId;
use sc_service::ChainType;
use sp_core::{sr25519, Pair, Public};
use sp_runtime::{
	traits::{AccountIdConversion, IdentifyAccount, Verify},
	AccountId32, Percent,
};

pub(crate) use acurast_rococo_runtime::{
	self as acurast_runtime, AcurastAssetsConfig, AcurastConfig, AcurastProcessorManagerConfig,
	AssetsConfig, DemocracyConfig, SudoConfig, EXISTENTIAL_DEPOSIT,
};
use acurast_runtime_common::*;

use crate::chain_spec::{accountid_from_str, processor_manager, Extensions, DEFAULT_PARACHAIN_ID};

/// Specialized `ChainSpec` for the normal parachain runtime.
pub type ChainSpec = sc_service::GenericChainSpec<acurast_runtime::GenesisConfig, Extensions>;

/// The default XCM version to set in genesis config.
const SAFE_XCM_VERSION: u32 = xcm::prelude::XCM_VERSION;

/// Helper function to generate a crypto pair from seed
pub fn get_from_seed<TPublic: Public>(seed: &str) -> <TPublic::Pair as Pair>::Public {
	TPublic::Pair::from_string(&format!("//{}", seed), None)
		.expect("static values are valid; qed")
		.public()
}

type AccountPublic = <Signature as Verify>::Signer;

const NATIVE_IS_SUFFICIENT: bool = true;
const NATIVE_MIN_BALANCE: u128 = 1_000_000_000_000;
const NATIVE_INITIAL_BALANCE: u128 = 1_000_000_000_000_000;
const NATIVE_TOKEN_NAME: &str = "reserved_native_asset";
const NATIVE_TOKEN_SYMBOL: &str = "ACRST";
const NATIVE_TOKEN_DECIMALS: u8 = 12;
const BURN_ACCOUNT: sp_runtime::AccountId32 = sp_runtime::AccountId32::new([0u8; 32]);

const FAUCET_INITIAL_BALANCE: u128 = 1_000_000_000_000_000;

const TEST_TOKEN_ID: u32 = 22;
const TEST_TOKEN_NAME: &str = "acurast_test_asset";
const TEST_TOKEN_SYMBOL: &str = "ACRST_TEST";
const TEST_TOKEN_DECIMALS: u8 = 12;
const TEST_TOKEN_IS_SUFFICIENT: bool = false;
const TEST_TOKEN_MIN_BALANCE: u128 = 1_000;
const TEST_TOKEN_INITIAL_BALANCE: u128 = 1_000_000_000_000_000;

fn get_test_token_holder() -> AccountId32 {
	get_account_id_from_seed::<sr25519::Public>("Ferdie")
}

/// Generate collator keys from seed.
///
/// This function's return type must always match the session keys of the chain in tuple format.
pub fn get_collator_keys_from_seed(seed: &str) -> NimbusId {
	get_from_seed::<NimbusId>(seed)
}

/// Helper function to generate an account ID from seed
pub fn get_account_id_from_seed<TPublic: Public>(seed: &str) -> AccountId
where
	AccountPublic: From<<TPublic::Pair as Pair>::Public>,
{
	AccountPublic::from(get_from_seed::<TPublic>(seed)).into_account()
}

/// Generate the session keys from individual elements.
///
/// The input must be a tuple of individual keys (a single arg for now since we have just one key).
pub fn acurast_session_keys(keys: NimbusId) -> acurast_runtime::SessionKeys {
	acurast_runtime::SessionKeys { nimbus: keys }
}

/// Returns the development [ChainSpec].
pub fn acurast_development_config() -> ChainSpec {
	// Give your base currency a unit name and decimal places
	let mut properties = sc_chain_spec::Properties::new();
	properties.insert("tokenSymbol".into(), "ACRST".into());
	properties.insert("tokenDecimals".into(), NATIVE_TOKEN_DECIMALS.into());
	properties.insert("ss58Format".into(), 42.into());

	ChainSpec::from_genesis(
		// Name
		"Development",
		// ID
		"dev",
		ChainType::Development,
		move || {
			genesis_config(
				// initial collators.
				vec![
					(
						get_account_id_from_seed::<sr25519::Public>("Alice"),
						get_collator_keys_from_seed("Alice"),
					),
					(
						get_account_id_from_seed::<sr25519::Public>("Bob"),
						get_collator_keys_from_seed("Bob"),
					),
				],
				vec![
					(get_account_id_from_seed::<sr25519::Public>("Alice"), 1 << 60),
					(get_account_id_from_seed::<sr25519::Public>("Bob"), 1 << 60),
					(get_account_id_from_seed::<sr25519::Public>("Charlie"), 1 << 60),
					(get_account_id_from_seed::<sr25519::Public>("Dave"), 1 << 60),
					(get_account_id_from_seed::<sr25519::Public>("Eve"), 1 << 60),
					(get_account_id_from_seed::<sr25519::Public>("Ferdie"), 1 << 60),
					(get_account_id_from_seed::<sr25519::Public>("Alice//stash"), 1 << 60),
					(get_account_id_from_seed::<sr25519::Public>("Bob//stash"), 1 << 60),
					(get_account_id_from_seed::<sr25519::Public>("Charlie//stash"), 1 << 60),
					(get_account_id_from_seed::<sr25519::Public>("Dave//stash"), 1 << 60),
					(get_account_id_from_seed::<sr25519::Public>("Eve//stash"), 1 << 60),
					(get_account_id_from_seed::<sr25519::Public>("Ferdie//stash"), 1 << 60),
					(acurast_pallet_account(), NATIVE_MIN_BALANCE),
					(fee_manager_pallet_account(), NATIVE_MIN_BALANCE),
					(acurast_faucet_account(), FAUCET_INITIAL_BALANCE),
					#[cfg(feature = "runtime-benchmarks")]
					([1; 32].into(), 1 << 60),
					#[cfg(feature = "runtime-benchmarks")]
					([2; 32].into(), 1 << 60),
				],
				DEFAULT_PARACHAIN_ID.into(),
				get_account_id_from_seed::<sr25519::Public>("Alice"),
				AcurastConfig { attestations: vec![(get_test_token_holder(), None)] },
				AssetsConfig {
					assets: vec![(
						TEST_TOKEN_ID,
						acurast_pallet_account(),
						TEST_TOKEN_IS_SUFFICIENT,
						TEST_TOKEN_MIN_BALANCE,
					)],
					metadata: vec![(
						TEST_TOKEN_ID,
						TEST_TOKEN_NAME.as_bytes().to_vec(),
						TEST_TOKEN_SYMBOL.as_bytes().to_vec(),
						TEST_TOKEN_DECIMALS,
					)],
					accounts: vec![(
						TEST_TOKEN_ID,
						get_test_token_holder(),
						TEST_TOKEN_INITIAL_BALANCE,
					)],
				},
			)
		},
		Vec::new(),
		None,
		None,
		None,
		Some(properties),
		Extensions {
			relay_chain: "atera-local".into(), // You MUST set this to the correct network!
			para_id: DEFAULT_PARACHAIN_ID,
		},
	)
}

/// Returns the testnet [acurast_runtime::GenesisConfig].
fn genesis_config(
	invulnerables: Vec<(AccountId, NimbusId)>,
	endowed_accounts: Vec<(AccountId, acurast_runtime::Balance)>,
	id: ParaId,
	sudo_account: AccountId,
	acurast: AcurastConfig,
	assets: AssetsConfig,
) -> acurast_runtime::GenesisConfig {
	acurast_runtime::GenesisConfig {
		system: acurast_runtime::SystemConfig {
			code: acurast_runtime::WASM_BINARY
				.expect("WASM binary was not build, please build it!")
				.to_vec(),
		},
		balances: acurast_runtime::BalancesConfig { balances: endowed_accounts },
		parachain_info: acurast_runtime::ParachainInfoConfig { parachain_id: id },
		collator_selection: acurast_runtime::CollatorSelectionConfig {
			invulnerables: invulnerables.iter().cloned().map(|(acc, _)| acc).collect(),
			candidacy_bond: EXISTENTIAL_DEPOSIT * 16,
			..Default::default()
		},
		session: acurast_runtime::SessionConfig {
			keys: invulnerables
				.clone()
				.into_iter()
				.map(|(acc, session_keys)| {
					(
						acc.clone(),                        // account id
						acc,                                // validator id
						acurast_session_keys(session_keys), // session keys
					)
				})
				.collect(),
		},
		parachain_system: Default::default(),
		parachain_staking: acurast_runtime::ParachainStakingConfig {
			blocks_per_round: 3600u32.into(), // 3600 * ~12s = ~12h (TBD)
			collator_commission: Perbill::from_percent(20), // TBD
			num_selected_candidates: 128u32.into(),
			parachain_bond_reserve_percent: Percent::from_percent(30), // TBD
			candidates: invulnerables
				.into_iter()
				.map(|(acc, _)| (acc, staking_info::MINIMUM_COLLATOR_STAKE))
				.collect(),
			delegations: vec![],
			inflation_config: staking_info::DEFAULT_INFLATION_CONFIG,
		},
		polkadot_xcm: acurast_runtime::PolkadotXcmConfig {
			safe_xcm_version: Some(SAFE_XCM_VERSION),
		},
		sudo: SudoConfig { key: Some(sudo_account) },
		acurast,
		acurast_processor_manager: acurast_processor_manager_config(),
		assets: AssetsConfig {
			assets: vec![(
				acurast_runtime::xcm_config::NativeAssetId::get(),
				acurast_pallet_account(),
				NATIVE_IS_SUFFICIENT,
				NATIVE_MIN_BALANCE,
			)]
			.into_iter()
			.chain(assets.assets.clone())
			.collect(),
			metadata: vec![(
				acurast_runtime::xcm_config::NativeAssetId::get(),
				NATIVE_TOKEN_NAME.as_bytes().to_vec(),
				NATIVE_TOKEN_SYMBOL.as_bytes().to_vec(),
				NATIVE_TOKEN_DECIMALS,
			)]
			.into_iter()
			.chain(assets.metadata)
			.collect(),
			accounts: vec![(
				acurast_runtime::xcm_config::NativeAssetId::get(),
				BURN_ACCOUNT,
				NATIVE_INITIAL_BALANCE,
			)]
			.into_iter()
			.chain(assets.accounts)
			.collect(),
		},
		acurast_assets: AcurastAssetsConfig {
			assets: vec![(
				100u32,
				acurast_runtime::xcm_config::StatemintChainId::get(),
				acurast_runtime::xcm_config::StatemintAssetsPalletIndex::get(),
				acurast_runtime::xcm_config::NativeAssetId::get() as u128,
			)]
			.into_iter()
			.chain(assets.assets.iter().map(|asset| {
				(
					asset.0,
					acurast_runtime::xcm_config::StatemintChainId::get(),
					acurast_runtime::xcm_config::StatemintAssetsPalletIndex::get(),
					asset.0 as u128,
				)
			}))
			.collect(),
		},
		democracy: DemocracyConfig::default(),
	}
}

/// Returns the pallet_acurast account id.
pub fn acurast_pallet_account() -> AccountId {
	acurast_runtime::AcurastPalletId::get().into_account_truncating()
}

/// Returns the pallet_fee_manager account id.
pub fn fee_manager_pallet_account() -> AccountId {
	acurast_runtime::FeeManagerPalletId::get().into_account_truncating()
}

/// returns the faucet account id.
pub fn acurast_faucet_account() -> AccountId {
	accountid_from_str("5EyaQQEQzzXdfsvFfscDaQUFiGBk5hX4B38j1x3rH7Zko2QJ")
}

fn acurast_processor_manager_config() -> AcurastProcessorManagerConfig {
	AcurastProcessorManagerConfig { managers: processor_manager() }
}
