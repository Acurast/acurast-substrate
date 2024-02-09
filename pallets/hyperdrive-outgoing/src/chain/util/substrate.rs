use crate::{Action, Leaf, LeafEncoder};
use codec::Encode;
use sp_io::hashing::blake2_256;
use sp_std::vec::Vec;

use pallet_acurast_marketplace::{PubKey, PubKeyBytes};
use sp_runtime::RuntimeDebug;

use acurast_core_ink::types::{
    AssignProcessorPayloadV1, FinalizeJobPayloadV1, IncomingAction, IncomingActionPayloadV1,
    VersionedIncomingActionPayload,
};

#[derive(RuntimeDebug)]
pub enum SubstrateValidationError {
    UnexpectedPublicKey,
}

/// The [`LeafEncoder`] for Evm encoding.
pub struct SubstrateEncoder();

impl LeafEncoder for SubstrateEncoder {
    type Error = SubstrateValidationError;

    /// Encodes the given message for EVM.
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
    /// `[JobIdSequence, PubKey]` in the case of [`Action::AssignJob`].
    fn encode(message: &Leaf) -> Result<Vec<u8>, Self::Error> {
        let payload = match &message.action {
            Action::AssignJob(job_id, processor_public_key) => {
                let address_bytes = match processor_public_key {
                    PubKey::SECP256k1(pk) => public_key_to_address_bytes(pk),
                    _ => Err(Self::Error::UnexpectedPublicKey)?,
                };

                let payload = AssignProcessorPayloadV1 {
                    job_id: *job_id,
                    processor: address_bytes,
                };
                IncomingActionPayloadV1::AssignJobProcessor(payload)
            }
            Action::FinalizeJob(job_id, refund_amount) => {
                let payload = FinalizeJobPayloadV1 {
                    job_id: *job_id,
                    unused_reward: *refund_amount,
                };

                IncomingActionPayloadV1::FinalizeJob(payload)
            }
            Action::Noop => IncomingActionPayloadV1::Noop,
        };
        let message = IncomingAction {
            id: message.id,
            payload: VersionedIncomingActionPayload::V1(payload),
        };

        Ok(message.encode())
    }
}

/// Helper function to covert the BoundedVec [`PubKeyBytes`] to an Substrate address.
pub fn public_key_to_address_bytes(pub_key: &PubKeyBytes) -> [u8; 32] {
    let account_id_bytes = blake2_256(pub_key);

    account_id_bytes
}
