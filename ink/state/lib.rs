#![cfg_attr(not(feature = "std"), no_std, no_main)]

use ink::env::call::Selector;
use ink::prelude::collections::BTreeMap;

type Map<T> = BTreeMap<u64, T>;

pub use state_aggregator::Error;

// Method selectors

pub const INSERT_SELECTOR: Selector = Selector::new(ink::selector_bytes!("insert"));
pub const GENERATE_PROOF_SELECTOR: Selector = Selector::new(ink::selector_bytes!("generate_proof"));

// Method types

pub type InsertReturn = Result<(u64, u128), state_aggregator::Error>;

pub type GenerateProofReturn =
    Result<state_aggregator::MerkleProof<[u8; 32]>, state_aggregator::Error>;

mod mmr {
    extern crate alloc;

    use alloc::borrow::Cow;
    use alloc::collections::VecDeque;
    use ckb_merkle_mountain_range::helper::{
        get_peak_map, get_peaks, parent_offset, pos_height_in_tree, sibling_offset,
    };
    use ckb_merkle_mountain_range::{Error, Merge, Result};
    use core::fmt::Debug;
    use core::marker::PhantomData;
    use ink::storage::traits::Packed;
    use scale::{Encode, EncodeLike};

    use ink::prelude::vec;
    use ink::prelude::vec::Vec;

    use crate::Map;

    #[allow(clippy::upper_case_acronyms)]
    pub struct MMR<T, M> {
        mmr_size: u64,
        batch: Vec<(u64, Vec<T>)>,
        merge: PhantomData<(T, M)>,
    }

    impl<T, M> MMR<T, M> {
        pub fn new(mmr_size: u64) -> Self {
            MMR {
                mmr_size,
                batch: Vec::new(),
                merge: PhantomData,
            }
        }

        pub fn mmr_size(&self) -> u64 {
            self.mmr_size
        }
    }

