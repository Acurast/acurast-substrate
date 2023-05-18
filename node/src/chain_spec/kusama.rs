use std::str::FromStr;

use crate::chain_spec::{Extensions, KUSAMA_PARACHAIN_ID};
use acurast_common::*;
pub(crate) use acurast_kusama_runtime::{
	self as acurast_runtime, AcurastAssetsConfig, AcurastConfig, AcurastProcessorManagerConfig,
	AssetsConfig, DemocracyConfig, SudoConfig, EXISTENTIAL_DEPOSIT,
};
use cumulus_primitives_core::ParaId;
use sc_service::ChainType;
use sp_runtime::{app_crypto::Ss58Codec, traits::AccountIdConversion, AccountId32};

/// Specialized `ChainSpec` for the normal parachain runtime.
pub type ChainSpec = sc_service::GenericChainSpec<acurast_runtime::GenesisConfig, Extensions>;

/// The default XCM version to set in genesis config.
const SAFE_XCM_VERSION: u32 = xcm::prelude::XCM_VERSION;

const NATIVE_IS_SUFFICIENT: bool = true;
const NATIVE_MIN_BALANCE: u128 = 1_000_000_000_000;
const NATIVE_INITIAL_BALANCE: u128 = 1_000_000_000_000_000;
const NATIVE_TOKEN_NAME: &str = "reserved_native_asset";
const NATIVE_TOKEN_SYMBOL: &str = "ACRST";
const NATIVE_TOKEN_DECIMALS: u8 = 12;
const BURN_ACCOUNT: sp_runtime::AccountId32 = sp_runtime::AccountId32::new([0u8; 32]);

/// Generate the session keys from individual elements.
///
/// The input must be a tuple of individual keys (a single arg for now since we have just one key).
pub fn acurast_session_keys(keys: AuraId) -> acurast_runtime::SessionKeys {
	acurast_runtime::SessionKeys { aura: keys }
}

/// Returns the kusama [ChainSpec].
pub fn acurast_kusama_config() -> ChainSpec {
	// Give your base currency a unit name and decimal places
	let mut properties = sc_chain_spec::Properties::new();
	properties.insert("tokenSymbol".into(), NATIVE_TOKEN_SYMBOL.into());
	properties.insert("tokenDecimals".into(), NATIVE_TOKEN_DECIMALS.into());
	properties.insert("ss58Format".into(), 42.into());

	ChainSpec::from_genesis(
		// Name
		"Acurast Kusama Testnet",
		// ID
		"acurast-kusama",
		ChainType::Live,
		move || {
			genesis_config(
				// initial collators.
				vec![
					(
						AccountId32::from_str("5G3ofXWgdH2fZZuYKgzTJMfDZLb9yNbiSuGCRQGKVBNgZXJi")
							.unwrap(),
						AuraId::from_string("5G3ofXWgdH2fZZuYKgzTJMfDZLb9yNbiSuGCRQGKVBNgZXJi")
							.unwrap(),
					),
					(
						AccountId32::from_str("5DAi7w3otvntMWvRLCWgorKMv4dpPvvU7jkZcrKxHpjWg6X7")
							.unwrap(),
						AuraId::from_string("5DAi7w3otvntMWvRLCWgorKMv4dpPvvU7jkZcrKxHpjWg6X7")
							.unwrap(),
					),
				],
				vec![
					(acurast_pallet_account(), NATIVE_MIN_BALANCE),
					(fee_manager_pallet_account(), NATIVE_MIN_BALANCE),
					(acurast_sudo_account(), acurast_runtime::Balance::MAX),
				],
				KUSAMA_PARACHAIN_ID.into(),
				acurast_sudo_account(),
				AcurastConfig { attestations: vec![] },
				AssetsConfig { assets: vec![], metadata: vec![], accounts: vec![] },
			)
		},
		Vec::new(),
		None,
		None,
		None,
		Some(properties),
		Extensions {
			relay_chain: "kusama".into(), // You MUST set this to the correct network!
			para_id: KUSAMA_PARACHAIN_ID,
		},
	)
}

