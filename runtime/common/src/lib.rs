#![cfg_attr(not(feature = "std"), no_std)]

pub use nimbus_primitives::NimbusId;
use pallet_acurast_marketplace::RegistrationExtra;
pub use parachains_common::Balance;
pub use sp_consensus_aura::sr25519::AuthorityId as AuraId;
use sp_core::H256;
#[cfg(any(feature = "std", test))]
pub use sp_runtime::BuildStorage;
use sp_runtime::{
	generic,
	traits::{BlakeTwo256, IdentifyAccount, Verify},
};
pub use sp_runtime::{MultiAddress, Perbill, Permill};
#[cfg(not(feature = "std"))]
use sp_std::alloc::string;
pub use xcm::{
	latest::{AssetId, MultiAsset},
	prelude::Fungible,
};

use acurast_p256_crypto::MultiSignature;
pub use pallet_acurast;
use pallet_acurast::CU32;

pub mod consensus;
pub mod constants;
// TODO: enable this again once we migrate Kusama to PoA -> PoS
// pub mod migrations;
pub mod weight;
pub mod weights;

/// Alias to 512-bit hash when used in the context of a transaction signature on the chain.
pub type Signature = MultiSignature;

/// Some way of identifying an account on the chain. We intentionally make it equivalent
/// to the public key of our transaction signing scheme.
pub type AccountId = <<Signature as Verify>::Signer as IdentifyAccount>::AccountId;

/// Index of a transaction in the chain.
pub type Nonce = u32;

/// A hash of some data used by the chain.
pub type Hash = H256;

/// An index to a block.
pub type BlockNumber = u32;

/// The address format for describing accounts.
pub type Address = MultiAddress<AccountId, ()>;

pub type MaxAllowedSources = pallet_acurast::CU32<1000>;
pub type MaxAllowedSourcesFor<T> = <T as pallet_acurast::Config>::MaxAllowedSources;
pub type MaxSlots = CU32<64>;
pub type MaxSlotsFor<T> = <T as pallet_acurast::Config>::MaxSlots;
pub type MaxEnvVars = CU32<10>;
pub type EnvKeyMaxSize = CU32<32>;
pub type EnvValueMaxSize = CU32<1024>;
pub type ExtraFor<T> = RegistrationExtra<Balance, AccountId, MaxSlotsFor<T>>;

// the base number of indivisible units for balances
pub const PICOUNIT: Balance = 1;
pub const NANOUNIT: Balance = 1_000;
pub const MICROUNIT: Balance = 1_000_000;
pub const MILLIUNIT: Balance = 1_000_000_000;
pub const UNIT: Balance = 1_000_000_000_000;
pub const KILOUNIT: Balance = 1_000_000_000_000_000;

/// Opaque types. These are used by the CLI to instantiate machinery that don't need to know
/// the specifics of the runtime. They can then be made to be agnostic over specific formats
/// of data like extrinsics, allowing for them to continue syncing the network through upgrades
/// to even the core data structures.
pub mod opaque {
	pub use sp_runtime::OpaqueExtrinsic as UncheckedExtrinsic;

	use super::*;

	/// Opaque block header type.
	pub type Header = generic::Header<BlockNumber, BlakeTwo256>;
	/// Opaque block type.
	pub type Block = generic::Block<Header, UncheckedExtrinsic>;
	/// Opaque block identifier type.
	pub type BlockId = generic::BlockId<Block>;
}

/// Stake information
pub mod staking_info {
	use pallet_parachain_staking::{inflation::Range, InflationInfoWithoutRound};
	use sp_runtime::Perbill;

	use crate::{Balance, UNIT};

	/// Minimum collators selected per round, default at genesis and minimum forever after
	pub const MINIMUM_SELECTED_CANDIDATES: u32 = 2; // TBD
	/// Minimum stake required to be reserved to be a candidate
	pub const MINIMUM_COLLATOR_STAKE: Balance = 20_000 * UNIT; // TBD
	/// Maximum top delegations per candidate
	pub const MAXIMUM_TOP_DELEGATIONS_PER_CANDIDATE: u32 = 300; // TBD
	/// Maximum bottom delegations per candidate
	pub const MAXIMUM_BOTTOM_DELEGATIONS_PER_CANDIDATE: u32 = 50; // TBD
	/// Maximum delegations per delegator
	pub const MAXIMUM_DELEGATIONS_PER_DELEGATOR: u32 = 100; // TBD
	/// Minimum stake required to be reserved to be a delegator
	pub const MINIMUM_DELEGATION: Balance = 10 * UNIT; // TBD
	pub const MINIMUM_DELEGATOR_STAKE: Balance = 10 * UNIT; // TBD

	pub const DEFAULT_INFLATION_CONFIG: InflationInfoWithoutRound = InflationInfoWithoutRound {
		ideal_staked: Perbill::from_percent(75),
		decay_rate: Perbill::from_percent(5),
		annual: Range { min: Perbill::from_percent(2), ideal: Perbill::from_percent(10) },
	};
}

pub mod utils {
	use pallet_acurast::{Attestation, VerifiedBootState};
	use sp_std::prelude::*;

	pub fn check_attestation(
		attestation: &Attestation,
		allowed_package_names: &[&[u8]],
		allowed_signature_digests: &[&[u8]],
	) -> bool {
		let mut result = false;
		let root_of_trust = &attestation.key_description.tee_enforced.root_of_trust;
		if let Some(root_of_trust) = root_of_trust {
			if root_of_trust.verified_boot_state == VerifiedBootState::Verified {
				let attestation_application_id = attestation
					.key_description
					.tee_enforced
					.attestation_application_id
					.as_ref()
					.or(attestation
						.key_description
						.software_enforced
						.attestation_application_id
						.as_ref());

				if let Some(attestation_application_id) = attestation_application_id {
					let package_names = attestation_application_id
						.package_infos
						.iter()
						.map(|package_info| package_info.package_name.as_slice())
						.collect::<Vec<_>>();
					let is_package_name_allowed = package_names
						.iter()
						.all(|package_name| allowed_package_names.contains(package_name));
					if is_package_name_allowed {
						let signature_digests = attestation_application_id
							.signature_digests
							.iter()
							.map(|signature_digest| signature_digest.as_slice())
							.collect::<Vec<_>>();
						result = signature_digests
							.iter()
							.all(|digest| allowed_signature_digests.contains(digest));
					}
				}
			}
		}

		return result
	}
}