    impl<T: Clone + PartialEq + Encode + Packed + EncodeLike, M: Merge<Item = T>> MMR<T, M> {
        // find internal MMR elem, the pos must exists, otherwise a error will return
        fn find_elem<'b>(&self, pos: u64, store: &Map<T>, hashes: &'b [T]) -> Result<Cow<'b, T>> {
            let pos_offset = pos.checked_sub(self.mmr_size);
            if let Some(elem) = pos_offset.and_then(|i| hashes.get(i as usize)) {
                return Ok(Cow::Borrowed(elem));
            }
            let elem = store.get(&pos).ok_or(Error::InconsistentStore)?;
            Ok(Cow::Owned(elem.clone()))
        }

        // push a element and return position
        pub fn push(&mut self, store: &Map<T>, elem: T) -> Result<Vec<(u64, Vec<T>)>> {
            let mut elems = vec![elem];
            let elem_pos = self.mmr_size;
            let peak_map = get_peak_map(self.mmr_size);
            let mut pos = self.mmr_size;
            let mut peak = 1;
            while (peak_map & peak) != 0 {
                peak <<= 1;
                pos += 1;
                let left_pos = pos - peak;
                let left_elem = self.find_elem(left_pos, store, elems.as_slice())?;
                let right_elem = elems.last().expect("checked");
                let parent_elem = M::merge(&left_elem, right_elem)?;
                elems.push(parent_elem);
            }
            // store hashes
            self.batch.push((elem_pos, elems));
            // update mmr_size
            self.mmr_size = pos + 1;
            Ok(self.batch.clone())
        }

        /// get_root
        pub fn get_root(&self, store: &Map<T>) -> Result<T> {
            if self.mmr_size == 0 {
                return Err(Error::GetRootOnEmpty);
            } else if self.mmr_size == 1 {
                return store
                    .get(&0)
                    .map(|v| v.clone())
                    .ok_or(Error::InconsistentStore);
            }
            let peaks: Vec<T> = get_peaks(self.mmr_size)
                .into_iter()
                .map(|peak_pos| {
                    store
                        .get(&peak_pos)
                        .map(|v| v.clone())
                        .ok_or(Error::InconsistentStore)
                })
                .collect::<Result<Vec<T>>>()?;
            self.bag_rhs_peaks(peaks)?.ok_or(Error::InconsistentStore)
        }

        fn bag_rhs_peaks(&self, mut rhs_peaks: Vec<T>) -> Result<Option<T>> {
            while rhs_peaks.len() > 1 {
                let right_peak = rhs_peaks.pop().expect("pop");
                let left_peak = rhs_peaks.pop().expect("pop");
                rhs_peaks.push(M::merge_peaks(&right_peak, &left_peak)?);
            }
            Ok(rhs_peaks.pop())
        }

        /// generate merkle proof for a peak
        /// the pos_list must be sorted, otherwise the behaviour is undefined
        ///
        /// 1. find a lower tree in peak that can generate a complete merkle proof for position
        /// 2. find that tree by compare positions
        /// 3. generate proof for each positions
        fn gen_proof_for_peak(
            &self,
            proof: &mut Vec<T>,
            pos_list: Vec<u64>,
            peak_pos: u64,
            store: &Map<T>,
        ) -> Result<()> {
            // do nothing if position itself is the peak
            if pos_list.len() == 1 && pos_list == [peak_pos] {
                return Ok(());
            }
            // take peak root from store if no positions need to be proof
            if pos_list.is_empty() {
                proof.push(
                    store
                        .get(&peak_pos)
                        .map(|v| v.clone())
                        .ok_or(Error::InconsistentStore)?,
                );
                return Ok(());
            }

            let mut queue: VecDeque<_> = pos_list.into_iter().map(|pos| (pos, 0)).collect();

            // Generate sub-tree merkle proof for positions
            while let Some((pos, height)) = queue.pop_front() {
                debug_assert!(pos <= peak_pos);
                if pos == peak_pos {
                    if queue.is_empty() {
                        break;
                    } else {
                        return Err(Error::NodeProofsNotSupported);
                    }
                }

                // calculate sibling
                let (sib_pos, parent_pos) = {
                    let next_height = pos_height_in_tree(pos + 1);
                    let sibling_offset = sibling_offset(height);
                    if next_height > height {
                        // implies pos is right sibling
                        (pos - sibling_offset, pos + 1)
                    } else {
                        // pos is left sibling
                        (pos + sibling_offset, pos + parent_offset(height))
                    }
                };

                if Some(&sib_pos) == queue.front().map(|(pos, _)| pos) {
                    // drop sibling
                    queue.pop_front();
                } else {
                    proof.push(
                        store
                            .get(&sib_pos)
                            .map(|v| v.clone())
                            .ok_or(Error::InconsistentStore)?,
                    );
                }
                if parent_pos < peak_pos {
                    // save pos to tree buf
                    queue.push_back((parent_pos, height + 1));
                }
            }
            Ok(())
        }

        /// Generate merkle proof for positions
        /// 1. sort positions
        /// 2. push merkle proof to proof by peak from left to right
        /// 3. push bagged right hand side root
        pub fn gen_proof(
            &self,
            mut pos_list: Vec<u64>,
            store: &Map<T>,
        ) -> Result<MerkleProof<T, M>> {
            if pos_list.is_empty() {
                return Err(Error::GenProofForInvalidLeaves);
            }
            if self.mmr_size == 1 && pos_list == [0] {
                return Ok(MerkleProof::new(self.mmr_size, Vec::new()));
            }
            if pos_list.iter().any(|pos| pos_height_in_tree(*pos) > 0) {
                return Err(Error::NodeProofsNotSupported);
            }
            // ensure positions are sorted and unique
            pos_list.sort_unstable();
            pos_list.dedup();
            let peaks = get_peaks(self.mmr_size);
            let mut proof: Vec<T> = Vec::new();
            // generate merkle proof for each peaks
            let mut bagging_track = 0;
            for peak_pos in peaks {
                let pos_list: Vec<_> = take_while_vec(&mut pos_list, |&pos| pos <= peak_pos);
                if pos_list.is_empty() {
                    bagging_track += 1;
                } else {
                    bagging_track = 0;
                }
                self.gen_proof_for_peak(&mut proof, pos_list, peak_pos, store)?;
            }

            // ensure no remain positions
            if !pos_list.is_empty() {
                return Err(Error::GenProofForInvalidLeaves);
            }

            if bagging_track > 1 {
                let rhs_peaks = proof.split_off(proof.len() - bagging_track);
                proof.push(self.bag_rhs_peaks(rhs_peaks)?.expect("bagging rhs peaks"));
            }

            Ok(MerkleProof::new(self.mmr_size, proof))
        }
    }

    #[derive(Debug)]
    pub struct MerkleProof<T: Encode + Packed + EncodeLike, M> {
        pub mmr_size: u64,
        pub proof: Vec<T>,
        merge: PhantomData<M>,
    }

    impl<T: Clone + PartialEq + Encode + Packed + EncodeLike, M: Merge<Item = T>> MerkleProof<T, M> {
        pub fn new(mmr_size: u64, proof: Vec<T>) -> Self {
            MerkleProof {
                mmr_size,
                proof,
                merge: PhantomData,
            }
        }
    }

    fn take_while_vec<T, P: Fn(&T) -> bool>(v: &mut Vec<T>, p: P) -> Vec<T> {
        for i in 0..v.len() {
            if !p(&v[i]) {
                return v.drain(..i).collect();
            }
        }
        v.drain(..).collect()
    }
}