/// Returns the testnet [acurast_runtime::GenesisConfig].
fn genesis_config(
	invulnerables: Vec<(AccountId, AuraId)>,
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
				.into_iter()
				.map(|(acc, aura)| {
					(
						acc.clone(),                // account id
						acc,                        // validator id
						acurast_session_keys(aura), // session keys
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

/// returns the root account id.
pub fn acurast_sudo_account() -> AccountId {
	accountid_from_str("5CkcmNYgbntGPLi866ouBh1xKNindayyZW3gZcrtUkg7ZqTx")
}

pub fn accountid_from_str(account: &str) -> AccountId {
	AccountId::from_str(account).expect("valid account id")
}

fn acurast_processor_manager_config() -> AcurastProcessorManagerConfig {
	AcurastProcessorManagerConfig {
		managers: vec![
			// (Manager ; Processors)
			(accountid_from_str("5FYkyB3FXzZBrRyYiHytHFuuyqCLU19vjc9NtdcfYAKAjLfe"), vec![]),
			(
				accountid_from_str("5FYzJ2GUxaAm7Z4be2ChoVcRrmga41A796h6sKTSdCkwrUb5"),
				vec![
					accountid_from_str("5DTkrid3L1h17EZaK8im6jzYhr9qKihXLqFCKEGgi2ckW3DQ"),
					accountid_from_str("5FyBCnjhJiT9zbVTbCGxtaKiKJCsbwLUbKdmvijgmQn1ensg"),
					accountid_from_str("5DpyWfnyQxoWnsSnkuwgsveoD8VAtrw56MiXHTZzRJ2Wz26A"),
				],
			),
			(
				accountid_from_str("5FcBRwWsXV5GQyjqZh5GoMPq6xhcn565mMf6CF4sgiHTpJjr"),
				vec![
					accountid_from_str("5EgAey9fPeiH2YHEDwx92Eyp3GvkVY1VRRywWGKfj6RLYw1i"),
					accountid_from_str("5CYuPXk3qbqJrpU4jH12sax3PuY2ZsWWa3zuZRA3Ux4D2xn4"),
					accountid_from_str("5DAG5twcsCtastsSySioSgDfxkN42SFUynwoMCWgoueTwwfE"),
					accountid_from_str("5DAkNDg7jQTaUR8nJvxXze2a8kX2x6yCzGNE3gXFcpSJq77W"),
					accountid_from_str("5Dh3mVsaLtqaEzG7gkqwKSVZG1UkuB4ZufLJ1CA6mdroRam8"),
				],
			),
			(
				accountid_from_str("5HGxvRPw2VWDUqSzWB9XbcBPuRUYkSWwX95XnUyypuGbx6Nh"),
				vec![accountid_from_str("5Ft3xhBmUHgMe4WgKoX2pi6eh6PAoipWHm1fShLFFXjT9Dgj")],
			),
			(
				accountid_from_str("5G6ieicxdNkEMq62NV5hnGRMvcWa5hnVotEahyq2ujAeJDZ5"),
				vec![
					accountid_from_str("5HVRFnYUjXMBjUgKN5WhbHg4gDAnaqec4kHWYBeQk4imsPrr"),
					accountid_from_str("5DUMW4VGx1FPUNfpohbWRhECha79Jd1kCPJaLfMephMA3BDr"),
					accountid_from_str("5DaqqBkvzYVc7Yo8gq3uL9NAzwcYZAUVK8LnZ4N6vcJrALED"),
				],
			),
			(
				accountid_from_str("5FYdfpJ16SydCqzCfx2noYnVRDU7YfCaPcxss6LzXerVacti"),
				vec![accountid_from_str("5DKt7Y13qVMfr7cj2FNLZVoiLyg6Hd9RqcrFY2d1uFgHLhS8")],
			),
			(
				accountid_from_str("5Co1Z2138wBguJ2hunK7tN4bFka7hTjHxhhzm1NnCME79Vw1"), // Rodrigo
				vec![
					accountid_from_str("5Ed1yyorcV7PcfbFTQaykvnU9PHfB3MtWDKQPjrki8HsrZU8"),
					accountid_from_str("5DKpr44rhYQfEToY8uC2wdiFsbAPh62DmmMubTboQDb34r15"),
					accountid_from_str("5Ft4n84yRnmUawREwRg8q9scWLUcCyT9H2FDqoNBiwLpQ685"),
				],
			),
			(
				accountid_from_str("5DvEVDzz4tBTdZ5FYjFfJs6EHj3HfuNKaP1cdJdESVyojK6n"),
				vec![
					accountid_from_str("5EC55EhW1vgiVYyDxQtgEMB97n5cK4boLoyEJGzULKBgdCNY"),
					accountid_from_str("5DJUMJFGCCotUb6syceZ2QR6BH6hzudJKQ7AWVyPxgv75AAt"),
				],
			),
			(
				accountid_from_str("5DFfQ6pkBReCMJvR2MMvqYWeqLWuhoBGbcwZ3hkEe4q46BsR"),
				vec![accountid_from_str("5Ea2DdmZy9JWc7zN8LsQVt6z1As1CUXXyvmLbVWsT2SHxX9V")],
			),
			(
				accountid_from_str("5GuKNzuMDDFtTuyKz1Q7z4dhBEoaJrPy7tKdqq3yujTHrFjP"),
				vec![accountid_from_str("5G1hNcarzBZSz55Eom5E55WCFHe8MnJGsRefDPnr8L68MZCG")],
			),
			(
				accountid_from_str("5GNvjrPEQjxfmVrt7muM7DBnb6AcRJWkkiXtuK9oTi5ZUR6T"),
				vec![accountid_from_str("5HUA8S1gywbA9qgvpYMeJk444TwmkrhMFS4youcz9uKqBcMb")],
			),
			(
				accountid_from_str("5Cr5dcF1GhGWoBSnkMuXfbccmdG1yqXR1rw5AjXJmNHLQmd7"),
				vec![accountid_from_str("5EtVzJiZpMhbNEvBsawFhioe9rD2pLAn9t3GbNrqgd8XdqBK")],
			),
		],
	}
}
