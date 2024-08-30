use cumulus_primitives_core::ParaId;
use sc_service::ChainType;
use sc_telemetry::serde_json;
use sp_runtime::{app_crypto::Ss58Codec, traits::AccountIdConversion, AccountId32};
use std::str::FromStr;

use crate::chain_spec::{accountid_from_str, Extensions, MAINNET_PARACHAIN_ID, SS58_FORMAT};
pub(crate) use acurast_mainnet_runtime::{self as acurast_runtime, EXISTENTIAL_DEPOSIT};
use acurast_runtime_common::*;

/// Specialized `ChainSpec` for the normal parachain runtime.
pub type ChainSpec =
	sc_service::GenericChainSpec<acurast_runtime::RuntimeGenesisConfig, Extensions>;

/// The default XCM version to set in genesis config.
const SAFE_XCM_VERSION: u32 = xcm::prelude::XCM_VERSION;

const NATIVE_MIN_BALANCE: u128 = 1_000_000_000_000;
const NATIVE_TOKEN_SYMBOL: &str = "cACU";
const NATIVE_TOKEN_DECIMALS: u8 = 12;

/// Generate the session keys from individual elements.
///
/// The input must be a tuple of individual keys (a single arg for now since we have just one key).
pub fn acurast_session_keys(keys: AuraId) -> acurast_runtime::SessionKeys {
	acurast_runtime::SessionKeys { aura: keys }
}

/// Returns the kusama [ChainSpec].
pub fn acurast_config() -> ChainSpec {
	// Give your base currency a unit name and decimal places
	let mut properties = sc_chain_spec::Properties::new();
	properties.insert("tokenSymbol".into(), NATIVE_TOKEN_SYMBOL.into());
	properties.insert("tokenDecimals".into(), NATIVE_TOKEN_DECIMALS.into());
	properties.insert("ss58Format".into(), SS58_FORMAT.into());

	ChainSpec::builder(
		acurast_runtime::WASM_BINARY.expect("WASM binary was not built, please build it!"),
		Extensions { relay_chain: "kusama".to_string(), para_id: MAINNET_PARACHAIN_ID },
	)
	.with_name("Acurast Mainnet")
	.with_id("acurast-mainnet")
	.with_chain_type(ChainType::Live)
	.with_genesis_config_patch(genesis_config(
		vec![
			(
				AccountId32::from_str("5DV9mD4yrswRtrXfH2QxAg5vF23r6FPipxTifkqx6pEnqCRA").unwrap(),
				AuraId::from_string("5DV9mD4yrswRtrXfH2QxAg5vF23r6FPipxTifkqx6pEnqCRA").unwrap(),
			),
			(
				AccountId32::from_str("5DNpDKA9AhsNZn32kd7VgL5kC7h1r5TTQjrHfbgKe4Ck78Z9").unwrap(),
				AuraId::from_string("5DNpDKA9AhsNZn32kd7VgL5kC7h1r5TTQjrHfbgKe4Ck78Z9").unwrap(),
			),
			(
				AccountId32::from_str("5G6zzYZZrokByrohZt1UBY4cYziQHGWQvMQYXXEDh9LSkhRZ").unwrap(),
				AuraId::from_string("5G6zzYZZrokByrohZt1UBY4cYziQHGWQvMQYXXEDh9LSkhRZ").unwrap(),
			),
			(
				AccountId32::from_str("5E5EdKrMArKtXnBW9QZF5MB6uGKVcvJYxbYyAkacdsNgdn7k").unwrap(),
				AuraId::from_string("5E5EdKrMArKtXnBW9QZF5MB6uGKVcvJYxbYyAkacdsNgdn7k").unwrap(),
			),
		],
		vec![
			(acurast_pallet_account(), NATIVE_MIN_BALANCE),
			(fee_manager_pallet_account(), NATIVE_MIN_BALANCE),
			(acurast_sudo_account(), NATIVE_MIN_BALANCE * 9_000_000),
		],
		MAINNET_PARACHAIN_ID.into(),
		acurast_sudo_account(),
	))
	.build()
}

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

/// returns the root account id.
pub fn acurast_sudo_account() -> AccountId {
	accountid_from_str("5HRRaxPnsaCGsbNWCj9dzLcJF2RDFG56VqfAfRt7zYakqTqC")
}