#[ink::contract]
pub mod state_aggregator {
    use ink::env::hash;
    use ink::prelude::collections::BTreeMap;
    use ink::prelude::vec;
    use ink::prelude::{format, string::String, vec::Vec};
    use ink::storage::Mapping;

    use ckb_merkle_mountain_range::{Merge, Result as MMRResult};

    use scale::{Decode, Encode};

    use crate::Map;

    #[derive(Decode, Encode, Debug)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub enum ConfigureArgument {
        SetOwner(AccountId),
        SetSnapshotDuration(u32),
        SetAuthorizedProvider(AccountId),
    }

    struct MergeKeccak;

    impl Merge for MergeKeccak {
        type Item = [u8; 32];
        fn merge(lhs: &Self::Item, rhs: &Self::Item) -> MMRResult<Self::Item> {
            let mut concat = vec![];
            concat.extend(lhs);
            concat.extend(rhs);

            let mut output = <hash::Keccak256 as hash::HashOutput>::Type::default();
            ink::env::hash_bytes::<hash::Keccak256>(&concat, &mut output);

            Ok(output.try_into().expect("INVALID_HASH_LENGTH"))
        }
    }

    #[derive(Decode, Encode, Debug)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub struct MerkleProof<T: scale::Decode + ink::storage::traits::Packed + scale::EncodeLike> {
        pub mmr_size: u64,
        pub proof: Vec<T>,
    }

    #[ink(event)]
    pub struct SnapshotFinalized {
        snapshot: u128,
        level: BlockNumber,
    }

    /// Errors returned by the contract's methods.
    #[derive(Debug, PartialEq, Eq, Encode, Decode)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub enum Error {
        NotAllowed,
        CouldNotGenerateProof(String),
        CouldNotInsert(String),
        InvalidBatchDuringInsert,
        CannotSnapshot,
    }

    #[ink::storage_item]
    #[derive(Debug)]
    pub struct Config {
        /// Multi-sig address allowed to manage the contract
        owner: AccountId,
        /// This constant defined how many levels each snapshot has
        snapshot_duration: u32,
        /// Authorized contract that can insert data
        acurast_contract: AccountId,
    }

    #[ink(storage)]
    pub struct StateAggregator {
        config: Config,
        snapshot_start_level: BlockNumber,
        snapshot_counter: u128,
        mmr_size: u64,
        tree: Map<[u8; 32]>,
        snapshot_level: Mapping<u128, u32>,
    }

    impl StateAggregator {
        #[ink(constructor)]
        pub fn new(owner: AccountId, snapshot_duration: u32) -> Self {
            let mut contract = Self::default();

            contract.config.owner = owner;
            contract.config.snapshot_duration = snapshot_duration;
            contract
        }

        #[ink(constructor)]
        pub fn default() -> Self {
            Self {
                config: Config {
                    owner: AccountId::from([0x0; 32]),
                    snapshot_duration: 5,
                    acurast_contract: AccountId::from([0x0; 32]),
                },
                snapshot_start_level: 0,
                snapshot_counter: 1,
                mmr_size: 0,
                tree: BTreeMap::new(),
                snapshot_level: Mapping::new(),
            }
        }

        fn fail_if_not_owner(&self) -> Result<(), Error> {
            if self.config.owner.eq(&self.env().caller()) {
                return Ok(());
            }
            Err(Error::NotAllowed)
        }

        fn finalize_snapshot(&mut self, required: bool) -> Result<(), Error> {
            let current_block_number = Self::env().block_number();

            if self.snapshot_start_level == 0 {
                // Start snapshot
                self.snapshot_start_level = current_block_number;
                self.tree.clear();
                self.mmr_size = 0;
            }

            if self.snapshot_start_level + self.config.snapshot_duration < current_block_number {
                // Finalize snapshot

                // Snapshot previous block level
                let snapshot_level = current_block_number - 1;
                self.snapshot_level
                    .insert(self.snapshot_counter, &snapshot_level);

                // Start new snapshot
                self.snapshot_start_level = current_block_number;
                self.tree.clear();
                self.mmr_size = 0;

                self.env().emit_event(SnapshotFinalized {
                    snapshot: self.snapshot_counter,
                    level: snapshot_level,
                });

                self.snapshot_counter += 1;
            } else if required {
                return Err(Error::CannotSnapshot);
            }

            Ok(())
        }

        #[ink(message)]
        pub fn configure(&mut self, configure: Vec<ConfigureArgument>) -> Result<(), Error> {
            // Only the administrator can configure the contract
            self.fail_if_not_owner()?;

            for c in configure {
                match c {
                    ConfigureArgument::SetOwner(address) => self.config.owner = address,
                    ConfigureArgument::SetSnapshotDuration(duration) => {
                        self.config.snapshot_duration = duration
                    }
                    ConfigureArgument::SetAuthorizedProvider(address) => {
                        self.config.acurast_contract = address
                    }
                }
            }

            Ok(())
        }

        #[ink(message)]
        pub fn snapshot(&mut self) -> Result<(), Error> {
            self.finalize_snapshot(true)
        }

        #[ink(message)]
        pub fn insert(&mut self, hash: [u8; 32]) -> crate::InsertReturn {
            // Check if the snapshot can be finalized
            self.finalize_snapshot(false)?;

            // Only the authorized contract can add data
            if self.config.acurast_contract != self.env().caller() {
                return Err(Error::NotAllowed);
            }

            let mut mmr = super::mmr::MMR::<[u8; 32], MergeKeccak>::new(self.mmr_size);

            let mut batch = mmr
                .push(&self.tree, hash)
                .map_err(|err| Error::CouldNotInsert(format!("{:?}", err)))?;

            if batch.len() != 1 {
                return Err(Error::InvalidBatchDuringInsert);
            }

            let mut leaf_index: u64 = 0;
            for (pos, elems) in batch.drain(..) {
                leaf_index = pos;
                for (i, elem) in elems.into_iter().enumerate() {
                    self.tree.insert(pos + i as u64, elem);
                }
            }
            self.mmr_size = mmr.mmr_size();

            Ok((leaf_index, self.snapshot_counter))
        }

        #[ink(message)]
        pub fn generate_proof(&self, positions: Vec<u64>) -> crate::GenerateProofReturn {
            let mmr = super::mmr::MMR::<[u8; 32], MergeKeccak>::new(self.mmr_size);

            let proof = mmr.gen_proof(positions, &self.tree);

            match proof {
                Err(err) => Err(Error::CouldNotGenerateProof(format!("{:?}", err))),
                Ok(proof) => Ok(MerkleProof {
                    mmr_size: proof.mmr_size,
                    proof: proof.proof,
                }),
            }
        }

        #[ink(message)]
        pub fn next_snapshot(&self) -> u128 {
            self.snapshot_counter
        }

        #[ink(message)]
        pub fn snapshot_level(&self, snapshot: u128) -> u32 {
            self.snapshot_level.get(snapshot).expect("UNKNOWN_SNAPSHOT")
        }

        #[ink(message)]
        pub fn snapshot_root(&self) -> [u8; 32] {
            let mmr = super::mmr::MMR::<[u8; 32], MergeKeccak>::new(self.mmr_size);

            mmr.get_root(&self.tree).expect("COULD_NOT_GET_ROOT")
        }

        #[ink(message)]
        pub fn can_snapshot(&self) -> bool {
            let current_block = Self::env().block_number();

            let not_empty = self.mmr_size > 0;

            not_empty && (self.snapshot_start_level + self.config.snapshot_duration) < current_block
        }
    }

    #[cfg(test)]
    mod tests {
        /// Imports all the definitions from the outer scope so we can use them here.
        use super::*;

        /// We test if the default constructor does its job.
        #[ink::test]
        fn test_constructor() {
            let accounts = ink::env::test::default_accounts::<ink::env::DefaultEnvironment>();
            let admin = accounts.alice;
            let snapshot_duration = 5;

            let validator = StateAggregator::new(admin.clone(), snapshot_duration);
            assert_eq!(validator.config.owner, admin);
            assert_eq!(validator.config.snapshot_duration, snapshot_duration);
        }

        #[ink::test]
        fn test_unauthorized_configure() {
            let accounts = ink::env::test::default_accounts::<ink::env::DefaultEnvironment>();
            let admin = accounts.alice;
            let data_provider = accounts.bob;
            let snapshot_duration = 5;

            let mut state_aggregator = StateAggregator::new(admin, snapshot_duration);

            ink::env::test::set_caller::<ink::env::DefaultEnvironment>(accounts.bob);

            // (Panic Expected) : Only the admin can call the configure method
            assert!(state_aggregator
                .configure(vec![ConfigureArgument::SetAuthorizedProvider(
                    data_provider,
                )])
                .is_err());
        }

        #[ink::test]
        fn test_authorized_configure() {
            let accounts = ink::env::test::default_accounts::<ink::env::DefaultEnvironment>();
            let admin = accounts.alice;
            let data_provider = accounts.bob;
            let snapshot_duration = 5;

            let leaf_index: Vec<u64> = (0..=1).collect();

            dbg!(leaf_index);

            let mut state_aggregator = StateAggregator::new(admin, snapshot_duration);

            ink::env::test::set_caller::<ink::env::DefaultEnvironment>(admin);

            // (Panic Expected) : Only the admin can call the configure method
            let _ = state_aggregator.configure(vec![ConfigureArgument::SetAuthorizedProvider(
                data_provider,
            )]);
        }

        #[ink::test]
        fn test_worflow() {
            let accounts = ink::env::test::default_accounts::<ink::env::DefaultEnvironment>();
            let admin = accounts.alice;
            let data_provider = accounts.bob;
            let snapshot_duration = 5;

            let mut state_aggregator = StateAggregator::new(admin, snapshot_duration);

            // Set data provider
            ink::env::test::set_caller::<ink::env::DefaultEnvironment>(admin);
            let _ = state_aggregator.configure(vec![ConfigureArgument::SetAuthorizedProvider(
                data_provider,
            )]);

            ink::env::test::advance_block::<ink::env::DefaultEnvironment>();

            // Insert data (First message)
            ink::env::test::set_caller::<ink::env::DefaultEnvironment>(data_provider);
            let data_hash = [0; 32];
            let _ = state_aggregator.insert(data_hash);
            assert_eq!(state_aggregator.mmr_size, 1);
            assert_eq!(state_aggregator.snapshot_root(), data_hash);
            assert_eq!(state_aggregator.snapshot_start_level, 1);
            assert_eq!(state_aggregator.snapshot_counter, 1);

            ink::env::test::advance_block::<ink::env::DefaultEnvironment>();

            let data_hash = [1; 32];
            let _ = state_aggregator.insert(data_hash);
            assert_eq!(state_aggregator.mmr_size, 3);
            assert_eq!(
                state_aggregator.snapshot_root(),
                [
                    213, 244, 247, 225, 217, 137, 132, 132, 128, 35, 111, 176, 165, 248, 8, 213,
                    135, 122, 191, 119, 131, 100, 174, 80, 132, 82, 52, 221, 108, 30, 128, 252
                ]
            );
            assert_eq!(state_aggregator.snapshot_start_level, 1);
            assert_eq!(state_aggregator.snapshot_counter, 1);

            // Snapshots have a duration of 5 blocks
            // Test that after 5 blocks, the next message will finalize the snapshot

            ink::env::test::advance_block::<ink::env::DefaultEnvironment>();
            ink::env::test::advance_block::<ink::env::DefaultEnvironment>();
            ink::env::test::advance_block::<ink::env::DefaultEnvironment>();
            ink::env::test::advance_block::<ink::env::DefaultEnvironment>();
            ink::env::test::advance_block::<ink::env::DefaultEnvironment>();

            let data_hash = [1; 32];
            let _ = state_aggregator.insert(data_hash);
            let data_hash = [2; 32];
            let _ = state_aggregator.insert(data_hash);
            assert_eq!(state_aggregator.mmr_size, 3);
            assert_eq!(
                state_aggregator.snapshot_root(),
                [
                    52, 109, 140, 150, 162, 69, 66, 19, 252, 192, 218, 255, 60, 150, 173, 3, 152,
                    20, 129, 129, 185, 250, 100, 136, 247, 174, 44, 10, 245, 178, 10, 160
                ]
            );
            assert_eq!(state_aggregator.snapshot_start_level, 7);
            assert_eq!(state_aggregator.snapshot_counter, 2);
        }
    }
}
