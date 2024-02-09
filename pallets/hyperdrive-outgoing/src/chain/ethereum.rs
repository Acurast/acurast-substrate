use crate::chain::util::evm::EvmEncoder;
use crate::instances::EthereumInstance;
use crate::traits::MMRInstance;
use crate::TargetChainConfig;
use sp_core::H256;
use sp_runtime::traits::Keccak256;

pub struct EthereumConfig;

impl TargetChainConfig for EthereumConfig {
    type TargetChainEncoder = EvmEncoder;
    type Hasher = Keccak256;
    type Hash = H256;
}

impl MMRInstance for EthereumInstance {
    const INDEXING_PREFIX: &'static [u8] = b"mmr-eth-";
    const TEMP_INDEXING_PREFIX: &'static [u8] = b"mmr-eth-temp-";
}

#[cfg(feature = "std")]
mod rpc {
    use crate::instances::EthereumInstance;
    use crate::rpc::RpcInstance;

    impl RpcInstance for EthereumInstance {
        const SNAPSHOT_ROOTS: &'static str = "hyperdrive_outgoing_ethereum_snapshotRoots";
        const SNAPSHOT_ROOT: &'static str = "hyperdrive_outgoing_ethereum_snapshotRoot";
        const GENERATE_PROOF: &'static str = "hyperdrive_outgoing_ethereum_generateProof";
    }
}
