#![cfg_attr(all(feature = "alloc", not(feature = "std"), not(test)), no_std)]

use core::marker::PhantomData;

use codec::{Decode, Encode};
use derive_more::Display;
use scale_info::{
    prelude::{format, string::String},
    TypeInfo,
};
use sp_core::bounded::BoundedVec;
use sp_core::{RuntimeDebug, H256};
use sp_runtime::traits::Hash;
use sp_std::prelude::*;
use sp_std::vec;

use ckb_merkle_mountain_range::{Error as MMRError, Merge, MerkleProof as MMRMerkleProof};

use pallet_acurast::{
    AllowedSources, Environment, JobModule, JobModules, JobRegistration, MultiOrigin, Schedule,
    Script, CU32,
};
use pallet_acurast_marketplace::{
    JobRequirements, PlannedExecution, PlannedExecutions, RegistrationExtra,
};

use crate::{traits, MessageIdentifier, ParsedAction};
use acurast_core_ink::types::{
    OutgoingAction as HyperdriveAction, OutgoingActionPayloadV1 as ActionPayloadV1,
    VersionedOutgoingActionPayload as HyperdriveVersionedActionPauload,
};

struct MergeKeccak;

impl Merge for MergeKeccak {
    type Item = [u8; 32];
    fn merge(lhs: &Self::Item, rhs: &Self::Item) -> Result<Self::Item, MMRError> {
        let mut concat = vec![];
        concat.extend(lhs);
        concat.extend(rhs);

        let hash = sp_runtime::traits::Keccak256::hash(&concat);

        Ok(hash.try_into().expect("INVALID_HASH_LENGTH"))
    }
}

pub type MMRProofItems = BoundedVec<H256, CU32<128>>;

#[derive(RuntimeDebug, Encode, Decode, TypeInfo, Clone, Eq, PartialEq)]
pub struct ProofLeaf {
    pub leaf_index: u64,
    pub data: Vec<u8>,
}

#[derive(RuntimeDebug, Encode, Decode, TypeInfo, Clone, Eq, PartialEq)]
#[scale_info(skip_type_params(AccountConverter))]
pub struct SubstrateProof<AccountConverter, AccountId> {
    pub mmr_size: u64,
    pub proof: MMRProofItems,
    pub leaves: Vec<ProofLeaf>,
    #[cfg(any(test, feature = "runtime-benchmarks"))]
    pub marker: PhantomData<(AccountConverter, AccountId)>,
    #[cfg(not(any(test, feature = "runtime-benchmarks")))]
    marker: PhantomData<(AccountConverter, AccountId)>,
}

/// Errors returned by this crate.
#[derive(RuntimeDebug, Display)]
pub enum SubstrateValidationError {
    ProofInvalid(String),
    InvalidMessage,
    CouldNotDecodeAction(String),
    TooManyPlannedExecutions,
    TooManyAllowedSources,
    InvalidJobModule,
    TooManyJobModules,
    CouldNotConvertAccountId,
}

impl<T, I: 'static, AccountConverter> traits::Proof<T, I>
    for SubstrateProof<AccountConverter, T::AccountId>
