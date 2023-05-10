use sc_chain_spec::{ChainSpecExtension, ChainSpecGroup};
use serde::{Deserialize, Serialize};

const DEFAULT_PARACHAIN_ID: u32 = 2001;
const ROCOCO_PARACHAIN_ID: u32 = 4191;
const KUSAMA_PARACHAIN_ID: u32 = 4191;

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
