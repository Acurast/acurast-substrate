use core::fmt::Debug;

#[cfg(not(feature = "std"))]
use codec::alloc::string::String;
use frame_support::pallet_prelude::*;
pub use mmr_lib;
use scale_info::TypeInfo;
use serde::{Deserialize, Serialize};
use sp_core::RuntimeDebug;
use sp_runtime::traits;
#[cfg(not(feature = "std"))]
use sp_std::prelude::*;
use strum_macros::{EnumString, IntoStaticStr};

use pallet_acurast::JobIdSequence;
use pallet_acurast_marketplace::PubKey;

/// A type to describe node position in the MMR (node index).
pub type NodeIndex = u64;

/// A type to describe snapshot number.
pub type SnapshotNumber = u64;

/// A type to describe leaf position in the MMR.
///
/// Note this is different from [`NodeIndex`], which can be applied to
/// both leafs and inner nodes. Leafs will always have consecutive `LeafIndex`,
/// but might be actually at different positions in the MMR `NodeIndex`.
pub type LeafIndex = u64;

/// New MMR root notification hook.
pub trait OnNewRoot<Hash> {
    /// Function called by the pallet in case new MMR root has been computed.
    fn on_new_root(root: &Hash);
}

/// No-op implementation of [OnNewRoot].
impl<Hash> OnNewRoot<Hash> for () {
    fn on_new_root(_root: &Hash) {}
}

/// The encodable version of an [`Action`].
#[derive(
    RuntimeDebug, Encode, Decode, TypeInfo, Clone, Eq, PartialEq, EnumString, IntoStaticStr,
)]
pub enum RawAction {
    #[strum(serialize = "ASSIGN_JOB_PROCESSOR")]
    AssignJob,
    #[strum(serialize = "FINALIZE_JOB")]
    FinalizeJob,
    #[strum(serialize = "NOOP")]
    Noop = 255,
}

impl From<&Action> for RawAction {
    fn from(action: &Action) -> Self {
        match action {
            Action::AssignJob(_, _) => RawAction::AssignJob,
            Action::FinalizeJob(_, _) => RawAction::FinalizeJob,
            Action::Noop => RawAction::Noop,
        }
    }
}

/// Convert [RawAction] to an index
impl Into<u16> for RawAction {
    fn into(self: Self) -> u16 {
        self as u16
    }
}

/// The action is triggered over Hyperdrive as part of a [`Message`].
#[derive(RuntimeDebug, Encode, Decode, TypeInfo, Eq, PartialEq, Clone)]
pub enum Action {
    /// Assigns a job on target chain.
    ///
    /// Consists of `(Job ID, processor address)`,
    /// where `Job ID` is the subset of [`pallet_acurast::JobId`] for jobs created externally.
    AssignJob(JobIdSequence, PubKey), // (u128, address)
    /// Finalizes a job on target chain.
    ///
    /// Consists of `(Job ID, refund amount)`,
    /// where `Job ID` is the subset of [`pallet_acurast::JobId`] for jobs created externally.
    FinalizeJob(JobIdSequence, u128), // (u128, u128)
    /// A noop action that solely suits the purpose of testing that messages get sent.
    Noop,
}

/// Message that is transferred to target chains.
#[derive(RuntimeDebug, Encode, Decode, TypeInfo, Eq, PartialEq, Clone)]
pub struct Message {
    pub id: u64,
    pub action: Action,
}

pub type Leaf = Message;

/// An element representing either full data or its hash.
#[derive(RuntimeDebug, Encode, Decode, TypeInfo, Clone, PartialEq, Eq)]
pub enum Node<Hash> {
    /// Arbitrary data in its full form.
    Data(Leaf),
    /// A hash of some data.
    Hash(Hash),
}

impl<H: traits::Hash> From<Leaf> for Node<H> {
    fn from(l: Leaf) -> Self {
        Self::Data(l)
    }
}

/// A bundled config of encoder/hasher for the target chain.
///
/// Extends traits [`traits::Hash`] and adds hashing with previously encoded value, using an encoding supported on target chain.
pub trait TargetChainConfig {
    type TargetChainEncoder: LeafEncoder;

    /// A hasher type for MMR.
    ///
    /// To construct trie nodes that result in merging (bagging) two peaks, depending on the
    /// node kind we take either:
    /// - The node (hash) itself if it's an inner node.
    /// - The hash of [`Self::LeafEncoder`]-encoded leaf data if it's a leaf node.
    ///
    /// Then we create a tuple of these two hashes (concatenate them) and
    /// hash, to obtain a new MMR inner node - the new peak.
    type Hasher: traits::Hash<Output = Self::Hash>;

    /// The hashing output type.
    ///
    /// This type is actually going to be stored in the MMR.
    /// Required to be provided separatly from [`Self::Hasher`], to satisfy trait bounds for storage items.
    type Hash: Member
        + MaybeSerializeDeserialize
        + Debug
        + sp_std::hash::Hash
        + AsRef<[u8]>
        + AsMut<[u8]>
        + Ord
        + Copy
        + Default
        + codec::Codec
        + codec::EncodeLike
        + scale_info::TypeInfo
        + MaxEncodedLen;

    /// Produce the hash of some encodable value.
    fn hash_for_target(
        leaf: &Leaf,
    ) -> Result<Self::Hash, <Self::TargetChainEncoder as LeafEncoder>::Error> {
        Ok(<Self::Hasher as traits::Hash>::hash(
            Self::TargetChainEncoder::encode(leaf)?.as_slice(),
        ))
    }
}

