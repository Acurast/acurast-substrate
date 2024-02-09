/// This trait exposes MMR constants specific to each target chain implementation
pub trait MMRInstance {
    /// Prefix for elements stored in the Off-chain DB via Indexing API.
    ///
    /// Each node of the MMR is inserted both on-chain and off-chain via Indexing API.
    /// The former does not store full leaf content, just its compact version (hash),
    /// and some of the inner mmr nodes might be pruned from on-chain storage.
    /// The latter will contain all the entries in their full form.
    ///
    /// Each node is stored in the Off-chain DB under key derived from the
    /// [`Self::INDEXING_PREFIX`] and its in-tree index (MMR position).
    const INDEXING_PREFIX: &'static [u8];
    /// Prefix for elements temporarily stored in the Off-chain DB via Indexing API.
    ///
    /// For fork resistency, nodes are first stored with their [`Self::TEMP_INDEXING_PREFIX`]
    /// before they get conanicalized and stored under a key with [`Self::INDEXING_PREFIX`].
    const TEMP_INDEXING_PREFIX: &'static [u8];
}

impl MMRInstance for () {
    const INDEXING_PREFIX: &'static [u8] = b"mmr-";
    const TEMP_INDEXING_PREFIX: &'static [u8] = b"mmr-temp-";
}
