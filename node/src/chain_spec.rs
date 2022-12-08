use acurast_runtime::{
	pallet_acurast, AccountId, AssetsConfig, AuraId, Runtime, Signature, EXISTENTIAL_DEPOSIT,
	SudoConfig
};
use std::str::FromStr;

use cumulus_primitives_core::ParaId;
use sc_chain_spec::{ChainSpecExtension, ChainSpecGroup};
use sc_service::ChainType;
use serde::{Deserialize, Serialize};
use sp_core::{sr25519, Pair, Public};
use sp_runtime::{
	traits::{AccountIdConversion, IdentifyAccount, Verify},
	AccountId32,
};

/// Specialized `ChainSpec` for the normal parachain runtime.
pub type ChainSpec = sc_service::GenericChainSpec<acurast_runtime::GenesisConfig, Extensions>;
pub type Balance = <Runtime as pallet_balances::Config>::Balance;

/// The default XCM version to set in genesis config.
const SAFE_XCM_VERSION: u32 = xcm::prelude::XCM_VERSION;

/// Helper function to generate a crypto pair from seed
pub fn get_public_from_seed<TPublic: Public>(seed: &str) -> <TPublic::Pair as Pair>::Public {
	<TPublic::Pair as Pair>::from_string(&format!("//{}", seed), None)
		.expect("static values are valid; qed")
		.public()
}

/// The extensions for the [`ChainSpec`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ChainSpecGroup, ChainSpecExtension)]
#[serde(deny_unknown_fields)]
pub struct Extensions {
	/// The relay chain of the Parachain.
	pub relay_chain: String,
	/// The id of the Parachain.
	pub para_id: u32,
}

impl Extensions {
	/// Try to get the extension from the given `ChainSpec`.
	pub fn try_get(chain_spec: &dyn sc_service::ChainSpec) -> Option<&Self> {
		sc_chain_spec::get_extension(chain_spec.extensions())
	}
}

type AccountPublic = <Signature as Verify>::Signer;

const DEFAULT_PARACHAIN_ID: u32 = 2001;

#[cfg(feature = "runtime-benchmarks")]
mod benchmark_items {
	pub fn acurast_consumer_account() -> sp_runtime::AccountId32 {
		pallet_acurast_marketplace::benchmarking::consumer_account::<acurast_runtime::Runtime>()
	}
	pub fn acurast_processor_account() -> sp_runtime::AccountId32 {
		pallet_acurast_marketplace::benchmarking::processor_account::<acurast_runtime::Runtime>()
	}
}
/// Generate collator keys from seed.
///
/// This function's return type must always match the session keys of the chain in tuple format.
pub fn get_collator_keys_from_seed(seed: &str) -> AuraId {
	get_public_from_seed::<AuraId>(seed)
}

/// Helper function to generate an account ID from seed
pub fn get_account_id_from_seed<TPublic: Public>(seed: &str) -> AccountId
where
	AccountPublic: From<<TPublic::Pair as Pair>::Public>,
{
	AccountPublic::from(get_public_from_seed::<TPublic>(seed)).into_account()
}

/// Generate the session keys from individual elements.
///
/// The input must be a tuple of individual keys (a single arg for now since we have just one key).
pub fn template_session_keys(keys: AuraId) -> acurast_runtime::SessionKeys {
	acurast_runtime::SessionKeys { aura: keys }
}