where
    T: crate::pallet::Config<I>,
    T::RegistrationExtra: From<RegistrationExtra<T::Balance, T::AccountId, T::MaxSlots>>,
    AccountConverter: TryFrom<Vec<u8>> + Into<T::AccountId>,
{
    type Error = SubstrateValidationError;

    fn calculate_root(self: &Self) -> Result<[u8; 32], Self::Error> {
        // Prepare proof instance
        let mmr_proof = MMRMerkleProof::<[u8; 32], MergeKeccak>::new(
            self.mmr_size,
            self.proof.iter().map(|h| h.0).collect(),
        );

        // Derive root from proof and leaves
        let hashed_leaves: Vec<(u64, [u8; 32])> = self
            .leaves
            .iter()
            .map(|item| {
                let hash = sp_runtime::traits::BlakeTwo256::hash(&item.data);

                match <[u8; 32]>::try_from(hash) {
                    Ok(h) => Ok((item.leaf_index, h)),
                    Err(err) => Err(Self::Error::ProofInvalid(format!("{:?}", err))),
                }
            })
            .collect::<Result<Vec<(u64, [u8; 32])>, Self::Error>>()?;

        mmr_proof
            .calculate_root(hashed_leaves)
            .map_err(|err| Self::Error::ProofInvalid(format!("{:?}", err)))
    }

    fn message_id(self: &Self) -> Result<MessageIdentifier, Self::Error> {
        // TODO: Process multiple messages (currently we only process the first leaf from the proof)
        let message_bytes = self
            .leaves
            .get(0)
            .map(|leaf| leaf.data.clone())
            .ok_or(Self::Error::InvalidMessage)?;

        let action = HyperdriveAction::decode(&message_bytes)
            .map_err(|err| Self::Error::CouldNotDecodeAction(format!("{:?}", err)))?;

        Ok(MessageIdentifier::from(action.id))
    }

    fn message(self: &Self) -> Result<ParsedAction<T>, Self::Error> {
        // TODO: Process multiple messages (currently we only process the first leaf from the proof)
        let message_bytes = self
            .leaves
            .get(0)
            .map(|leaf| leaf.data.clone())
            .ok_or(Self::Error::InvalidMessage)?;

        let action = HyperdriveAction::decode(&message_bytes)
            .map_err(|err| Self::Error::CouldNotDecodeAction(format!("{:?}", err)))?;

        let origin = MultiOrigin::AlephZero(convert_account_id::<T::AccountId, AccountConverter>(
            &action.origin,
        )?);

        fn convert_account_id<Account, AccountConverter: TryFrom<Vec<u8>> + Into<Account>>(
            bytes: &[u8; 32],
        ) -> Result<Account, SubstrateValidationError> {
            let parsed: AccountConverter = bytes
                .to_vec()
                .try_into()
                .map_err(|_| SubstrateValidationError::CouldNotConvertAccountId)?;
            Ok(parsed.into())
        }

        let parsed_action: ParsedAction<T> = match action.payload {
            HyperdriveVersionedActionPauload::V1(action) => match action {
                ActionPayloadV1::RegisterJob(payload) => {
                    let executions: PlannedExecutions<T::AccountId, T::MaxSlots> =
                        PlannedExecutions::try_from(
                            payload
                                .instant_match
                                .into_iter()
                                .map(|m| {
                                    Ok(PlannedExecution {
                                        source: convert_account_id::<T::AccountId, AccountConverter>(
                                            &m.source,
                                        )?,
                                        start_delay: m.start_delay,
                                    })
                                })
                                .collect::<Result<Vec<PlannedExecution<T::AccountId>>, Self::Error>>(
                                )?,
                        )
                        .map_err(|_| Self::Error::TooManyPlannedExecutions)?;

                    let extra: T::RegistrationExtra = RegistrationExtra {
                        requirements: JobRequirements {
                            slots: payload.slots.into(),
                            reward: T::Balance::from(payload.reward),
                            min_reputation: payload.min_reputation,
                            instant_match: Some(executions),
                        },
                    }
                    .into();
                    let registration = JobRegistration {
                        script: Script::truncate_from(payload.script),
                        allowed_sources: Some(
                            AllowedSources::try_from(
                                payload
                                    .allowed_sources
                                    .iter()
                                    .map(|s| {
                                        convert_account_id::<T::AccountId, AccountConverter>(&s)
                                    })
                                    .collect::<Result<Vec<T::AccountId>, Self::Error>>()?,
                            )
                            .map_err(|_| Self::Error::TooManyAllowedSources)?,
                        ),
                        allow_only_verified_sources: payload.allow_only_verified_sources,
                        schedule: Schedule {
                            duration: payload.duration,
                            start_time: payload.start_time,
                            end_time: payload.end_time,
                            interval: payload.interval,
                            max_start_delay: payload.max_start_delay,
                        },
                        memory: payload.memory,
                        network_requests: payload.network_requests,
                        storage: payload.storage,
                        required_modules: JobModules::try_from(
                            payload
                                .required_modules
                                .iter()
                                .map(|item| {
                                    Ok(JobModule::try_from(*item as u32)
                                        .map_err(|_| Self::Error::InvalidJobModule)?)
                                })
                                .collect::<Result<Vec<_>, Self::Error>>()?,
                        )
                        .map_err(|_| Self::Error::TooManyJobModules)?,
                        extra: extra,
                    };

                    let job_id = (origin, payload.job_id as u128);

                    ParsedAction::RegisterJob(job_id, registration)
                }
                ActionPayloadV1::DeregisterJob(job_id) => {
                    ParsedAction::DeregisterJob((origin, job_id as u128))
                }
                ActionPayloadV1::FinalizeJob(payload) => ParsedAction::FinalizeJob(
                    payload
                        .iter()
                        .map(|id| (origin.clone(), *id as u128))
                        .collect(),
                ),
                ActionPayloadV1::SetJobEnvironment(payload) => {
                    let job_id = (origin, payload.job_id as u128);

                    let variables = payload
                        .processors
                        .iter()
                        .map(|processor| {
                            let processor_address =
                                convert_account_id::<T::AccountId, AccountConverter>(
                                    &processor.address,
                                )?;
                            let env = Environment {
                                public_key: BoundedVec::truncate_from(payload.public_key.clone()),
                                variables: BoundedVec::truncate_from(
                                    processor
                                        .variables
                                        .iter()
                                        .map(|(key, value)| {
                                            (
                                                BoundedVec::truncate_from(key.clone()),
                                                BoundedVec::truncate_from(value.clone()),
                                            )
                                        })
                                        .collect(),
                                ),
                            };

                            Ok((processor_address, env))
                        })
                        .collect::<Result<Vec<_>, Self::Error>>()?;

                    ParsedAction::SetJobEnvironment(job_id, BoundedVec::truncate_from(variables))
                }
                ActionPayloadV1::Noop => ParsedAction::Noop,
            },
        };

        Ok(parsed_action)
    }
}
