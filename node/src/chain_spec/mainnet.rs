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
const ERC20_ALLOCATION: Balance = 65_000_000 * UNIT;
const VESTING_START: BlockNumber = 0;
const VESTING_INITIAL_LIQUIDITY: Balance = UNIT;
const MONTH: BlockNumber = DAYS * 30;

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
			+ ERC20_ALLOCATION;
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
		Some(acurast_sudo_account()),
	))
	.build()
}

fn genesis_config(
	invulnerables: Vec<(AccountId, AuraId)>,
	endowed_accounts: Vec<(AccountId, Balance)>,
	vesting: Vec<(AccountId, BlockNumber, BlockNumber, Balance)>,
	id: ParaId,
	sudo_account: Option<AccountId>,
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
			"initialEthTokenAllocation": Some(ERC20_ALLOCATION),
		},
		"sudo": {
			"key": sudo_account
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

pub fn treasury_pallet_account() -> AccountId {
	acurast_runtime::TreasuryPalletId::get().into_account_truncating()
}

pub fn hyperdrive_accounts() -> Vec<AccountId> {
	vec![
		HyperdriveTokenEthereumFeeVault::get(),
		HyperdriveTokenEthereumVault::get(),
		HyperdriveIbcFeePalletAccount::get(),
		HyperdriveTokenSolanaFeeVault::get(),
		HyperdriveTokenSolanaVault::get(),
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
		accountid_from_str("5Dt7iJBxvWztigqXiXqm8EU5xVBWcUrfXA5am1e8sF1RjUuW"),
		accountid_from_str("5EEe4WLNneqz3Fp2n61ZcTiGU6GLEvUgVmnkKaaxARSdVpdg"),
		accountid_from_str("5EbvNf3q5Xb918UvHBuB6rPfYuom38QAqw8osV5TQeaELWxP"),
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

	balances
}

pub fn allocations() -> Vec<(AccountId, BlockNumber, BlockNumber, Balance)> {
	use super::accountid_from_str as aid;
	let mut res = vec![
		// 9
		(
			aid("5FHFmtrCmCshiuo2kz4ebKUtEaxjqsLgkN4hVaCjrYjaTB8C"),
			VESTING_START,
			24 * MONTH,
			12_500_000 * UNIT,
		),
		// 10
		(
			aid("5FF2mdznLCFFY4qnJLzUEUW783jw2fN8NoZxVhdxAtw7FZeY"),
			VESTING_START,
			24 * MONTH,
			650_000 * UNIT,
		),
		// 11
		(
			aid("5DaHTXS2s3JVNKJC9J4ig9gNhqiwKsdLQMQM1EHM7Je4ph5A"),
			VESTING_START,
			24 * MONTH,
			8_000_000 * UNIT,
		),
		// 12
		(
			aid("5DvYaypHDLPh3ecvd3y8KQgUiGUgs4L9jpWMq1jCwWRzcVTV"),
			VESTING_START,
			24 * MONTH,
			5_000_000 * UNIT,
		),
		// 13
		(
			aid("5CXhVk1woyNyjKoC1NEBZdHXoBcSTQSQbaEPYbt3e2gHeAvy"),
			VESTING_START,
			24 * MONTH,
			3_750_000 * UNIT,
		),
		// 14
		(
			aid("5Fsv2HuAobzW9HqHYgg4tP8t9AK913VdHR3HcFrN5GnNkhQx"),
			VESTING_START,
			24 * MONTH,
			3_333_333 * UNIT,
		),
		// 15
		(
			aid("5GU6WVizgG7mgFjBhtpjQENg2u3GEbGDz5gr66ac4tA6rY29"),
			VESTING_START,
			24 * MONTH,
			2_975_000 * UNIT,
		),
		// 16
		(
			aid("5GEbRgiwPkvGD5ysvg4X1ScvmGm9zw1sHATRA4j6vtGHhhGf"),
			VESTING_START,
			24 * MONTH,
			2_500_000 * UNIT,
		),
		// 17
		(
			aid("5DcEYPwdT42tveBji3AYqQYiNXiPWyQKkk2qJfZ11jocPqnM"),
			VESTING_START,
			24 * MONTH,
			1_500_000 * UNIT,
		),
		// 18
		(
			aid("5CLw4Q1ED88CqnMt4SPLXpvs9M1g2ra2NLZG7C9CwvW7CRhT"),
			VESTING_START,
			24 * MONTH,
			2_083_330 * UNIT,
		),
		// 19
		(
			aid("5DaYt67ALZQkMPATFFyHs7nhmoP2YzLmQhF5Vc3YF53XKHT7"),
			VESTING_START,
			24 * MONTH,
			1_660_000 * UNIT,
		),
		// 20
		(
			aid("5EZ8nmLL3Y2zipViZAFREdPKkvk2HMZG2ecYLiQTrmNZb2gX"),
			VESTING_START,
			24 * MONTH,
			1_250_000 * UNIT,
		),
		// 21
		(
			aid("5ConWEnTaM5gpBZKEeKPC1DGQK4FvnXMkH4AvxmdkLEbBA9u"),
			VESTING_START,
			24 * MONTH,
			1_250_000 * UNIT,
		),
		// 22
		(
			aid("5CfwSpatsBEFosCYT2sgzDxtWQLLUsS2VbM9vn5KNdCfBbXd"),
			VESTING_START,
			24 * MONTH,
			1_250_000 * UNIT,
		),
		// 23
		(
			aid("5Fn7nfAmNsYsNksmpgBgTGidYyPjJfwMHFivBYzuo2VgqFyC"),
			VESTING_START,
			24 * MONTH,
			1_000_000 * UNIT,
		),
		// 24
		(
			aid("5Eq8vFnMK9EtSeQVVQyDrjb3rsFh3Y5v8keSarTw684XSxaG"),
			VESTING_START,
			24 * MONTH,
			1_000_000 * UNIT,
		),
		// 25
		(
			aid("5HWFhUkYeVUEpB9J4HcGkZTjp5GYKbL5L6VtdMMtUHCRXQ15"),
			VESTING_START,
			24 * MONTH,
			769_230 * UNIT,
		),
		// 26
		(
			aid("5DFbP5ahcuxmwnndf8G8SMDoLTDAqupzpgQfSyKkbPNoSRfm"),
			VESTING_START,
			24 * MONTH,
			2_307_000 * UNIT,
		),
		// 27
		(
			aid("5GBmEHDvvQesJKq7TphQEVa27ZB3Wtr7aa49GocWS1u9gYVY"),
			VESTING_START,
			24 * MONTH,
			1_923_000 * UNIT,
		),
		// 28
		(
			aid("5DXNyGYWWc37yBQMh1xabqdqTKvHLM7upCbVn49Yo7nNom39"),
			VESTING_START,
			24 * MONTH,
			762_930 * UNIT,
		),
		// 29
		(
			aid("5DrBcCrQZBq5rphg2DPAZXKdsC9kQeKdktfpo7a7Enhq8ta2"),
			VESTING_START,
			24 * MONTH,
			153_800 * UNIT,
		),
		// 30
		(
			aid("5G1qzLoLoAngfmphcNurx7JcdJuRAYxjfvk2u3CPaKqpo57e"),
			VESTING_START,
			24 * MONTH,
			30_769 * UNIT,
		),
		// 39
		(
			aid("5FpTNNzRHk9FpAn1dfU6XCwJrugiUyfZZVnNRpWZBKMKVCJM"),
			VESTING_START,
			36 * MONTH,
			8_770_000 * UNIT,
		),
		// 40
		(
			aid("5EoDpZbVpESbGCWJ8jSyLEhm3qMftGvt2bC9iLhfpGhFyAxA"),
			VESTING_START,
			36 * MONTH,
			858_333 * UNIT,
		),
		// 41
		(
			aid("5DaVdX8aXvmuNXoDhDLpf9269qAjLezfhc44p3vDf1NdpK9e"),
			VESTING_START,
			36 * MONTH,
			769_231 * UNIT,
		),
		// 42
		(
			aid("5GCFaVsnZTnZg5DuXCEhNe8neovbLxDvVo5XQwRq4Cfc4xtk"),
			VESTING_START,
			36 * MONTH,
			600_000 * UNIT,
		),
		// 43
		(
			aid("5GhDQoHkQB7gLJDFb1i3KBY3jYZcRRBjgmAwbiiDmB9NcWDc"),
			VESTING_START,
			36 * MONTH,
			600_000 * UNIT,
		),
		// 44
		(
			aid("5Ey5hSbQF4fFZfP9QbpW41RuzoSjdrjkb1A7rj9Mxx7FJFPD"),
			VESTING_START,
			36 * MONTH,
			600_000 * UNIT,
		),
		// 45
		(
			aid("5FTSWTFSefDrcZLmnmZiFG6WARfCFJeozxmhf6Wn8waLGuDf"),
			VESTING_START,
			36 * MONTH,
			600_000 * UNIT,
		),
		// 46
		(
			aid("5FNVnF6C7wRWJu53W9vXgf7tqCYM1kZTVAjgFLH6qvNFhHQo"),
			VESTING_START,
			36 * MONTH,
			1_412_831 * UNIT,
		),
		// 47
		(
			aid("5DcEVE8tK131ZGwroEUdZ3N8LcKtCYjZRfQ3cACWht5PhMWN"),
			VESTING_START,
			36 * MONTH,
			769_200 * UNIT,
		),
		// 49
		(
			aid("5DfYNQJSMojZdXP1Zk6yiTKdXPcMC6rfteC7t7kGCBwjy3B2"),
			VESTING_START,
			36 * MONTH,
			227_202_436 * UNIT,
		),
		// 50
		(
			aid("5Ci66NwksK9nuji9WsQoF9154AHKxPHWCBUazUQyLGN1fzbi"),
			VESTING_START,
			24 * MONTH,
			175_000_000 * UNIT,
		),
		// 51
		(treasury_pallet_account(), VESTING_START, 24 * MONTH, 240_000_000 * UNIT),
		// 53
		(
			aid("5HdHnVgj1qd7yRqogUN3Mo2ZLoBCQHfBmVVpqqCDbkbssuTH"),
			VESTING_START,
			24 * MONTH,
			100_000_000 * UNIT,
		),
	];

	// 52
	let initial_balances: Balance =
		initial_balances().iter().fold(0, |current, (_, a)| current + a);
	res.push((
		aid("5G4SRUSXqBu86aoz44dXjMcXLCcK46DA4jumAjdhYKKdM5T4"),
		VESTING_START,
		24 * MONTH,
		(115_000_000 * UNIT) - initial_balances,
	));

	res
}

#[cfg(test)]
mod test {
	use sp_runtime::traits::Zero;

	use super::*;

	#[test]
	fn test_total_supply() {
		let computed_total_supply: Balance =
			endowed_accounts().iter().fold(0, |current, (_, amount)| current + *amount)
				+ ERC20_ALLOCATION;
		println!("TARGET TOTAL SUPPLY: {INITIAL_TOTAL_SUPPLY}");
		println!("COMPUTED TOTAL SUPPLY: {computed_total_supply}");
		let diff = INITIAL_TOTAL_SUPPLY.saturating_sub(computed_total_supply);
		if !diff.is_zero() {
			println!("MISSING: {diff}");
		}
		assert!(computed_total_supply <= INITIAL_TOTAL_SUPPLY);
	}
}