//noinspection ALL
pub fn acurast_development_config() -> ChainSpec {
	// Give your base currency a unit name and decimal places
	let mut properties = sc_chain_spec::Properties::new();
	properties.insert("tokenSymbol".into(), "ACRST".into());
	properties.insert("tokenDecimals".into(), token_decimals().into());
	properties.insert("ss58Format".into(), 42.into());

	ChainSpec::from_genesis(
		// Name
		"Development",
		// ID
		"dev",
		ChainType::Development,
		move || {
			testnet_genesis(
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
					get_account_id_from_seed::<sr25519::Public>("Alice"),
					get_account_id_from_seed::<sr25519::Public>("Bob"),
					get_account_id_from_seed::<sr25519::Public>("Charlie"),
					get_account_id_from_seed::<sr25519::Public>("Dave"),
					get_account_id_from_seed::<sr25519::Public>("Eve"),
					get_account_id_from_seed::<sr25519::Public>("Ferdie"),
					get_account_id_from_seed::<sr25519::Public>("Alice//stash"),
					get_account_id_from_seed::<sr25519::Public>("Bob//stash"),
					get_account_id_from_seed::<sr25519::Public>("Charlie//stash"),
					get_account_id_from_seed::<sr25519::Public>("Dave//stash"),
					get_account_id_from_seed::<sr25519::Public>("Eve//stash"),
					get_account_id_from_seed::<sr25519::Public>("Ferdie//stash"),
					acurast_pallet_account(),
					fee_manager_pallet_account(),
				],
				DEFAULT_PARACHAIN_ID.into(),
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

//noinspection ALL
pub fn rococo_development_config() -> ChainSpec {
	// Give your base currency a unit name and decimal places
	let mut properties = sc_chain_spec::Properties::new();
	properties.insert("tokenSymbol".into(), "ACRST".into());
	properties.insert("tokenDecimals".into(), token_decimals().into());
	properties.insert("ss58Format".into(), 42.into());

	ChainSpec::from_genesis(
		// Name
		"Acurast Testnet",
		// ID
		"acurast_testnet",
		ChainType::Local,
		move || {
			testnet_genesis(
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
					get_account_id_from_seed::<sr25519::Public>("Alice"),
					get_account_id_from_seed::<sr25519::Public>("Bob"),
					get_account_id_from_seed::<sr25519::Public>("Charlie"),
					get_account_id_from_seed::<sr25519::Public>("Dave"),
					get_account_id_from_seed::<sr25519::Public>("Eve"),
					get_account_id_from_seed::<sr25519::Public>("Ferdie"),
					get_account_id_from_seed::<sr25519::Public>("Alice//stash"),
					get_account_id_from_seed::<sr25519::Public>("Bob//stash"),
					get_account_id_from_seed::<sr25519::Public>("Charlie//stash"),
					get_account_id_from_seed::<sr25519::Public>("Dave//stash"),
					get_account_id_from_seed::<sr25519::Public>("Eve//stash"),
					get_account_id_from_seed::<sr25519::Public>("Ferdie//stash"),
					acurast_pallet_account(),
					fee_manager_pallet_account(),
				],
				DEFAULT_PARACHAIN_ID.into(),
			)
		},
		// Bootnodes
		Vec::new(),
		// Telemetry
		None,
		// Protocol ID
		Some("acurast-local"),
		// Fork ID
		None,
		// Properties
		Some(properties),
		// Extensions
		Extensions {
			relay_chain: "rococo-local".into(), // You MUST set this to the correct network!
			para_id: DEFAULT_PARACHAIN_ID,
		},
	)
}

fn testnet_genesis(
	invulnerables: Vec<(AccountId, AuraId)>,
	endowed_accounts: Vec<AccountId>,
	id: ParaId,
) -> acurast_runtime::GenesisConfig {
	acurast_runtime::GenesisConfig {
		system: acurast_runtime::SystemConfig {
			code: acurast_runtime::WASM_BINARY
				.expect("WASM binary was not build, please build it!")
				.to_vec(),
		},
		balances: {
			#[allow(unused_mut)]
			let mut balances: Vec<(AccountId, Balance)> =
				endowed_accounts.iter().cloned().map(|k| (k, 1 << 60)).collect();
			cfg_if::cfg_if! {
				if #[cfg(feature = "runtime-benchmarks")] {
					balances.push((benchmark_items::acurast_consumer_account(), 1 << 60));
					balances.push((benchmark_items::acurast_processor_account(), 1 << 60));
				}
			}

			acurast_runtime::BalancesConfig { balances }
		},
		parachain_info: acurast_runtime::ParachainInfoConfig { parachain_id: id },
		collator_selection: acurast_runtime::CollatorSelectionConfig {
			invulnerables: invulnerables.iter().cloned().map(|(acc, _)| acc).collect(),
			candidacy_bond: EXISTENTIAL_DEPOSIT * 16,
			..Default::default()
		},
		session: acurast_runtime::SessionConfig {
			keys: invulnerables
				.into_iter()
				.map(|(acc, aura)| {
					(
						acc.clone(),                 // account id
						acc,                         // validator id
						template_session_keys(aura), // session keys
					)
				})
				.collect(),
		},
		// no need to pass anything to aura, in fact it will panic if we do. Session will take care
		// of this.
		aura: Default::default(),
		aura_ext: Default::default(),
		parachain_system: Default::default(),
		polkadot_xcm: acurast_runtime::PolkadotXcmConfig {
			safe_xcm_version: Some(SAFE_XCM_VERSION),
		},
		sudo: SudoConfig { key: acurast_sudo_account() },
		assets: {
			#[allow(unused_mut)]
			let mut assets = vec![(NATIVE_ASSET_ID, BURN_ACCOUNT, NATIVE_IS_SUFFICIENT, NATIVE_MIN_BALANCE)];
			#[allow(unused_mut)]
			let mut metadata = vec![(
				NATIVE_ASSET_ID,
				NATIVE_TOKEN_NAME.as_bytes().to_vec(),
				NATIVE_TOKEN_SYMBOL.as_bytes().to_vec(),
				token_decimals(),
			)];
			#[allow(unused_mut)]
			let mut accounts = vec![(NATIVE_ASSET_ID, BURN_ACCOUNT, NATIVE_INITIAL_BALANCE)];

			// add assets to run acurast-marketplace benchmarks
			cfg_if::cfg_if! {
				if #[cfg(feature = "runtime-benchmarks")]{
					use benchmark_consts::*;

					assets.push(
						(BENCHMARK_ASSET_ID, acurast_pallet_account(), BENCHMARK_ASSET_IS_SUFFICIENT, BENCHMARK_MIN_BALANCE)

					);
					metadata.push(
						(
							BENCHMARK_ASSET_ID,
							BENCHMARK_TOKEN_NAME.as_bytes().to_vec(),
							BENCHMARK_TOKEN_SYMBOL.as_bytes().to_vec(),
							BENCHMARK_TOKEN_DECIMALS
						)
					);
					accounts.push(
						(BENCHMARK_ASSET_ID, benchmark_items::acurast_consumer_account(), BENCHMARK_INITIAL_BALANCE)
					)
				}
			}

			AssetsConfig { assets, metadata, accounts }
		},
	}
}

