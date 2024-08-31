use std::str::FromStr;

use sc_chain_spec::{ChainSpecExtension, ChainSpecGroup};
use serde::{Deserialize, Serialize};

use acurast_runtime_common::AccountId;

#[cfg(any(feature = "acurast-local", feature = "acurast-dev"))]
const DEFAULT_PARACHAIN_ID: u32 = 2001;
#[cfg(feature = "acurast-rococo")]
const ROCOCO_PARACHAIN_ID: u32 = 2239;
#[cfg(feature = "acurast-kusama")]
const KUSAMA_PARACHAIN_ID: u32 = 2239;
#[cfg(feature = "acurast-mainnet")]
const MAINNET_PARACHAIN_ID: u32 = 3396;

#[cfg(feature = "acurast-dev")]
pub mod dev;
#[cfg(feature = "acurast-kusama")]
pub mod kusama;
#[cfg(feature = "acurast-local")]
pub mod local;
#[cfg(feature = "acurast-mainnet")]
pub mod mainnet;
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

pub const SS58_FORMAT: u32 = 42;
