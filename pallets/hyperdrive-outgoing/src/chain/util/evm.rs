use crate::{Action, Leaf, LeafEncoder, RawAction};
use sp_runtime::traits::{Hash, Keccak256};
use sp_std::vec::Vec;

use alloy_sol_types::{sol, SolType};
use pallet_acurast_marketplace::{PubKey, PubKeyBytes};
use sp_runtime::RuntimeDebug;

#[derive(RuntimeDebug)]
pub enum EvmValidationError {
    UnexpectedPublicKey,
}

// Declare a solidity type in standard solidity
sol! {
    struct EvmMessage {
        uint16 action;
        uint128 messageId;
        bytes payload;
    }

    type JobId is uint128;

    struct EvmFinalizeJob {
        uint128 job_id;
        uint128 refund_amount;
    }

    struct EvmAssignJob {
        uint128 job_id;
        address processor;
    }
}

/// The [`LeafEncoder`] for Evm encoding.
pub struct EvmEncoder();

impl LeafEncoder for EvmEncoder {
    type Error = EvmValidationError;

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
        let raw_action: RawAction = (&message.action).into();
        let payload = match &message.action {
            Action::AssignJob(job_id, processor_public_key) => {
                let address_bytes = match processor_public_key {
                    PubKey::SECP256k1(pk) => public_key_to_address(pk),
                    _ => Err(EvmValidationError::UnexpectedPublicKey)?,
                };

                let processor_address =
                    alloy_primitives::Address::from_slice(address_bytes.as_slice());

                let payload = EvmAssignJob {
                    job_id: *job_id,
                    processor: processor_address,
                };
                EvmAssignJob::encode_single(&payload)
            }
            Action::FinalizeJob(job_id, refund_amount) => {
                let payload = EvmFinalizeJob {
                    job_id: *job_id,
                    refund_amount: *refund_amount,
                };

                EvmFinalizeJob::encode_single(&payload)
            }
            Action::Noop => [].to_vec(),
        };
        let message = EvmMessage {
            action: raw_action.into(),
            messageId: message.id as u128,
            payload,
        };

        Ok(EvmMessage::encode_single(&message))
    }
}

/// Helper function to covert the BoundedVec [`PubKeyBytes`] to an EVM address.
pub fn public_key_to_address(pub_key: &PubKeyBytes) -> Vec<u8> {
    let hash = Keccak256::hash(pub_key);

    hash.0[..20].to_vec()
}
