use std::str::FromStr;

use sc_chain_spec::{ChainSpecExtension, ChainSpecGroup};
use serde::{Deserialize, Serialize};

use acurast_runtime_common::AccountId;

const DEFAULT_PARACHAIN_ID: u32 = 2001;
const ROCOCO_PARACHAIN_ID: u32 = 2239;
const KUSAMA_PARACHAIN_ID: u32 = 2239;

#[cfg(feature = "acurast-dev")]
pub mod dev;
#[cfg(feature = "acurast-kusama")]
pub mod kusama;
#[cfg(feature = "acurast-local")]
pub mod local;
#[cfg(feature = "acurast-rococo")]
pub mod rococo;

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

fn accountid_from_str(account: &str) -> AccountId {
	AccountId::from_str(account).expect("valid account id")
}

fn processor_manager() -> Vec<(AccountId, Vec<AccountId>)> {
	vec![
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
	]
}
