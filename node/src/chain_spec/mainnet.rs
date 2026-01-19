use acurast_runtime_common::{
	constants::{DAYS, UNIT},
	types::{AccountId, AuraId, Balance, BlockNumber},
};
use cumulus_primitives_core::ParaId;
use sc_service::ChainType;
use sc_telemetry::serde_json;
use sp_runtime::{app_crypto::Ss58Codec, traits::AccountIdConversion, AccountId32};
use std::str::FromStr;

use super::{accountid_from_str, ChainSpec, Extensions, MAINNET_PARACHAIN_ID, SS58_FORMAT};
pub(crate) use acurast_mainnet_runtime::{
	self as acurast_runtime, HyperdriveIbcFeePalletAccount, HyperdriveTokenEthereumFeeVault,
	HyperdriveTokenEthereumVault, HyperdriveTokenPalletAccount, HyperdriveTokenSolanaFeeVault,
	HyperdriveTokenSolanaVault, EXISTENTIAL_DEPOSIT,
};

/// The default XCM version to set in genesis config.
const SAFE_XCM_VERSION: u32 = xcm::prelude::XCM_VERSION;

const NATIVE_TOKEN_SYMBOL: &str = "ACU";
const NATIVE_TOKEN_DECIMALS: u8 = 12;

const INITIAL_TOTAL_SUPPLY: Balance = 1_000_000_000 * UNIT;
const VESTING_START: BlockNumber = 527_410;
const VESTING_START_6_MONTHS: BlockNumber = VESTING_START + (6 * MONTH);
const VESTING_INITIAL_LIQUIDITY: Balance = UNIT;
const MONTH: BlockNumber = DAYS * 30;
const VESTING_PERIOD_1: BlockNumber = 24 * MONTH;
const VESTING_PERIOD_2: BlockNumber = 36 * MONTH;
const VESTING_PERIOD_3: BlockNumber = 12 * MONTH;

/// Generate the session keys from individual elements.
///
/// The input must be a tuple of individual keys (a single arg for now since we have just one key).
pub fn acurast_session_keys(keys: AuraId) -> acurast_runtime::SessionKeys {
	acurast_runtime::SessionKeys { aura: keys }
}

/// Returns the mainnet [ChainSpec].
pub fn acurast_config() -> ChainSpec {
	// Give your base currency a unit name and decimal places
	let mut properties = sc_chain_spec::Properties::new();
	properties.insert("tokenSymbol".into(), NATIVE_TOKEN_SYMBOL.into());
	properties.insert("tokenDecimals".into(), NATIVE_TOKEN_DECIMALS.into());
	properties.insert("ss58Format".into(), SS58_FORMAT.into());

	let computed_total_supply: Balance =
		endowed_accounts().iter().fold(0, |current, (_, amount)| current + *amount)
			+ erc20_allocations().iter().sum::<Balance>();
	assert!(computed_total_supply <= INITIAL_TOTAL_SUPPLY);

	ChainSpec::builder(
		acurast_runtime::WASM_BINARY.expect("WASM binary was not built, please build it!"),
		Extensions { relay_chain: "polkadot".to_string(), para_id: MAINNET_PARACHAIN_ID },
	)
	.with_name("Acurast Mainnet")
	.with_id("acurast-mainnet")
	.with_chain_type(ChainType::Live)
	.with_properties(properties)
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
		endowed_accounts(),
		vesting(),
		MAINNET_PARACHAIN_ID.into(),
	))
	.build()
}