const NATIVE_ASSET_ID: u32 = 42;
const NATIVE_IS_SUFFICIENT: bool = true;
const NATIVE_MIN_BALANCE: u128 = 1;
const NATIVE_INITIAL_BALANCE: u128 = 1_000_000_000_000_000;
const NATIVE_TOKEN_NAME: &str = "reserved_native_asset";
const NATIVE_TOKEN_SYMBOL: &str = "RNA";

const BURN_ACCOUNT: sp_runtime::AccountId32 = sp_runtime::AccountId32::new([0u8; 32]);

#[cfg(feature = "runtime-benchmarks")]
pub mod benchmark_consts {
	pub const BENCHMARK_ASSET_ID: u32 = 22;
	pub const BENCHMARK_ASSET_IS_SUFFICIENT: bool = false;
	pub const BENCHMARK_MIN_BALANCE: u128 = 1;
	pub const BENCHMARK_INITIAL_BALANCE: u128 = 1_000_000_000_000_000;
	pub const BENCHMARK_TOKEN_NAME: &str = "benchmark_token";
	pub const BENCHMARK_TOKEN_SYMBOL: &str = "BK";
	pub const BENCHMARK_TOKEN_DECIMALS: u8 = 12;
}

// pallet_acurast Config Type "PalletId" currently defines the account that acts as owner of statemint
// assets minted locally through reserve backed transfers
pub fn acurast_pallet_account() -> sp_runtime::AccountId32 {
	<Runtime as pallet_acurast::Config>::PalletId::get().into_account_truncating()
}
pub fn fee_manager_pallet_account() -> <Runtime as frame_system::Config>::AccountId {
	acurast_runtime::FeeManagerPalletId::get().into_account_truncating()
}
pub fn token_decimals() -> u8 {
	let mut x = acurast_runtime::UNIT as u128;
	let mut decimals = 0;
	while x > 0 {
		x /= 10;
		decimals += 1;
	}
	decimals - 1
}

pub fn acurast_sudo_account() -> Option<<Runtime as frame_system::Config>::AccountId> {
	<Runtime as frame_system::Config>::AccountId::from_str(
		"5CkcmNYgbntGPLi866ouBh1xKNindayyZW3gZcrtUkg7ZqTx",
	)
	.ok()
}
