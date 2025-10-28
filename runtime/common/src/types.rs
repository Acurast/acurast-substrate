mod tracks_info;
mod transaction_charger;

pub use parachains_common::Balance;
pub use sp_consensus_aura::sr25519::AuthorityId as AuraId;
use sp_core::H256;
use sp_runtime::{
	traits::{IdentifyAccount, Verify},
	MultiAddress,
};

use acurast_p256_crypto::MultiSignature;
use pallet_acurast::CU32;
use pallet_acurast_marketplace::RegistrationExtra;

pub use tracks_info::*;
pub use transaction_charger::*;

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
pub type ProcessorVersionFor<T> = <T as pallet_acurast::Config>::ProcessorVersion;
pub type MaxVersions = CU32<2>;
pub type MaxVersionsFor<T> = <T as pallet_acurast::Config>::MaxVersions;
pub type MaxEnvVars = CU32<10>;
pub type EnvKeyMaxSize = CU32<32>;
pub type EnvValueMaxSize = CU32<1024>;
pub type ExtraFor<T> = RegistrationExtra<
	Balance,
	AccountId,
	MaxSlotsFor<T>,
	ProcessorVersionFor<T>,
	MaxVersionsFor<T>,
>;
pub type CouncilInstance = pallet_collective::Instance1;
pub type CouncilMembershipInstance = pallet_membership::Instance1;
pub type CouncilThreeSeventh =
	pallet_collective::EnsureProportionAtLeast<AccountId, CouncilInstance, 3, 7>;
pub type CouncilMajority =
	pallet_collective::EnsureProportionAtLeast<AccountId, CouncilInstance, 1, 2>;
pub type CouncilTwoThirds =
	pallet_collective::EnsureProportionAtLeast<AccountId, CouncilInstance, 2, 3>;
pub type CouncilUnanimous =
	pallet_collective::EnsureProportionAtLeast<AccountId, CouncilInstance, 1, 1>;

pub type TreasuryInstance = pallet_treasury::Instance1;
pub type OperationFundsInstance = pallet_treasury::Instance2;
pub type LiquidityFundsInstance = pallet_treasury::Instance3;
pub type ExtraFundsInstance = pallet_treasury::Instance4;