fn genesis_config(
	invulnerables: Vec<(AccountId, AuraId)>,
	endowed_accounts: Vec<(AccountId, Balance)>,
	vesting: Vec<(AccountId, BlockNumber, BlockNumber, Balance)>,
	id: ParaId,
) -> serde_json::Value {
	serde_json::json!({
		"balances": {
			"balances": endowed_accounts,
		},
		"vesting": {
			"vesting": vesting,
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
		"acurastHyperdriveToken": {
			"initialEthTokenAllocation": Some(erc20_allocations().iter().sum::<Balance>()),
		},
		"acurastProcessorManager": {
			"apiVersion": 1,
			"onboardingSettings": {
				"funds": 100_000_000_000u128,
				"max_funds": 300_000_000_000u128,
				"funds_account": onboarding_funds_account(),
			}
		},
		"councilMembership": {
			"members": council_members()
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

pub fn compute_pallet_account() -> AccountId {
	acurast_runtime::ComputePalletId::get().into_account_truncating()
}

pub fn token_conversion_pallet_account() -> AccountId {
	acurast_runtime::AcurastTokenConversion::account_id()
}

pub fn treasury_pallet_account() -> AccountId {
	acurast_runtime::Treasury::account_id()
}

pub fn operation_funds_pallet_account() -> AccountId {
	acurast_runtime::OperationFunds::account_id()
}

pub fn liquidity_funds_pallet_account() -> AccountId {
	acurast_runtime::LiquidityFunds::account_id()
}

pub fn extra_funds_pallet_account() -> AccountId {
	acurast_runtime::ExtraFunds::account_id()
}

pub fn onboarding_funds_account() -> AccountId {
	accountid_from_str("5EAFqBNRWhe93pXvvkXB1oBHe15btTyw6vy21eGtwRqXjFLz")
}

pub fn matcher_accounts() -> Vec<AccountId> {
	vec![
		accountid_from_str("5Hp3hUqsTd9SfKksAUNLcUYqbSrVnumvGyC7Eop9u895EuWK"),
		accountid_from_str("5GgLEWXHT5iiiaKBMiJL9jauVP3utihKU8wwtJLtZBqKyre1"),
	]
}

pub fn hyperdrive_accounts() -> Vec<AccountId> {
	vec![
		HyperdriveTokenEthereumFeeVault::get(),
		HyperdriveTokenEthereumVault::get(),
		HyperdriveTokenSolanaFeeVault::get(),
		HyperdriveTokenSolanaVault::get(),
		HyperdriveIbcFeePalletAccount::get(),
	]
}

/// returns the root account id.
pub fn acurast_sudo_account() -> AccountId {
	accountid_from_str("5HRRaxPnsaCGsbNWCj9dzLcJF2RDFG56VqfAfRt7zYakqTqC")
}

fn council_members() -> Vec<AccountId> {
	vec![
		accountid_from_str("5CGV1Sm6Qzt3s5qabiDAEjni6xT15MZ8LumkVPob4SJqAN7C"),
		accountid_from_str("5DFhdYCuTc4uubFu6XGpiF5uKu6e7erNZa6QKExZDRFMTuv8"),
		accountid_from_str("5DXDTbjLtDDUXzFy24Fhkjs9fY3PQwQR2ohzjQPT1JvUAcEy"),
		accountid_from_str("5EUnFHHEFd4mzTA6cjg8JfKHeteCDrcEhMdxUXSK3QzHSPe8"),
		accountid_from_str("5EJSc1seHmJhvC45UMtgg1vNThMhdURj4VY8qCiXQtJcTQrE"),
		accountid_from_str("5HL5KrhemuUu3fmDKunRZKLkGfWukq55WJMbct6qEcGvMuoM"),
		accountid_from_str("5EFpch6M9U9dmWQV427dvgwUQeAjVowFDZefhcyXJqjqT3si"),
	]
}

pub fn endowed_accounts() -> Vec<(AccountId, Balance)> {
	let mut res = initial_balances();
	res.extend_from_slice(
		allocations()
			.into_iter()
			.map(|(account, _, _, balance)| (account, balance))
			.collect::<Vec<_>>()
			.as_slice(),
	);
	// 69
	res.push((
		accountid_from_str("5EhTHsee9Xtj89kXAnK9PxzEssXjD32evv95GtP8i68cR2sg"),
		10_000_000 * UNIT,
	));
	// 70
	res.push((token_conversion_pallet_account(), (65_000_000 * UNIT) + EXISTENTIAL_DEPOSIT));
	// 71
	res.push((extra_funds_pallet_account(), 100_000_000 * UNIT));
	// 78
	res.push((liquidity_funds_pallet_account(), 100_000_000 * UNIT));

	res
}

pub fn vesting() -> Vec<(AccountId, BlockNumber, BlockNumber, Balance)> {
	allocations()
		.into_iter()
		.map(|(account, begin, length, _)| (account, begin, length, VESTING_INITIAL_LIQUIDITY))
		.collect::<Vec<_>>()
}

pub fn initial_balances() -> Vec<(AccountId, Balance)> {
	let mut balances = vec![
		(acurast_pallet_account(), EXISTENTIAL_DEPOSIT),
		(fee_manager_pallet_account(), EXISTENTIAL_DEPOSIT),
		(compute_pallet_account(), EXISTENTIAL_DEPOSIT),
		(HyperdriveTokenPalletAccount::get(), UNIT),
		(acurast_sudo_account(), 1000 * UNIT),
		(onboarding_funds_account(), 3000 * UNIT),
	];
	balances.extend_from_slice(
		hyperdrive_accounts()
			.into_iter()
			.map(|account| (account, EXISTENTIAL_DEPOSIT))
			.collect::<Vec<_>>()
			.as_slice(),
	);
	balances.extend_from_slice(
		council_members()
			.into_iter()
			.map(|m| (m, 100 * UNIT))
			.collect::<Vec<_>>()
			.as_slice(),
	);
	balances.extend_from_slice(
		matcher_accounts()
			.into_iter()
			.map(|account| (account, 5 * UNIT))
			.collect::<Vec<_>>()
			.as_slice(),
	);

	balances
}

pub fn allocations() -> Vec<(AccountId, BlockNumber, BlockNumber, Balance)> {
	use super::accountid_from_str as aid;
	let mut res = vec![
		// 9
		(
			aid("5FugRZGYAYWiAxq2nLMFnbLauj4hdz9dUo8xeALAEkHznSqk"),
			VESTING_START,
			VESTING_PERIOD_1,
			12_500_000 * UNIT,
		),
		// 10
		(
			aid("5E6qqTqHTzhuNzEJprpAc7dyDLYD6LqjCPHcHFv6dE3iGmfp"),
			VESTING_START,
			VESTING_PERIOD_3,
			7_500_000 * UNIT,
		),
		// 12 5_000_000
		(
			aid("5ECwUn4m91JcHjduZLXGptDfxJMTX7bvDVgsjs74DY5qiv6y"),
			VESTING_START,
			VESTING_PERIOD_1,
			2_000_000 * UNIT,
		),
		(
			aid("5Dq1qDYmyTSWvCPShZukBo8c3BK3vVzpdvuk4k5UJo5UsgEs"),
			VESTING_START,
			VESTING_PERIOD_1,
			2_000_000 * UNIT,
		),
		(
			aid("5Fgxkwm1i5A6kbnez9seVxsU8BabWo1nRmbu86y3GrbZDwWa"),
			VESTING_START,
			VESTING_PERIOD_1,
			1_000_000 * UNIT,
		),
		// 13 3_750_000
		(
			aid("5DZsD9hQyw7CnPV1NwvGouKbNFfrn7CzFycZ5umadufg4ats"),
			VESTING_START,
			VESTING_PERIOD_1,
			1_250_000 * UNIT,
		),
		(
			aid("5GemdJPH2EQsBZ6F6xKfBV7f7T1ZNNfLGQujpi2gkoK8W9KL"),
			VESTING_START,
			VESTING_PERIOD_1,
			750_000 * UNIT,
		),
		(
			aid("5EF6Ls4eGHaNmo94VhSoNneyPgsHVuBS3sHyzVJQewCKcM3o"),
			VESTING_START,
			VESTING_PERIOD_1,
			250_000 * UNIT,
		),
		(
			aid("5CobCEgbJ5bMydqNq5YskbBGpxitMTcYMe4tsTs2SVdQ4khy"),
			VESTING_START,
			VESTING_PERIOD_1,
			250_000 * UNIT,
		),
		(
			aid("5CDfhYpEFXYdp5GBj7CRSN3pwde5kwtxqLFUYoXZBu1HVMsN"),
			VESTING_START,
			VESTING_PERIOD_1,
			250_000 * UNIT,
		),
		(
			aid("5Cz258Uxak8cZTNm1Zwy9WWeMwc4A5HpqGe8hroBsNxU5ucD"),
			VESTING_START,
			VESTING_PERIOD_1,
			250_000 * UNIT,
		),
		(
			aid("5FUTdoeeeWdEFhWefa6k5cyX2EexgCbeKcQwcTE9uKctFsTR"),
			VESTING_START,
			VESTING_PERIOD_1,
			250_000 * UNIT,
		),
		(
			aid("5CG3UhU2gjP32E6uzRRDebnHHbLmJRudMf8bpdETXbEaW72A"),
			VESTING_START,
			VESTING_PERIOD_1,
			250_000 * UNIT,
		),
		(
			aid("5FqcQTCT5EqmcXyH1jDWebcRp4FD1AhAhGnHw8CJdZu5xHZR"),
			VESTING_START,
			VESTING_PERIOD_1,
			250_000 * UNIT,
		),
		// 15
		(
			aid("5FnKWNkVW1zfyMJfHah4prpCZV5T8iuHAQggYjCEED7q7zYA"),
			VESTING_START,
			VESTING_PERIOD_1,
			2_975_000 * UNIT,
		),
		// 16
		(
			aid("5HmJsFDQFhoPWjmDJxvUSgYfwPGvUkk5AV1798QrcNZEVscv"),
			VESTING_START,
			VESTING_PERIOD_1,
			2_500_000 * UNIT,
		),
		// 17
		(
			aid("5Houib3TFsdf32WQ5q7tkBLY9nDNXmMKoRwJoRnABdAuHUNw"),
			VESTING_START,
			VESTING_PERIOD_1,
			1_500_000 * UNIT,
		),
		// 18
		(
			aid("5FpqN7Nbrocr52XbbzmSjMQeHxtzrYhVeXDsu8d8KSqZ8CS4"),
			VESTING_START,
			VESTING_PERIOD_1,
			2_083_330 * UNIT,
		),
		// 19
		(
			aid("5DhN5RKJiCGmkburNX7nuar8CUUiXYXmhoNvDrrgF1UqfmRJ"),
			VESTING_START,
			VESTING_PERIOD_1,
			4_050_170 * UNIT,
		),
		// 20
		(
			aid("5FL5AykCFECwHPA8f64anz3makk8fL4434kJ6nS7TVUtnaK5"),
			VESTING_START,
			VESTING_PERIOD_1,
			1_250_000 * UNIT,
		),
		// 21
		(
			aid("5D9xc9nkM6YUZ2UipNZj4qSYSrAfHrmU5QSRc7wxMq7vd9CD"),
			VESTING_START,
			VESTING_PERIOD_1,
			1_250_000 * UNIT,
		),
		// 22
		(
			aid("5CiVxaRa22jD6THmb6sgDTs4Chkv4qMeuLMugHC13xJSdhjH"),
			VESTING_START,
			VESTING_PERIOD_1,
			1_250_000 * UNIT,
		),
		// 23 538_460
		(
			aid("5G6BRksZH85d3ivCM3heKSbKHoYiWvTZURYJKohvSmM83wNT"),
			VESTING_START,
			VESTING_PERIOD_1,
			358_973 * UNIT,
		),
		(
			aid("5CaDPpjzx9hS71oyYApae6ND9GVtY1tiBHGu6xpMoBhquQy2"),
			VESTING_START,
			VESTING_PERIOD_1,
			179_487 * UNIT,
		),
		// 24
		(
			aid("5HSzZEEFg4gKTEmkTJRLSBSk3byQaSqHusXqaLi5K9hT5iUj"),
			VESTING_START,
			VESTING_PERIOD_1,
			1_000_000 * UNIT,
		),
		// 25
		(
			aid("5H9EaE3rKZir6EG1FjNuavGtotWPJW2B9EoAbMNuvofbM63F"),
			VESTING_START,
			VESTING_PERIOD_1,
			769_230 * UNIT,
		),
		// 26
		(
			aid("5CKvq5zYM44y5mAqKygyhgsjKnS4FhWELFM8kTEEvUSig9RK"),
			VESTING_START,
			VESTING_PERIOD_1,
			2_307_000 * UNIT,
		),
		// 27
		(
			aid("5H9485UgortAPPQk5cajqCmTDkjg37Kqnnhv9JcsXr8FRSRD"),
			VESTING_START,
			VESTING_PERIOD_1,
			1_923_000 * UNIT,
		),
		// 28
		(
			aid("5H3qUV29BHiEmLWmwJvk5RKpE4wEeL6PoK4A37Fe1dNDabsy"),
			VESTING_START,
			VESTING_PERIOD_1,
			762_930 * UNIT,
		),
		// 29
		(
			aid("5CiiZwUvneuJU7WE2iGgpzMoPWpKsMk4UBoRT85ygKbdXkvH"),
			VESTING_START,
			VESTING_PERIOD_1,
			153_800 * UNIT,
		),
		// 30
		(
			aid("5Hg6A2a5w9CFmivBwiAg1XNHB6MBGcciDEmTyhVatezYS7WS"),
			VESTING_START,
			VESTING_PERIOD_1,
			111_111 * UNIT,
		),
		// 31
		(
			aid("5ChRhVnzx6krEqJC6nushfiRrxdekNUbVwKsDkhEygNhjCzc"),
			VESTING_START,
			VESTING_PERIOD_1,
			30_769 * UNIT,
		),
		// 32
		(
			aid("5GR1Dixbr66EdacvzcpiKmLkK8p56Zyou2sVhcMBSJQgLhaj"),
			VESTING_START,
			VESTING_PERIOD_1,
			555_560 * UNIT,
		),
		// ------------------------------------------------------------//
		// 41
		(
			aid("5GbrNFVC3QrmCbwhURDDVmU4LMUDKaAhTk2zXytdXdJtAkCT"),
			VESTING_START_6_MONTHS,
			VESTING_PERIOD_2,
			8_770_000 * UNIT,
		),
		// 42
		(
			aid("5Ci4ZwigxqhoXpmHTRmdHZWVvzJBqxHabHTixCJXipeJpEgt"),
			VESTING_START_6_MONTHS,
			VESTING_PERIOD_2,
			858_333 * UNIT,
		),
		// 43
		(
			aid("5DFMRy8wEPF8ztjMFrXcwg1cHiGJ5wAthSDBXCiJGh9C5oq3"),
			VESTING_START_6_MONTHS,
			VESTING_PERIOD_2,
			769_231 * UNIT,
		),
		// 44
		(
			aid("5CCodUNXSmRHDjBtjUPyCRGERwGpB7z6vpNgABqVSEPmCD6f"),
			VESTING_START_6_MONTHS,
			VESTING_PERIOD_2,
			600_000 * UNIT,
		),
		// 45
		(
			aid("5CUiS5rUKPJviERreb7ckqqU26hUjcMRvc8DuQpqjKwLhQNq"),
			VESTING_START_6_MONTHS,
			VESTING_PERIOD_2,
			600_000 * UNIT,
		),
		// 46
		(
			aid("5HQAkXwq7y7QU6A9GV1o5Y4t3cimRGC5gKnmHDo6C3zKyuAk"),
			VESTING_START_6_MONTHS,
			VESTING_PERIOD_2,
			600_000 * UNIT,
		),
		// 47
		(
			aid("5HC27HXUELANZU2QPe6Wf7EFjwsxbJABiUxK2uwXxegHgXwJ"),
			VESTING_START_6_MONTHS,
			VESTING_PERIOD_2,
			600_000 * UNIT,
		),
		// 48
		(
			aid("5GH41K1xNow4kANzxhuZgvDYWyv4VANDiEHYqaf2HYHaDAbQ"),
			VESTING_START_6_MONTHS,
			VESTING_PERIOD_2,
			1_415_676 * UNIT,
		),
		// 49
		(
			aid("5HXREMkpDD38wF2KpCT8aQsq3HdnVn1JJirUxx59CSw6uZd2"),
			VESTING_START_6_MONTHS,
			VESTING_PERIOD_2,
			769_200 * UNIT,
		),
		// 50
		(
			aid("5EZZnVytkCfFrc1xCUr9jabsT2xByBJgPk3QddcyscAPfknw"),
			VESTING_START_6_MONTHS,
			VESTING_PERIOD_2,
			600_000 * UNIT,
		),
		// ------------------------------------------------------------//
		// 54 1_301_833
		(
			aid("5EcJuXSGS9k2eQrjgoSsNXHTLjTSPWnqT75hjG6c3mz6xe21"),
			VESTING_START_6_MONTHS,
			VESTING_PERIOD_2,
			1_000_000 * UNIT,
		),
		(
			aid("5DkLKA3nRFUsLfKgoknH6oFJCBJtrLZALzt2gCo6yYUNMR2c"),
			VESTING_START_6_MONTHS,
			VESTING_PERIOD_2,
			301_833 * UNIT,
		),
		// 55
		(
			aid("5ExgdfQyutnAeqvB6kkD9xh7dYQ414dYkCEKfcJM78bC9nCv"),
			VESTING_START_6_MONTHS,
			VESTING_PERIOD_2,
			1_079_569 * UNIT,
		),
		// 56
		(
			aid("5H8mFZDW4tKFBhDbEGevcvMaHoRPGLJdxwBkTypcRRu1LdZK"),
			VESTING_START_6_MONTHS,
			VESTING_PERIOD_2,
			892_857 * UNIT,
		),
		// 57
		(
			aid("5E5D4ZzeeJziCLgJ9o1nG28PRskK7mxECnUSEuLNyTfiBvzf"),
			VESTING_START_6_MONTHS,
			VESTING_PERIOD_2,
			1_679_786 * UNIT,
		),
		// 58
		(
			aid("5GQqn8H4YSeisZYkuw5ZRiRoto2v7uYZ1AXP18ML4ozgyKvi"),
			VESTING_START_6_MONTHS,
			VESTING_PERIOD_2,
			932_203 * UNIT,
		),
		// 59
		(
			aid("5D7knkQ8aCGLtpd8rfWh1PRCUsBmCVSZ8ex1fbAtVLy3DNQL"),
			VESTING_START_6_MONTHS,
			VESTING_PERIOD_2,
			892_857 * UNIT,
		),
		// 60
		(
			aid("5EqGk8NmLiNPGqJY9GdEiRMoTDZKcjH6vCqEiMNuQDxY9guF"),
			VESTING_START_6_MONTHS,
			VESTING_PERIOD_2,
			39_346 * UNIT,
		),
		// 61
		(
			aid("5CMZfoaSeGxxipfEPRFx4gGDawjc1nAEhJFfHME7Aday6qRj"),
			VESTING_START_6_MONTHS,
			VESTING_PERIOD_2,
			230_769 * UNIT,
		),
		// 62
		(
			aid("5GGmFnQEzyiyepxGVVZbZGUnTY44poHCXQXPjSDfEyVUruti"),
			VESTING_START_6_MONTHS,
			VESTING_PERIOD_2,
			196_730 * UNIT,
		),
		// 63
		(
			aid("5FjECQd4WXN4fAZA5Hufa8jFTYUwvCzSz2DBUwDsSoKNTRNC"),
			VESTING_START_6_MONTHS,
			VESTING_PERIOD_2,
			100_726 * UNIT,
		),
		// 64
		(
			aid("5GZK8UaUrkh4fLkh1NXesEbeLwDzNPS2qL2nhij2F1TxSCD6"),
			VESTING_START_6_MONTHS,
			VESTING_PERIOD_2,
			59_019 * UNIT,
		),
		// 65
		(
			aid("5FeVG5yU4eiaNns5wHY2fKLc4BVP8cNCadbAFJQVy2KP26Bg"),
			VESTING_START_6_MONTHS,
			VESTING_PERIOD_2,
			21_225 * UNIT,
		),
		// ------------------------------------------------------------//
		// 75 216_990_640
		(
			aid("5GNpZ4uYK2FdYynfjmnBdd3rEQao2jrZWA5yiXXZrDPc1utd"),
			VESTING_START_6_MONTHS,
			VESTING_PERIOD_2,
			9_847_203 * UNIT,
		),
		(
			aid("5ChwtgueM1qg3HEM59B8kLT7d5ipyJUgof2i46SohB1Z4rWi"),
			VESTING_START_6_MONTHS,
			VESTING_PERIOD_2,
			18_534_771 * UNIT,
		),
		(
			aid("5HCH7NhC63dQGkF57Mj2kc6ZtK2ZiQ6FoXQ8JbzCxXDoV13R"),
			VESTING_START_6_MONTHS,
			VESTING_PERIOD_2,
			12_099_488 * UNIT,
		),
		(
			aid("5Dcff2aGrPY3E99F5H66MyjfGnVEZe3VM8J5ePX92yA8ud8M"),
			VESTING_START_6_MONTHS,
			VESTING_PERIOD_2,
			25_803_640 * UNIT,
		),
		(
			aid("5GYR3gUaXbLoPwGTqkgvk2U36BrWDgNXDVJaKmAY7ujvrUJi"),
			VESTING_START_6_MONTHS,
			VESTING_PERIOD_2,
			14_255_691 * UNIT,
		),
		(
			aid("5G1hb3aG4EvqshKAgZy5qTQHgVZZUEEgRvHczZ8zUjgXzfKK"),
			VESTING_START_6_MONTHS,
			VESTING_PERIOD_2,
			21_708_925 * UNIT,
		),
		(
			aid("5ELibc3edcNaLJNG458xYtRxTB68tZ7jbEoT4f7HMa1QmUZw"),
			VESTING_START_6_MONTHS,
			VESTING_PERIOD_2,
			8_932_474 * UNIT,
		),
		(
			aid("5GTjkj8bfz2UVRmfTfE9hhkUJno4PE1WvijQHbZgM2EgoAR3"),
			VESTING_START_6_MONTHS,
			VESTING_PERIOD_2,
			16_340_518 * UNIT,
		),
		(
			aid("5HgYD4a3zzGARZ9y6hawtzgd6aNVivdtV5YoJUbYNjzujBMT"),
			VESTING_START_6_MONTHS,
			VESTING_PERIOD_2,
			19_075_299 * UNIT,
		),
		(
			aid("5FxbtnLwxso6gWYW8ibxbVSEXhVkhoUMubtSP76W6YWAxwDC"),
			VESTING_START_6_MONTHS,
			VESTING_PERIOD_2,
			10_561_377 * UNIT,
		),
		(
			aid("5FWdwuDjsApXCvUmC8462okijSc442gxtehMk6ChkQSSNMZM"),
			VESTING_START_6_MONTHS,
			VESTING_PERIOD_2,
			7_388_549 * UNIT,
		),
		(
			aid("5HMvczemAjM5kNgCzL6m7qsKgy92bZQrUbAPrbUYctztsJ6G"),
			VESTING_START_6_MONTHS,
			VESTING_PERIOD_2,
			11_665_888 * UNIT,
		),
		(
			aid("5CcdakwQVLysn1Y9JnCh22S89cfQq5wZdDMu1eTuxG7XKAew"),
			VESTING_START_6_MONTHS,
			VESTING_PERIOD_2,
			13_699_562 * UNIT,
		),
		(
			aid("5DcpeKUdLtCsQ7rnw7cXhyRirbMvfkbbHkCxe75HRYsPLAXX"),
			VESTING_START_6_MONTHS,
			VESTING_PERIOD_2,
			27_077_255 * UNIT,
		),
		// 76
		(treasury_pallet_account(), VESTING_START, VESTING_PERIOD_1, 240_000_000 * UNIT),
	];

	// 77
	let initial_balances: Balance =
		initial_balances().iter().fold(0, |current, (_, a)| current + *a);
	res.push((
		operation_funds_pallet_account(),
		VESTING_START,
		VESTING_PERIOD_1,
		(115_000_000 * UNIT) - initial_balances - EXISTENTIAL_DEPOSIT, // - EXISTENTIAL_DEPOSIT from 70
	));

	res
}

fn erc20_allocations() -> Vec<Balance> {
	vec![
		// 11
		8_000_000 * UNIT,
		// 14
		3_333_333 * UNIT,
		// ------------------------------------------------------------//
		// 64_906_307
		// 35
		58_517_419 * UNIT,
		// 36
		5_000_000 * UNIT,
		// 37
		1_111_111 * UNIT,
		// 38
		277_777 * UNIT,
	]
}

#[cfg(test)]
mod test {
	use sp_runtime::traits::Zero;

	use super::*;

	#[test]
	fn test_total_supply() {
		let computed_total_supply: Balance =
			endowed_accounts().iter().fold(0, |current, (_, amount)| current + *amount)
				+ erc20_allocations().iter().sum::<Balance>();
		println!("TARGET TOTAL SUPPLY: {}", format_amount(INITIAL_TOTAL_SUPPLY));
		println!("COMPUTED TOTAL SUPPLY: {}", format_amount(computed_total_supply));
		let diff = INITIAL_TOTAL_SUPPLY.saturating_sub(computed_total_supply);
		if !diff.is_zero() {
			println!("MISSING: {}", format_amount(diff));
		}
		assert!(computed_total_supply <= INITIAL_TOTAL_SUPPLY);
	}

	pub fn format_amount(amount: Balance) -> String {
		use bigdecimal::{num_bigint::BigInt, BigDecimal};

		let amount = BigDecimal::new(BigInt::from(amount), 12);
		let rounded = amount.round(3);

		// Convert to string
		let s = rounded.to_string();
		let mut parts = s.split('.');
		let int_part = parts.next().unwrap_or("0");
		let frac_part = parts.next().unwrap_or("");

		// Insert thousands separators
		let mut grouped = String::new();
		for (i, ch) in int_part.chars().rev().enumerate() {
			if i > 0 && i % 3 == 0 {
				grouped.push('\'');
			}
			grouped.push(ch);
		}
		if grouped.ends_with('\'') {
			grouped.pop();
		}

		grouped = grouped.chars().rev().collect::<String>();

		// Ensure exactly 2 decimals
		let frac_fmt = match frac_part.len() {
			0 => "00".to_string(),
			1 => format!("{frac_part}0"),
			_ => frac_part[..3].to_string(),
		};

		format!("{grouped}.{frac_fmt} ACU")
	}
}