/// Hashing used for the pallet.
pub trait TargetChainNodeHasher<Hash> {
    type Error;
    fn hash_node(node: &Node<Hash>) -> Result<Hash, Self::Error>;
}

/// Implements node hashing for all nodes that contain leaves that support target chain hashing.
impl<H: TargetChainConfig> TargetChainNodeHasher<H::Hash> for H {
    type Error = <H::TargetChainEncoder as LeafEncoder>::Error;
    fn hash_node(node: &Node<H::Hash>) -> Result<H::Hash, Self::Error> {
        match *node {
            Node::Data(ref leaf) => H::hash_for_target(leaf),
            Node::Hash(ref hash) => Ok(*hash),
        }
    }
}

/// An encoder for leaves that can be decoded on target chains.
///
/// Note that we can't use [`codec::Encode`] since we derive that trait for the SCALE-encoding used to store leaves
/// in the off-chain index.
pub trait LeafEncoder {
    type Error: Debug;
    fn encode(leaf: &Leaf) -> Result<Vec<u8>, Self::Error>;
}

/// An MMR proof for a group of leaves.
#[derive(codec::Encode, codec::Decode, RuntimeDebug, Clone, PartialEq, Eq, TypeInfo)]
pub struct Proof<Hash> {
    /// The indices of the leaves the proof is for.
    pub leaf_indices: Vec<LeafIndex>,
    /// Number of leaves in MMR, when the proof was generated.
    pub leaf_count: NodeIndex,
    /// Proof elements (hashes of siblings of inner nodes on the path to the leaf).
    pub items: Vec<Hash>,
}

/// A self-contained MMR proof for a group of leaves, containing messages encoded for target chain.
#[derive(
    codec::Encode,
    codec::Decode,
    RuntimeDebug,
    Clone,
    PartialEq,
    Eq,
    TypeInfo,
    Serialize,
    Deserialize,
)]
#[serde(rename_all = "camelCase")]
pub struct TargetChainProof<Hash> {
    /// The indices of the leaves the proof is for.
    pub leaves: Vec<TargetChainProofLeaf>,
    /// Number of leaves in MMR, when the proof was generated.
    pub mmr_size: NodeIndex,
    /// Proof elements (hashes of siblings of inner nodes on the path to the leaf).
    /// Excluding MMR root.
    pub items: Vec<Hash>,
}

/// A leaf of a self-contained MMR [`TargetChainProof`].
#[derive(
    codec::Encode,
    codec::Decode,
    RuntimeDebug,
    Clone,
    PartialEq,
    Eq,
    TypeInfo,
    Serialize,
    Deserialize,
)]
#[serde(rename_all = "camelCase")]
pub struct TargetChainProofLeaf {
    /// The k-index of this leaf.
    pub k_index: NodeIndex,
    /// The position of this leaf.
    pub position: NodeIndex,
    /// The encoded message on this leaf.
    pub message: Vec<u8>,
}

/// Merkle Mountain Range operation error.
#[cfg_attr(feature = "std", derive(thiserror::Error))]
#[derive(RuntimeDebug, codec::Encode, codec::Decode, PartialEq, Eq, TypeInfo)]
pub enum MMRError {
    /// Error while pushing new node.
    #[cfg_attr(feature = "std", error("Error pushing new node"))]
    Push,
    /// Error getting the new root.
    #[cfg_attr(feature = "std", error("Error getting new root"))]
    GetRoot,
    /// Error committing changes.
    #[cfg_attr(feature = "std", error("Error committing changes"))]
    Commit,
    /// Error when snapshot meta index became inconsistent.
    #[cfg_attr(
        feature = "std",
        error("Snapshot meta index is inconsistent: missing snapshot that should be there")
    )]
    InconsistentSnapshotMeta,
    /// Error during proof generation.
    #[cfg_attr(feature = "std", error("Error generating proof"))]
    GenerateProof,
    /// Error during proof generation when no snapshot was taken yet.
    #[cfg_attr(
        feature = "std",
        error("Error generating proof: no snapshot taken yet")
    )]
    GenerateProofNoSnapshot,
    /// Error during proof generation when requested snapshot lies in the future.
    #[cfg_attr(
        feature = "std",
        error("Error generating proof: snapshot in the future")
    )]
    GenerateProofFutureSnapshot,
    /// Error during proof generation when requested message start lies in the future.
    #[cfg_attr(
        feature = "std",
        error("Error generating proof: message in the future")
    )]
    GenerateProofFutureMessage,
    /// Proof verification error.
    #[cfg_attr(feature = "std", error("Invalid proof"))]
    Verify,
    /// Leaf not found in the storage.
    #[cfg_attr(feature = "std", error("Leaf was not found"))]
    LeafNotFound,
}

impl MMRError {
    /// Consume given error `e` with `self` and generate a native log entry with error details.
    pub fn log_error(self, e: impl Debug) -> Self {
        log::error!(
            target: "runtime::acurast_hyperdrive_outgoing",
            "[{:?}] MMR error: {:?}",
            self,
            e,
        );
        self
    }

    /// Consume given error `e` with `self` and generate a native log entry with error details.
    pub fn log_debug(self, e: impl Debug) -> Self {
        log::debug!(
            target: "runtime::acurast_hyperdrive_outgoing",
            "[{:?}] MMR error: {:?}",
            self,
            e,
        );
        self
    }
}
