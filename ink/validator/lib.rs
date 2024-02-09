#![cfg_attr(not(feature = "std"), no_std, no_main)]

use ink::env::call::Selector;

pub use validator::Error;

// Method selectors

pub const VERIFY_PROOF_SELECTOR: Selector = Selector::new(ink::selector_bytes!("verify_proof"));

// Method types

pub type VerifyProofReturn = Result<bool, validator::Error>;

#[ink::contract]
pub mod validator {
    use ink::env::hash;
    use ink::prelude::vec;
    use ink::prelude::{format, string::String, vec::Vec};
    use ink::storage::{traits::Packed, Mapping};
    use scale::{Decode, Encode, EncodeLike};

    use ckb_merkle_mountain_range::{Error as MMRError, Merge, MerkleProof as MMRMerkleProof};

    struct MergeKeccak;

    impl Merge for MergeKeccak {
        type Item = [u8; 32];
        fn merge(lhs: &Self::Item, rhs: &Self::Item) -> Result<Self::Item, MMRError> {
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
    pub struct LeafProof {
        pub leaf_index: u64,
        pub data: Vec<u8>,
    }

    #[derive(Decode, Encode, Debug)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub struct MerkleProof<T: Decode + Packed + EncodeLike> {
        pub mmr_size: u64,
        pub proof: Vec<T>,
        pub leaves: Vec<LeafProof>,
    }

    const MAX_VALIDATORS: usize = 50;

    #[derive(Decode, Encode, Debug)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub enum ConfigureArgument {
        SetOwner(AccountId),
        SetValidators(Vec<AccountId>),
        SetMinimumEndorsements(u16),
    }

