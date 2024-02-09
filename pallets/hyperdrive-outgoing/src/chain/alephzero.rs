use crate::instances::AlephZeroInstance;
use crate::traits::MMRInstance;
use crate::TargetChainConfig;
use sp_core::H256;
use sp_runtime::traits::Keccak256;

use super::util::substrate::SubstrateEncoder;

pub struct AlephZeroConfig;

impl TargetChainConfig for AlephZeroConfig {
    type TargetChainEncoder = SubstrateEncoder;
    type Hasher = Keccak256;
    type Hash = H256;
}

impl MMRInstance for AlephZeroInstance {
    const INDEXING_PREFIX: &'static [u8] = b"mmr-alephzero-";
    const TEMP_INDEXING_PREFIX: &'static [u8] = b"mmr-alephzero-temp-";
}

#[cfg(feature = "std")]
mod rpc {
    use crate::instances::AlephZeroInstance;
    use crate::rpc::RpcInstance;

    impl RpcInstance for AlephZeroInstance {
        const SNAPSHOT_ROOTS: &'static str = "hyperdrive_outgoing_alephzero_snapshotRoots";
        const SNAPSHOT_ROOT: &'static str = "hyperdrive_outgoing_alephzero_snapshotRoot";
        const GENERATE_PROOF: &'static str = "hyperdrive_outgoing_alephzero_generateProof";
    }
}
