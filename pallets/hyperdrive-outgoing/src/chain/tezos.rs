use crate::traits::MMRInstance;
use alloc::string::String;
use codec::alloc;
use derive_more::Error as DError;
use derive_more::{Display, From};
use once_cell::race::OnceBox;
use sp_core::{RuntimeDebug, H256};
use sp_runtime::traits::Keccak256;
use sp_std::prelude::*;
use sp_std::vec;
use tezos_core::types::encoded::{Encoded, P256PublicKey, PublicKey};
use tezos_core::types::number::Nat;
use tezos_core::Error as TezosCoreError;
use tezos_michelson::micheline::Micheline;
use tezos_michelson::michelson::data;
use tezos_michelson::michelson::data::String as TezosString;
use tezos_michelson::michelson::types::{address, bytes, nat, pair, string};
use tezos_michelson::Error as TezosMichelineError;

use pallet_acurast_marketplace::{PubKey, PubKeyBytes};

use crate::instances::TezosInstance;
use crate::types::TargetChainConfig;
use crate::Action;
use crate::Leaf;
use crate::{LeafEncoder, RawAction};

#[derive(RuntimeDebug, Display, From)]
#[cfg_attr(feature = "std", derive(DError))]
pub enum TezosValidationError {
    TezosMichelineError(TezosMichelineError),
    TezosCoreError(TezosCoreError),
    UnexpectedPublicKey,
}

/// The [`LeafEncoder`] for Tezos using Micheline/Michelson encoding/packing.
pub struct TezosEncoder();

impl LeafEncoder for TezosEncoder {
    type Error = TezosValidationError;

    /// Encodes the given message for Tezos.
    ///
    /// Message gets encoded/packed as
    ///
    /// ```text
    /// RawMessage {
    ///     id: u32,
    ///     action: crate::RawAction,
    ///     payload: Vec<u8>,
    /// }
    /// ```
    ///
    /// where payload is dependent on `action` and encoded as a sequence of the [`Action`] variants' bodies, e.g.
    /// `[JobIdSequence, Vec<TezosAddressBytes>]` in the case of [`Action::AssignJob`].
    fn encode(message: &Leaf) -> Result<Vec<u8>, Self::Error> {
        let raw_action: RawAction = (&message.action).into();
        let action_str: &'static str = raw_action.into();
        let data = data::pair(vec![
            data::int(message.id as i64),
            data::try_string(action_str)?,
            data::bytes(match &message.action {
                Action::AssignJob(job_id, processor_public_key) => {
                    let address = match processor_public_key {
                        PubKey::SECP256r1(pk) => p256_pub_key_to_address(pk)?,
                        _ => Err(TezosValidationError::UnexpectedPublicKey)?,
                    };
                    let data = data::pair(vec![
                        data::nat(Nat::from_integer(*job_id)),
                        data::string(TezosString::from_string(address.to_owned())?),
                    ]);
                    Micheline::pack(data, Some(assign_payload_schema()))
                }
                Action::FinalizeJob(job_id, refund) => {
                    let data = data::pair(vec![
                        data::nat(Nat::from_integer(*job_id)),
                        data::int(Nat::from_integer(*refund)),
                    ]);
                    Micheline::pack(data, Some(finalize_payload_schema()))
                }
                Action::Noop => Ok(Default::default()),
            }?),
        ]);

        Ok(Micheline::pack(data, Some(message_schema()))?)
    }
}

#[cfg_attr(rustfmt, rustfmt::skip)]
fn message_schema() -> &'static Micheline {
    static MESSAGE_SCHEMA: OnceBox<Micheline> = OnceBox::new();
    MESSAGE_SCHEMA.get_or_init(|| {
        let schema: Micheline = pair(vec![
            // id
            nat(),
            // action
            string(),
            // payload
            bytes(),
        ]);
        Box::new(schema)
    })
}

#[cfg_attr(rustfmt, rustfmt::skip)]
fn assign_payload_schema() -> &'static Micheline {
    static ASSIGN_PAYLOAD_SCHEMA: OnceBox<Micheline> = OnceBox::new();
    ASSIGN_PAYLOAD_SCHEMA.get_or_init(|| {
        let schema: Micheline = pair(vec![
            // job_id_seq
            nat(),
            // processor_address
            address()
        ]);
        Box::new(schema)
    })
}

#[cfg_attr(rustfmt, rustfmt::skip)]
fn finalize_payload_schema() -> &'static Micheline {
    static FINALIZE_PAYLOAD_SCHEMA: OnceBox<Micheline> = OnceBox::new();
    FINALIZE_PAYLOAD_SCHEMA.get_or_init(|| {
        let schema: Micheline = pair(vec![
            // job_id_seq
            nat(),
            // refund
            nat()
        ]);
        Box::new(schema)
    })
}

pub struct DefaultTezosConfig;

impl TargetChainConfig for DefaultTezosConfig {
    type TargetChainEncoder = TezosEncoder;
    type Hasher = Keccak256;
    type Hash = H256;
}

/// Helper function to covert the BoundedVec [`PubKeyBytes`] to a Tezos [`String`].
pub fn p256_pub_key_to_address(pub_key: &PubKeyBytes) -> Result<String, TezosCoreError> {
    let key = P256PublicKey::from_bytes(pub_key)?;
    let key: PublicKey = key.into();
    key.bs58_address()
}

impl MMRInstance for TezosInstance {
    const INDEXING_PREFIX: &'static [u8] = b"mmr-tez-";
    const TEMP_INDEXING_PREFIX: &'static [u8] = b"mmr-tez-temp-";
}

#[cfg(feature = "std")]
pub mod rpc {
    use crate::instances::TezosInstance;
    use crate::rpc::RpcInstance;

    impl RpcInstance for TezosInstance {
        const SNAPSHOT_ROOTS: &'static str = "hyperdrive_outgoing_tezos_snapshotRoots";
        const SNAPSHOT_ROOT: &'static str = "hyperdrive_outgoing_tezos_snapshotRoot";
        const GENERATE_PROOF: &'static str = "hyperdrive_outgoing_tezos_generateProof";
    }
}

#[cfg(test)]
mod tests {
    use hex_literal::hex;

    use crate::stub::p256_public_key;
    use crate::{chain::tezos, Message};

    use super::*;

    #[test]
    fn test_pack_assign_job() -> Result<(), <TezosEncoder as LeafEncoder>::Error> {
        let encoded = tezos::TezosEncoder::encode(&Message {
            id: 5,
            action: Action::AssignJob(4, p256_public_key()),
        })?;

        let expected = &hex!("05070700050707010000001441535349474e5f4a4f425f50524f434553534f520a0000002005070700040a00000016000292251ea7a095ef710f65258ecd6b7246e209436e");
        assert_eq!(expected, &*encoded);
        Ok(())
    }

    #[test]
    fn test_pack_finalize_job() -> Result<(), <TezosEncoder as LeafEncoder>::Error> {
        let encoded = tezos::TezosEncoder::encode(&Message {
            id: 2,
            action: Action::FinalizeJob(3, 10),
        })?;

        let expected =
            &hex!("05070700020707010000000c46494e414c495a455f4a4f420a000000070507070003000a");
        assert_eq!(expected, &*encoded);
        Ok(())
    }
}