    /// Errors returned by the contract's methods.
    #[derive(Debug, PartialEq, Eq, Encode, Decode)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub enum Error {
        ProofInvalid(String),
        SnapshotUnknown,
        SnapshotInvalid,
        NotAllowed,
    }

    /// A custom type that we can use in our contract storage
    #[ink::storage_item]
    #[derive(Debug)]
    pub struct Config {
        /// Multi-sig address allowed to manage the contract
        owner: AccountId,
        /// Minimum expected endorsements for a given state root to be considered valid
        minimum_endorsements: u16,
        /// Validators
        validators: Vec<AccountId>,
    }

    /// Defines the storage of your contract.
    /// Add new fields to the below struct in order
    /// to add new static storage fields to your contract.
    #[ink(storage)]
    pub struct Validator {
        config: Config,
        current_snapshot: u128,
        root: Mapping<u128, [u8; 32]>,
        snapshot_submissions: Mapping<AccountId, [u8; 32]>,
        snapshot_submissions_accounts: Vec<AccountId>,
    }

    impl Validator {
        #[ink(constructor)]
        pub fn new(
            admin: AccountId,
            minimum_endorsements: u16,
            mut validators: Vec<AccountId>,
        ) -> Self {
            assert!(validators.len() <= MAX_VALIDATORS, "TOO_MANY_VALIDATORS");
            assert!(minimum_endorsements > 0, "NON_ZERO_ENDORSEMENTS");

            let mut contract = Self::default();
            validators.sort_unstable();
            validators.dedup();

            contract.config.validators = validators;
            contract.config.owner = admin;
            contract.config.minimum_endorsements = minimum_endorsements;
            contract
        }

        #[ink(constructor)]
        pub fn default() -> Self {
            Self {
                config: Config {
                    owner: AccountId::from([0x0; 32]),
                    minimum_endorsements: 0,
                    validators: vec![],
                },
                current_snapshot: 1,
                root: Default::default(),
                snapshot_submissions: Default::default(),
                snapshot_submissions_accounts: Default::default(),
            }
        }

        fn fail_if_not_validator(&self) -> Result<(), Error> {
            if self.config.validators.contains(&self.env().caller()) {
                return Ok(());
            }
            Err(Error::NotAllowed)
        }

        fn validate_block_state_root(&self) -> Option<[u8; 32]> {
            let mut endorsements_per_root: Mapping<[u8; 32], u128> = Default::default();
            let mut candidate_roots = vec![];

            for account in self.snapshot_submissions_accounts.iter() {
                if let Some(hash) = self.snapshot_submissions.get(account) {
                    let submissions = endorsements_per_root.get(hash).unwrap_or(0);
                    endorsements_per_root.insert(hash, &(submissions + 1));

                    if !candidate_roots.contains(&hash) {
                        candidate_roots.push(hash);
                    }
                }
            }

            let mut selected_candidate: [u8; 32] = [0; 32];
            let mut selected_candidade_submissions = 0;
            for candidate in candidate_roots {
                let submissions = endorsements_per_root.get(candidate).unwrap_or(0);
                if u128::from(selected_candidade_submissions) < submissions {
                    selected_candidate = candidate;
                    selected_candidade_submissions = submissions;
                }
            }

            if selected_candidade_submissions < self.config.minimum_endorsements.into() {
                return None;
            }

            Some(selected_candidate)
        }

        fn fail_if_not_owner(&self) -> Result<(), Error> {
            if self.config.owner.eq(&self.env().caller()) {
                return Ok(());
            }
            Err(Error::NotAllowed)
        }

        #[ink(message)]
        pub fn configure(&mut self, configure: Vec<ConfigureArgument>) -> Result<(), Error> {
            // Only the administrator can configure the contract
            self.fail_if_not_owner()?;

            for c in configure {
                match c {
                    ConfigureArgument::SetOwner(address) => self.config.owner = address,
                    ConfigureArgument::SetMinimumEndorsements(minimum_endorsements) => {
                        self.config.minimum_endorsements = minimum_endorsements
                    }
                    ConfigureArgument::SetValidators(validators) => {
                        self.config.validators = validators
                    }
                }
            }

            Ok(())
        }

        #[ink(message)]
        pub fn submit_root(&mut self, snapshot: u128, root: [u8; 32]) -> Result<(), Error> {
            let caller = self.env().caller();

            // Check if sender is a validator
            Self::fail_if_not_validator(self)?;

            // Make sure the snapshots are submitted sequencially
            if self.current_snapshot != snapshot {
                return Err(Error::SnapshotInvalid);
            }

            if !self.snapshot_submissions.contains(caller) {
                self.snapshot_submissions_accounts.push(caller);
            }

            // Store the root per validator
            self.snapshot_submissions.insert(caller, &root);

            // Finalize snapshot if consensus has been reached
            let can_finalize_snapshot = Self::validate_block_state_root(self);

            if can_finalize_snapshot.is_some() {
                self.root.insert(self.current_snapshot, &root);
                self.current_snapshot += 1;
                self.snapshot_submissions = Default::default();
            }

            Ok(())
        }

        //
        // Views
        //

        #[ink(message)]
        pub fn verify_proof(
            &self,
            snapshot: u128,
            proof: MerkleProof<[u8; 32]>,
        ) -> crate::VerifyProofReturn {
            // Get snapshot root
            let snaptshot_root = self.root.get(snapshot).ok_or(Error::SnapshotUnknown)?;

            // Prepare proof instance
            let mmr_proof =
                MMRMerkleProof::<[u8; 32], MergeKeccak>::new(proof.mmr_size, proof.proof);

            // Derive root from proof and leaves
            let hashed_leaves: Vec<(u64, [u8; 32])> = proof
                .leaves
                .iter()
                .map(|item| {
                    let mut hash = <hash::Keccak256 as hash::HashOutput>::Type::default();
                    ink::env::hash_bytes::<hash::Keccak256>(&item.data, &mut hash);

                    match <[u8; 32]>::try_from(hash) {
                        Ok(h) => Ok((item.leaf_index, h)),
                        Err(err) => Err(Error::ProofInvalid(format!("{:?}", err))),
                    }
                })
                .collect::<Result<Vec<(u64, [u8; 32])>, Error>>()?;

            match mmr_proof.calculate_root(hashed_leaves) {
                Err(err) => Err(Error::ProofInvalid(format!("{:?}", err))),
                Ok(derived_root) => {
                    // Check if the derived proof matches the one from the snapshot
                    Ok(snaptshot_root == derived_root)
                }
            }
        }

        #[ink(message)]
        pub fn current_snapshot(&self) -> u128 {
            self.current_snapshot
        }

        #[ink(message)]
        pub fn snapshot_submissions_accounts(&self) -> Vec<AccountId> {
            self.snapshot_submissions_accounts.clone()
        }
    }

    #[cfg(test)]
    mod tests {
        use hex_literal::hex;

        /// Imports all the definitions from the outer scope so we can use them here.
        use super::*;

        #[ink::test]
        fn test_constructor() {
            let accounts = ink::env::test::default_accounts::<ink::env::DefaultEnvironment>();
            let admin = accounts.alice;
            let minimum_endorsements: u16 = 1;
            let validators: Vec<AccountId> = vec![admin];

            let validator = Validator::new(
                admin.clone(),
                minimum_endorsements.clone(),
                validators.clone(),
            );
            assert_eq!(validator.config.owner, admin);
            assert_eq!(validator.config.minimum_endorsements, minimum_endorsements);
            assert_eq!(validator.config.validators, validators);
            assert_eq!(validator.current_snapshot, 1);
        }

        #[ink::test]
        fn test_submit_root() {
            let accounts = ink::env::test::default_accounts::<ink::env::DefaultEnvironment>();
            let admin = accounts.alice;
            let minimum_endorsements: u16 = 1;
            let validators: Vec<AccountId> = vec![admin];

            let mut validator = Validator::new(admin, minimum_endorsements, validators);

            let snapshot_number = 1;
            let snapshot_root = [0; 32];

            ink::env::test::set_caller::<ink::env::DefaultEnvironment>(admin);
            let _ = validator.submit_root(snapshot_number, snapshot_root);

            assert_eq!(validator.current_snapshot, 2);
            assert_eq!(validator.snapshot_submissions_accounts.len(), 1);
            assert_eq!(validator.root.get(snapshot_number), Some(snapshot_root));

            assert_eq!(validator.validate_block_state_root(), Some(snapshot_root));
        }

        /// We test a simple use case of our contract.
        #[ink::test]
        fn test_verify_proof() {
            let accounts = ink::env::test::default_accounts::<ink::env::DefaultEnvironment>();
            let admin = accounts.alice;
            let minimum_endorsements: u16 = 1;
            let validators: Vec<AccountId> = vec![admin];

            let mut validator = Validator::new(admin, minimum_endorsements, validators);

            let snapshot_number = 1;
            let snapshot_root =
                hex!("f805950edaf6f0ee75cf7ba469c2ea381667f1b75d5bfacf1749500448019049");

            ink::env::test::set_caller::<ink::env::DefaultEnvironment>(admin);
            let _ = validator.submit_root(snapshot_number, snapshot_root);

            let proof = MerkleProof {
                mmr_size: 3,
                proof: vec![hex!(
                    "79dd2180cc76e44fd7d3b6d1c89b9dfae07800741f7d36837d64bedd7300ed2e"
                )],
                leaves: vec![
                    LeafProof {
                        leaf_index: 1,
                        data: hex!("000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000ff000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000000600000000000000000000000000000000000000000000000000000000000000000").to_vec()
                    }
                ],
            };

            let proof_validation = validator.verify_proof(snapshot_number, proof);

            assert_eq!(proof_validation, Ok(true));
        }
    }
}
