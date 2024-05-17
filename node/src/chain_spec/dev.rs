use cumulus_primitives_core::ParaId;
use jsonrpsee::core::__reexports::serde_json;
use sc_service::ChainType;
use sp_core::{sr25519, Pair, Public};
use sp_runtime::traits::{AccountIdConversion, IdentifyAccount, Verify};

pub(crate) use acurast_rococo_runtime::{self as acurast_runtime, EXISTENTIAL_DEPOSIT};
use acurast_runtime_common::*;

use crate::chain_spec::{accountid_from_str, Extensions, DEFAULT_PARACHAIN_ID, SS58_FORMAT};

/// Specialized `ChainSpec` for the normal parachain runtime.
pub type ChainSpec =
	sc_service::GenericChainSpec<acurast_runtime::RuntimeGenesisConfig, Extensions>;

/// The default XCM version to set in genesis config.
const SAFE_XCM_VERSION: u32 = xcm::prelude::XCM_VERSION;

/// Helper function to generate a crypto pair from seed
pub fn get_from_seed<TPublic: Public>(seed: &str) -> <TPublic::Pair as Pair>::Public {
	TPublic::Pair::from_string(&format!("//{}", seed), None)
		.expect("static values are valid; qed")
		.public()
}

type AccountPublic = <Signature as Verify>::Signer;

const NATIVE_MIN_BALANCE: u128 = 1_000_000_000_000;
const NATIVE_INITIAL_BALANCE: u128 = 1_000_000_000_000_000;
const NATIVE_TOKEN_SYMBOL: &str = "dACU";
const NATIVE_TOKEN_DECIMALS: u8 = 12;

const FAUCET_INITIAL_BALANCE: u128 = 1_000_000_000_000_000;

/// Generate collator keys from seed.
///
/// This function's return type must always match the session keys of the chain in tuple format.
pub fn get_collator_keys_from_seed(seed: &str) -> AuraId {
	get_from_seed::<AuraId>(seed)
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
pub fn acurast_session_keys(keys: AuraId) -> acurast_runtime::SessionKeys {
	acurast_runtime::SessionKeys { aura: keys }
}

/// Returns the development [ChainSpec].
pub fn acurast_development_config() -> ChainSpec {
	// Give your base currency a unit name and decimal places
	let mut properties = sc_chain_spec::Properties::new();
	properties.insert("tokenSymbol".into(), NATIVE_TOKEN_SYMBOL.into());
	properties.insert("tokenDecimals".into(), NATIVE_TOKEN_DECIMALS.into());
	properties.insert("ss58Format".into(), SS58_FORMAT.into());
	ChainSpec::builder(
		acurast_runtime::WASM_BINARY.expect("WASM binary was not built, please build it!"),
		Extensions { relay_chain: "rococo".to_string(), para_id: DEFAULT_PARACHAIN_ID },
	)
	.with_name("Acurast Devnet")
	.with_id("dev")
	.with_chain_type(ChainType::Development)
	.with_genesis_config_patch(genesis_config(
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
			(acurast_sudo_account(), NATIVE_INITIAL_BALANCE),
		],
		DEFAULT_PARACHAIN_ID.into(),
		acurast_sudo_account(),
	))
	.build()
}

/// Returns the testnet [acurast_runtime::RuntimeGenesisConfig].
fn genesis_config(
	invulnerables: Vec<(AccountId, AuraId)>,
	endowed_accounts: Vec<(AccountId, acurast_runtime::Balance)>,
	id: ParaId,
	sudo_account: AccountId,
) -> serde_json::Value {
	serde_json::json!({
		"balances": {
			"balances": endowed_accounts,
		},
		"parachainInfo": {
			"parachainId": id,
		},
		"collatorSelection": {
			"invulnerables": invulnerables.iter().cloned().map(|(acc, _)| acc).collect::<Vec<_>>(),
			"candidacyBond": EXISTENTIAL_DEPOSIT * 16,
		},
		"session": {
			"keys": invulnerables
				.into_iter()
				.map(|(acc, session_keys)| {
					(
						acc.clone(),                        // account id
						acc,                                // validator id
						acurast_session_keys(session_keys), // session keys
					)
				})
				.collect::<Vec<_>>(),
		},
		"polkadotXcm": {
			"safeXcmVersion": Some(SAFE_XCM_VERSION),
		},
		"sudo": {
			"key": Some(sudo_account)
		}
	})
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

pub fn acurast_sudo_account() -> AccountId {
	accountid_from_str("5DJCQnpbFHnFZHHc5XJGKP1rduYuNaKNe6kAWgRoZc2JXJ5m")
}
