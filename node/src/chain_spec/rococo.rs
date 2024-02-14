use cumulus_primitives_core::ParaId;
use sc_service::ChainType;
use sp_runtime::{app_crypto::Ss58Codec, traits::AccountIdConversion, AccountId32, Percent};
use std::str::FromStr;

pub(crate) use acurast_rococo_runtime::{
	self as acurast_runtime, AcurastConfig, AcurastProcessorManagerConfig, AcurastVestingConfig,
	DemocracyConfig, Runtime, SudoConfig, VestingFor, EXISTENTIAL_DEPOSIT,
};
use acurast_runtime_common::*;

use crate::chain_spec::{accountid_from_str, processor_manager, Extensions, ROCOCO_PARACHAIN_ID};

/// Specialized `ChainSpec` for the normal parachain runtime.
pub type ChainSpec =
	sc_service::GenericChainSpec<acurast_runtime::RuntimeGenesisConfig, Extensions>;

/// The default XCM version to set in genesis config.
const SAFE_XCM_VERSION: u32 = xcm::prelude::XCM_VERSION;

const NATIVE_MIN_BALANCE: u128 = 1_000_000_000_000;
const NATIVE_INITIAL_BALANCE: u128 = 1_000_000_000_000_000;
const NATIVE_TOKEN_SYMBOL: &str = "ACRST";
const NATIVE_TOKEN_DECIMALS: u8 = 12;

const FAUCET_INITIAL_BALANCE: u128 = 1_000_000_000_000_000;

/// Generate the session keys from individual elements.
///
/// The input must be a tuple of individual keys (a single arg for now since we have just one key).
pub fn acurast_session_keys(keys: AuraId) -> acurast_runtime::SessionKeys {
	acurast_runtime::SessionKeys { aura: keys }
}

/// Returns the rococo [ChainSpec].
pub fn acurast_rococo_config() -> ChainSpec {
	// Give your base currency a unit name and decimal places
	let mut properties = sc_chain_spec::Properties::new();
	properties.insert("tokenSymbol".into(), NATIVE_TOKEN_SYMBOL.into());
	properties.insert("tokenDecimals".into(), NATIVE_TOKEN_DECIMALS.into());
	properties.insert("ss58Format".into(), 42.into());

	ChainSpec::from_genesis(
		// Name
		"Acurast Rococo Testnet",
		// ID
		"acurast-rococo",
		ChainType::Live,
		move || {
			genesis_config(
				// initial collators.
				vec![
					(
						AccountId32::from_str("5D592NKdEvudZ34Tad9Psb4fhTUA8gRnHZ9aZMWS9HjR754f")
							.unwrap(),
						AuraId::from_string("5D592NKdEvudZ34Tad9Psb4fhTUA8gRnHZ9aZMWS9HjR754f")
							.unwrap(),
					),
					(
						AccountId32::from_str("5CyfKHo81NTwbpbLVXCBN3dc7s9LVCdz59NW44LnzhkwvS58")
							.unwrap(),
						AuraId::from_string("5CyfKHo81NTwbpbLVXCBN3dc7s9LVCdz59NW44LnzhkwvS58")
							.unwrap(),
					),
				],
				vec![
					(acurast_pallet_account(), NATIVE_MIN_BALANCE),
					(fee_manager_pallet_account(), NATIVE_MIN_BALANCE),
					(acurast_sudo_account(), NATIVE_INITIAL_BALANCE),
					(acurast_faucet_account(), FAUCET_INITIAL_BALANCE),
				],
				ROCOCO_PARACHAIN_ID.into(),
				acurast_sudo_account(),
				AcurastConfig { attestations: vec![] },
			)
		},
		// Bootnodes
		Vec::new(),
		// Telemetry
		None,
		// Protocol ID
		None,
		// Fork ID
		None,
		// Properties
		Some(properties),
		// Extensions
		Extensions {
			relay_chain: "rococo".into(), // You MUST set this to the correct network!
			para_id: ROCOCO_PARACHAIN_ID,
		},
	)
}

/// Returns the testnet [acurast_runtime::RuntimeGenesisConfig].
fn genesis_config(
	invulnerables: Vec<(AccountId, AuraId)>,
	endowed_accounts: Vec<(AccountId, acurast_runtime::Balance)>,
	id: ParaId,
	sudo_account: AccountId,
	acurast: AcurastConfig,
) -> acurast_runtime::RuntimeGenesisConfig {
	acurast_runtime::RuntimeGenesisConfig {
		system: acurast_runtime::SystemConfig {
			code: acurast_runtime::WASM_BINARY
				.expect("WASM binary was not build, please build it!")
				.to_vec(),
			..Default::default()
		},
		balances: acurast_runtime::BalancesConfig { balances: endowed_accounts },
		parachain_info: acurast_runtime::ParachainInfoConfig {
			parachain_id: id,
			..Default::default()
		},
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
		// no need to pass anything to aura, in fact it will panic if we do. Session will take care
		// of this.
		aura: Default::default(),
		aura_ext: Default::default(),
		parachain_system: Default::default(),
		parachain_staking: acurast_runtime::ParachainStakingConfig {
			blocks_per_round: 3600u32.into(), // 3600 * ~12s = ~12h (TBD)
			collator_commission: Perbill::from_percent(20), // TBD
			num_selected_candidates: 128u32.into(),
			parachain_bond_reserve_percent: Percent::from_percent(30), // TBD
			candidates: invulnerables
				.iter()
				.cloned()
				.map(|(acc, _)| (acc, staking_info::MINIMUM_COLLATOR_STAKE))
				.collect(),
			delegations: vec![],
			inflation_config: staking_info::DEFAULT_INFLATION_CONFIG,
		},
		acurast_vesting: AcurastVestingConfig {
			vesters: invulnerables
				.into_iter()
				.map(|(acc, _)| {
					(
						acc,
						VestingFor::<Runtime, _> {
							stake: staking_info::MINIMUM_COLLATOR_STAKE,
							// ~ 1 month
							locking_period: 262144,
						},
					)
				})
				.collect(),
		},
		polkadot_xcm: acurast_runtime::PolkadotXcmConfig {
			safe_xcm_version: Some(SAFE_XCM_VERSION),
			..Default::default()
		},
		sudo: SudoConfig { key: Some(sudo_account) },
		acurast,
		acurast_processor_manager: acurast_processor_manager_config(),
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

/// returns the root account id.
pub fn acurast_sudo_account() -> AccountId {
	accountid_from_str("5CkcmNYgbntGPLi866ouBh1xKNindayyZW3gZcrtUkg7ZqTx")
}

/// returns the faucet account id.
pub fn acurast_faucet_account() -> AccountId {
	accountid_from_str("5EyaQQEQzzXdfsvFfscDaQUFiGBk5hX4B38j1x3rH7Zko2QJ")
}

fn acurast_processor_manager_config() -> AcurastProcessorManagerConfig {
	AcurastProcessorManagerConfig { managers: processor_manager() }
}
