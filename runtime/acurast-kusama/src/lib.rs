#![cfg_attr(not(feature = "std"), no_std)]
// `construct_runtime!` does a lot of recursion and requires us to increase the limit to 256.
#![recursion_limit = "256"]

// Make the WASM binary available.
#[cfg(feature = "std")]
include!(concat!(env!("OUT_DIR"), "/wasm_binary.rs"));

pub mod xcm_config;

#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarking;

use core::{default::Default, marker::PhantomData};

use cumulus_pallet_parachain_system::RelayNumberStrictlyIncreases;
use frame_support::traits::{ConstBool, ConstU128, ConstU32};
use smallvec::smallvec;
use sp_api::impl_runtime_apis;
use sp_core::{crypto::KeyTypeId, OpaqueMetadata, H256};
use sp_runtime::{
	create_runtime_str, generic, impl_opaque_keys,
	traits::{
		AccountIdLookup, BlakeTwo256, Block as BlockT, DispatchInfoOf, PostDispatchInfoOf, Zero,
	},
	transaction_validity::{TransactionSource, TransactionValidity},
	ApplyExtrinsicResult, DispatchError,
};

use sp_std::prelude::*;
#[cfg(feature = "std")]
use sp_version::NativeVersion;
use sp_version::RuntimeVersion;

use derive_more::{From, Into};
use frame_support::{
	construct_runtime,
	dispatch::{DispatchClass, DispatchResultWithPostInfo},
	ord_parameter_types,
	pallet_prelude::InvalidTransaction,
	parameter_types,
	traits::{
		fungible::{Inspect, Mutate},
		nonfungibles::{Create, InspectEnumerable as NFTInspectEnumerable},
		AsEnsureOriginWithArg, Currency, ExistenceRequirement, Imbalance, OnUnbalanced,
		WithdrawReasons,
	},
	unsigned::TransactionValidityError,
	weights::{
		constants::WEIGHT_REF_TIME_PER_SECOND, ConstantMultiplier, Weight, WeightToFeeCoefficient,
		WeightToFeeCoefficients, WeightToFeePolynomial,
	},
	PalletId,
};
use frame_system::{
	limits::{BlockLength, BlockWeights},
	EnsureRoot, EnsureRootWithSuccess, EnsureSignedBy, EnsureWithSuccess,
};
use sp_runtime::AccountId32;
pub use sp_runtime::{MultiAddress, Perbill, Permill};
use xcm_config::{XcmConfig, XcmOriginToTransactDispatchOrigin};

#[cfg(any(feature = "std", test))]
pub use sp_runtime::BuildStorage;

// Polkadot imports
use polkadot_runtime_common::{BlockHashCount, SlowAdjustingFeeUpdate};

use weights::{BlockExecutionWeight, ExtrinsicBaseWeight, RocksDbWeight};

// XCM Imports
use xcm::latest::prelude::BodyId;
use xcm_executor::XcmExecutor;

pub use parachains_common::Balance;

use frame_support::traits::{
	tokens::{Fortitude, Precision, Preservation},
	EitherOfDiverse,
};

/// Acurast Imports
use acurast_p256_crypto::MultiSignature;
use acurast_runtime_common::*;

use pallet_acurast::{JobId, MultiOrigin, CU32};
use pallet_acurast_hyperdrive::{
	instances::{AlephZeroInstance, EthereumInstance, HyperdriveInstance, TezosInstance},
	ParsedAction, StateOwner,
};
use pallet_acurast_hyperdrive_outgoing::{
	chain::tezos::DefaultTezosConfig, Action, LeafIndex, MMRError, SnapshotNumber, TargetChainProof,
};
pub use pallet_acurast_marketplace;
use pallet_acurast_marketplace::{
	MarketplaceHooks, PartialJobRegistration, PubKey, PubKeys, RuntimeApiError,
};
pub use pallet_acurast_processor_manager;
use sp_runtime::traits::{AccountIdConversion, NumberFor};

/// Wrapper around [`AccountId32`] to allow the implementation of [`TryFrom<Vec<u8>>`].
#[derive(Debug, From, Into, Clone, Eq, PartialEq)]
pub struct AcurastAccountId(AccountId32);
impl TryFrom<Vec<u8>> for AcurastAccountId {
	type Error = ();

	fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
		let a: [u8; 32] = value.try_into().map_err(|_| ())?;
		Ok(AcurastAccountId(AccountId32::new(a)))
	}
}

/// Block type as expected by this runtime.
pub type Block = generic::Block<opaque::Header, UncheckedExtrinsic>;

/// A Block signed with a Justification
pub type SignedBlock = generic::SignedBlock<Block>;

/// BlockId type as expected by this runtime.
pub type BlockId = generic::BlockId<Block>;

/// The SignedExtension to the basic transaction logic.
pub type SignedExtra = (
	frame_system::CheckNonZeroSender<Runtime>,
	frame_system::CheckSpecVersion<Runtime>,
	frame_system::CheckTxVersion<Runtime>,
	frame_system::CheckGenesis<Runtime>,
	frame_system::CheckEra<Runtime>,
	frame_system::CheckNonce<Runtime>,
	frame_system::CheckWeight<Runtime>,
	pallet_transaction_payment::ChargeTransactionPayment<Runtime>,
);

/// Unchecked extrinsic type as expected by this runtime.
pub type UncheckedExtrinsic =
	generic::UncheckedExtrinsic<Address, RuntimeCall, Signature, SignedExtra>;

/// Extrinsic type that has already been checked.
pub type CheckedExtrinsic = generic::CheckedExtrinsic<AccountId, RuntimeCall, SignedExtra>;

/// Executive: handles dispatch to the various modules.
pub type Executive = frame_executive::Executive<
	Runtime,
	Block,
	frame_system::ChainContext<Runtime>,
	Runtime,
	AllPalletsWithSystem,
>;

type MaxEnvVars = CU32<10>;
type EnvKeyMaxSize = CU32<32>;
type EnvValueMaxSize = CU32<1024>;
/// Handles converting a weight scalar to a fee value, based on the scale and granularity of the
/// node's balance type.
///
/// This should typically create a mapping between the following ranges:
///   - `[0, MAXIMUM_BLOCK_WEIGHT]`
///   - `[Balance::min, Balance::max]`
///
/// Yet, it can be used for any other sort of change to weight-fee. Some examples being:
///   - Setting it to `0` will essentially disable the weight fee.
///   - Setting it to `1` will cause the literal `#[weight = x]` values to be charged.
pub struct WeightToFee;
impl WeightToFeePolynomial for WeightToFee {
	type Balance = Balance;
	fn polynomial() -> WeightToFeeCoefficients<Self::Balance> {
		// in Kusama, extrinsic base weight (smallest non-zero weight) is mapped to 1 MILLIUNIT:
		// for acurast, we map to 1/10 of that, or 1/10 MILLIUNIT
		let p = MILLIUNIT / 10;
		let q = 100 * Balance::from(ExtrinsicBaseWeight::get().ref_time());
		smallvec![WeightToFeeCoefficient {
			degree: 1,
			negative: false,
			coeff_frac: Perbill::from_rational(p % q, q),
			coeff_integer: p / q,
		}]
	}
}

impl_opaque_keys! {
	pub struct SessionKeys {
		pub aura: Aura,
	}
}

#[sp_version::runtime_version]
pub const VERSION: RuntimeVersion = RuntimeVersion {
	spec_name: create_runtime_str!("acurast-parachain"),
	impl_name: create_runtime_str!("acurast-parachain"),
	authoring_version: 1,
	spec_version: 11,
	impl_version: 1,
	apis: RUNTIME_API_VERSIONS,
	transaction_version: 1,
	state_version: 1,
};

/// This determines the average expected block time that we are targeting.
/// Blocks will be produced at a minimum duration defined by `SLOT_DURATION`.
/// `SLOT_DURATION` is picked up by `pallet_timestamp` which is in turn picked
/// up by `pallet_aura` to implement `fn slot_duration()`.
///
/// Change this to adjust the block time.
pub const MILLISECS_PER_BLOCK: u64 = 12000;

// NOTE: Currently it is not possible to change the slot duration after the chain has started.
//       Attempting to do so will brick block production.
pub const SLOT_DURATION: u64 = MILLISECS_PER_BLOCK;

// Time is measured by number of blocks.
pub const MINUTES: BlockNumber = 60_000 / (MILLISECS_PER_BLOCK as BlockNumber);
pub const HOURS: BlockNumber = MINUTES * 60;
pub const DAYS: BlockNumber = HOURS * 24;

// Provide a common factor between runtimes based on a supply of 1_000_000_000_000 tokens == 1 UNIT.
pub const SUPPLY_FACTOR: Balance = 1;

pub const STORAGE_BYTE_FEE: Balance = 100 * MICROUNIT * SUPPLY_FACTOR;

pub const fn deposit(items: u32, bytes: u32) -> Balance {
	items as Balance * 1 * UNIT * SUPPLY_FACTOR + (bytes as Balance) * STORAGE_BYTE_FEE
}

/// The existential deposit. Set to 1/10 of the Connected Relay Chain.
pub const EXISTENTIAL_DEPOSIT: Balance = MILLIUNIT;

/// We assume that ~5% of the block weight is consumed by `on_initialize` handlers. This is
/// used to limit the maximal weight of a single extrinsic.
const AVERAGE_ON_INITIALIZE_RATIO: Perbill = Perbill::from_percent(5);

/// We allow `Normal` extrinsics to fill up the block up to 75%, the rest can be used by
/// `Operational` extrinsics.
const NORMAL_DISPATCH_RATIO: Perbill = Perbill::from_percent(75);

/// We allow for 0.5 of a second of compute with a 12 second average block time.
const MAXIMUM_BLOCK_WEIGHT: Weight = Weight::from_parts(
	WEIGHT_REF_TIME_PER_SECOND.saturating_div(2),
	cumulus_primitives_core::relay_chain::MAX_POV_SIZE as u64,
);

/// The version information used to identify this runtime when compiled natively.
#[cfg(feature = "std")]
pub fn native_version() -> NativeVersion {
	NativeVersion { runtime_version: VERSION, can_author_with: Default::default() }
}

parameter_types! {
	pub const Version: RuntimeVersion = VERSION;

	// This part is copied from Substrate's `bin/node/runtime/src/lib.rs`.
	//  The `RuntimeBlockLength` and `RuntimeBlockWeights` exist here because the
	// `DeletionWeightLimit` and `DeletionQueueDepth` depend on those to parameterize
	// the lazy contract deletion.
	pub RuntimeBlockLength: BlockLength =
		BlockLength::max_with_normal_ratio(5 * 1024 * 1024, NORMAL_DISPATCH_RATIO);
	pub RuntimeBlockWeights: BlockWeights = BlockWeights::builder()
		.base_block(BlockExecutionWeight::get())
		.for_class(DispatchClass::all(), |weights| {
			weights.base_extrinsic = ExtrinsicBaseWeight::get();
		})
		.for_class(DispatchClass::Normal, |weights| {
			weights.max_total = Some(NORMAL_DISPATCH_RATIO * MAXIMUM_BLOCK_WEIGHT);
		})
		.for_class(DispatchClass::Operational, |weights| {
			weights.max_total = Some(MAXIMUM_BLOCK_WEIGHT);
			// Operational transactions have some extra reserved space, so that they
			// are included even if block reached `MAXIMUM_BLOCK_WEIGHT`.
			weights.reserved = Some(
				MAXIMUM_BLOCK_WEIGHT - NORMAL_DISPATCH_RATIO * MAXIMUM_BLOCK_WEIGHT
			);
		})
		.avg_block_initialization(AVERAGE_ON_INITIALIZE_RATIO)
		.build_or_panic();
	pub const SS58Prefix: u16 = 42;
}

// Configure FRAME pallets to include in runtime.

/// Extrinsic Call Filter
pub struct KusamaCallFilter;
impl frame_support::traits::Contains<RuntimeCall> for KusamaCallFilter {
	fn contains(c: &RuntimeCall) -> bool {
		match c {
			// We dont allow (non ROOT) calls to the pallet_balances while the tokenomics are not ready
			RuntimeCall::Balances(..) => false,
			_ => true,
		}
	}
}

impl frame_system::Config for Runtime {
	/// The identifier used to distinguish between accounts.
	type AccountId = AccountId;
	/// The aggregated dispatch type that is available for extrinsics.
	type RuntimeCall = RuntimeCall;
	/// The lookup mechanism to get account ID from whatever is passed in dispatchers.
	type Lookup = AccountIdLookup<AccountId, ()>;
	/// The index type for storing how many extrinsics an account has signed.
	type Nonce = Nonce;
	/// The block type.
	type Block = Block;
	/// The type for hashing blocks and tries.
	type Hash = Hash;
	/// The hashing algorithm used.
	type Hashing = BlakeTwo256;
	/// The ubiquitous event type.
	type RuntimeEvent = RuntimeEvent;
	/// The ubiquitous origin type.
	type RuntimeOrigin = RuntimeOrigin;
	/// Maximum number of block number to block hash mappings to keep (oldest pruned first).
	type BlockHashCount = BlockHashCount;
	/// Runtime version.
	type Version = Version;
	/// Converts a module to an index of this module in the runtime.
	type PalletInfo = PalletInfo;
	/// The data to be stored in an account.
	type AccountData = pallet_balances::AccountData<Balance>;
	/// What to do if a new account is created.
	type OnNewAccount = ();
	/// What to do if an account is fully reaped from the system.
	type OnKilledAccount = ();
	/// The weight of database operations that the runtime can invoke.
	type DbWeight = RocksDbWeight;
	/// The basic call filter to use in dispatchable.
	type BaseCallFilter = KusamaCallFilter;
	/// Weight information for the extrinsics of this pallet.
	type SystemWeightInfo = ();
	/// Block & extrinsics weights: base values and limits.
	type BlockWeights = RuntimeBlockWeights;
	/// The maximum length of a block (in bytes).
	type BlockLength = RuntimeBlockLength;
	/// This is used as an identifier of the chain. 42 is the generic substrate prefix.
	type SS58Prefix = SS58Prefix;
	/// The action to take on a Runtime Upgrade
	type OnSetCode = cumulus_pallet_parachain_system::ParachainSetCode<Self>;
	type MaxConsumers = frame_support::traits::ConstU32<16>;
}

parameter_types! {
	pub const MinimumPeriod: u64 = SLOT_DURATION / 2;
}

/// Runtime configuration for pallet_timestamp.
impl pallet_timestamp::Config for Runtime {
	/// A timestamp: milliseconds since the unix epoch.
	type Moment = u64;
	type OnTimestampSet = ();
	type MinimumPeriod = MinimumPeriod;
	type WeightInfo = weight::pallet_timestamp::WeightInfo<Runtime>;
}

parameter_types! {
	pub const UncleGenerations: u32 = 0;
}

/// Runtime configuration for pallet_authorship.
impl pallet_authorship::Config for Runtime {
	type FindAuthor = pallet_session::FindAccountFromAuthorIndex<Self, Aura>;
	type EventHandler = (CollatorSelection,);
}

parameter_types! {
	pub const ExistentialDeposit: Balance = EXISTENTIAL_DEPOSIT;
	pub const MaxLocks: u32 = 50;
	pub const MaxReserves: u32 = 50;
}

/// Runtime configuration for pallet_balances.
impl pallet_balances::Config for Runtime {
	type MaxLocks = MaxLocks;
	/// The type for recording an account's balance.
	type Balance = Balance;
	/// The ubiquitous event type.
	type RuntimeEvent = RuntimeEvent;
	type DustRemoval = ();
	type ExistentialDeposit = ExistentialDeposit;
	type AccountStore = System;
	type WeightInfo = weight::pallet_balances::WeightInfo<Runtime>;
	type MaxReserves = MaxReserves;
	type ReserveIdentifier = [u8; 8];
	type RuntimeHoldReason = RuntimeHoldReason;
	type FreezeIdentifier = ();
	type MaxHolds = ConstU32<{ u32::MAX }>;
	type MaxFreezes = ConstU32<0>;
}

parameter_types! {
	/// Relay Chain `TransactionByteFee` / 10
	pub const TransactionByteFee: Balance = 10 * MICROUNIT;
	pub const OperationalFeeMultiplier: u8 = 5;
}

type NegativeImbalanceOf<C, T> =
	<C as Currency<<T as frame_system::Config>::AccountId>>::NegativeImbalance;

pub struct LiquidityInfo {
	pub imbalance: Option<NegativeImbalanceOf<Balances, Runtime>>,
	pub fee_payer: Option<<Runtime as frame_system::Config>::AccountId>,
}

impl Default for LiquidityInfo {
	fn default() -> Self {
		Self { imbalance: None, fee_payer: None }
	}
}

pub struct TransactionCharger<OU>(PhantomData<OU>);
impl<OU> pallet_transaction_payment::OnChargeTransaction<Runtime> for TransactionCharger<OU>
where
	OU: OnUnbalanced<NegativeImbalanceOf<Balances, Runtime>>,
{
	type Balance = Balance;
	type LiquidityInfo = Option<LiquidityInfo>;

	fn withdraw_fee(
		who: &<Runtime as frame_system::Config>::AccountId,
		call: &<Runtime as frame_system::Config>::RuntimeCall,
		_dispatch_info: &DispatchInfoOf<<Runtime as frame_system::Config>::RuntimeCall>,
		fee: Self::Balance,
		tip: Self::Balance,
	) -> Result<Self::LiquidityInfo, TransactionValidityError> {
		if fee.is_zero() {
			return Ok(None)
		}

		let withdraw_reason = if tip.is_zero() {
			WithdrawReasons::TRANSACTION_PAYMENT
		} else {
			WithdrawReasons::TRANSACTION_PAYMENT | WithdrawReasons::TIP
		};

		let mut manager = AcurastProcessorManager::manager_for_processor(who);

		if manager.is_none() {
			if let RuntimeCall::AcurastProcessorManager(
				pallet_acurast_processor_manager::Call::pair_with_manager { pairing },
			) = call
			{
				if pairing.validate_timestamp::<Runtime>() {
					let counter = AcurastProcessorManager::counter_for_manager(&pairing.account)
						.unwrap_or(0)
						.checked_add(1);
					if let Some(counter) = counter {
						if pairing.validate_signature::<Runtime>(&pairing.account, counter) {
							manager = Some(pairing.account.clone());
						}
					}
				}
			}
		}

		let fee_payer = manager.unwrap_or(who.clone());

		match Balances::withdraw(&fee_payer, fee, withdraw_reason, ExistenceRequirement::KeepAlive)
		{
			Ok(imbalance) =>
				Ok(Some(LiquidityInfo { imbalance: Some(imbalance), fee_payer: Some(fee_payer) })),
			Err(_) => Err(InvalidTransaction::Payment.into()),
		}
	}

	fn correct_and_deposit_fee(
		who: &<Runtime as frame_system::Config>::AccountId,
		_dispatch_info: &DispatchInfoOf<<Runtime as frame_system::Config>::RuntimeCall>,
		_post_info: &PostDispatchInfoOf<<Runtime as frame_system::Config>::RuntimeCall>,
		corrected_fee: Self::Balance,
		tip: Self::Balance,
		info: Self::LiquidityInfo,
	) -> Result<(), TransactionValidityError> {
		if let Some(LiquidityInfo { imbalance, fee_payer }) = info {
			if let Some(paid) = imbalance {
				let fee_payer = fee_payer.as_ref().unwrap_or(who);
				// Calculate how much refund we should return
				let refund_amount = paid.peek().saturating_sub(corrected_fee);
				// refund to the the account that paid the fees. If this fails, the
				// account might have dropped below the existential balance. In
				// that case we don't refund anything.
				let refund_imbalance = Balances::deposit_into_existing(fee_payer, refund_amount)
					.unwrap_or_else(|_| {
						<Balances as Currency<AccountId>>::PositiveImbalance::zero()
					});
				// merge the imbalance caused by paying the fees and refunding parts of it again.
				let adjusted_paid = paid
					.offset(refund_imbalance)
					.same()
					.map_err(|_| TransactionValidityError::Invalid(InvalidTransaction::Payment))?;
				// Call someone else to handle the imbalance (fee and tip separately)
				let (tip, fee) = adjusted_paid.split(tip);
				OU::on_unbalanceds(Some(fee).into_iter().chain(Some(tip)));
			}
		}
		Ok(())
	}
}

/// Runtime configuration for pallet_transaction_payment.
impl pallet_transaction_payment::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type OnChargeTransaction = TransactionCharger<()>;
	type WeightToFee = WeightToFee;
	type LengthToFee = ConstantMultiplier<Balance, TransactionByteFee>;
	type FeeMultiplierUpdate = SlowAdjustingFeeUpdate<Self>;
	type OperationalFeeMultiplier = OperationalFeeMultiplier;
}

parameter_types! {
	pub const ReservedXcmpWeight: Weight = MAXIMUM_BLOCK_WEIGHT.saturating_div(4);
	pub const ReservedDmpWeight: Weight = MAXIMUM_BLOCK_WEIGHT.saturating_div(4);
}

/// Runtime configuration for cumulus_pallet_parachain_system.
impl cumulus_pallet_parachain_system::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type OnSystemEvent = ();
	type SelfParaId = parachain_info::Pallet<Runtime>;
	type OutboundXcmpMessageSource = XcmpQueue;
	type DmpMessageHandler = DmpQueue;
	type ReservedDmpWeight = ReservedDmpWeight;
	type XcmpMessageHandler = XcmpQueue;
	type ReservedXcmpWeight = ReservedXcmpWeight;
	type CheckAssociatedRelayNumber = RelayNumberStrictlyIncreases;
}

/// Runtime configuration for parachain_info.
impl parachain_info::Config for Runtime {}

/// Runtime configuration for cumulus_pallet_aura_ext.
impl cumulus_pallet_aura_ext::Config for Runtime {}

/// Runtime configuration for cumulus_pallet_xcmp_queue.
impl cumulus_pallet_xcmp_queue::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type XcmExecutor = XcmExecutor<XcmConfig>;
	type ChannelInfo = ParachainSystem;
	type VersionWrapper = ();
	type ExecuteOverweightOrigin = EnsureAdminOrRoot;
	type ControllerOrigin = EnsureAdminOrRoot;
	type ControllerOriginConverter = XcmOriginToTransactDispatchOrigin;
	type PriceForSiblingDelivery = ();
	type WeightInfo = weight::cumulus_pallet_xcmp_queue::WeightInfo<Self>;
}

/// Runtime configuration for cumulus_pallet_dmp_queue.
impl cumulus_pallet_dmp_queue::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type XcmExecutor = XcmExecutor<XcmConfig>;
	type ExecuteOverweightOrigin = EnsureAdminOrRoot;
}

parameter_types! {
	pub const Period: u32 = 6 * HOURS;
	pub const Offset: u32 = 0;
	pub const MaxAuthorities: u32 = 100_000;
}

/// Runtime configuration for pallet_session.
impl pallet_session::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type ValidatorId = <Self as frame_system::Config>::AccountId;
	// we don't have stash and controller, thus we don't need the convert as well.
	type ValidatorIdOf = pallet_collator_selection::IdentityCollator;
	type ShouldEndSession = pallet_session::PeriodicSessions<Period, Offset>;
	type NextSessionRotation = pallet_session::PeriodicSessions<Period, Offset>;
	type SessionManager = CollatorSelection;
	// Essentially just Aura, but lets be pedantic.
	type SessionHandler = <SessionKeys as sp_runtime::traits::OpaqueKeys>::KeyTypeIdProviders;
	type Keys = SessionKeys;
	type WeightInfo = weight::pallet_session::WeightInfo<Runtime>;
}

/// Runtime configuration for pallet_aura.
impl pallet_aura::Config for Runtime {
	type AuthorityId = AuraId;
	type MaxAuthorities = MaxAuthorities;
	type DisabledValidators = ();
	type AllowMultipleBlocksPerSlot = ConstBool<false>;
}

parameter_types! {
	pub const PotId: PalletId = PalletId(*b"PotStake");
	pub const MaxCandidates: u32 = 1000;
	pub const MinCandidates: u32 = 5;
	pub const SessionLength: BlockNumber = 6 * HOURS;
	pub const MaxInvulnerables: u32 = 100;
	pub const ExecutiveBody: BodyId = BodyId::Executive;
	pub Admins: Vec<AccountId> = vec![];
}

// We allow root only to execute privileged collator selection operations.
pub type CollatorSelectionUpdateOrigin = EnsureAdminOrRoot;

/// Runtime configuration for pallet_collator_selection.
impl pallet_collator_selection::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type UpdateOrigin = CollatorSelectionUpdateOrigin;
	type PotId = PotId;
	type MaxCandidates = MaxCandidates;
	type MinCandidates = MinCandidates;
	type MaxInvulnerables = MaxInvulnerables;
	// should be a multiple of session or things will get inconsistent
	type KickThreshold = Period;
	type ValidatorId = <Self as frame_system::Config>::AccountId;
	type ValidatorIdOf = pallet_collator_selection::IdentityCollator;
	type AccountIdOf = pallet_collator_selection::IdentityCollator;
	type ValidatorRegistration = Session;
	type WeightInfo = weight::pallet_collator_selection::WeightInfo<Runtime>;
}

/// The permissioned multisig account `5DoK1CQfR86SLvmYxwBTvmavUAqfF5thayDpCvEnQDRd77Je`.
///
/// It consists of pre-generated 1-out-of-3 multisig account built from (in this order):
///
/// * Hex: `0x6ed85136cf2f86fafd38ca4655ce6188fb73fc0ca3e4eecd0eac38a4c87c0b41`
///
///   SS58: `5Ea3PESLfJ8uKbbbFNRyyitkfXBnPq8YAhRA4c7xg4o7cDaN`
///
/// * Hex: `0x94161be257be99009a7ee8d454c843f28f3ed9c720a07d284c07709fbcaffe06`
///
///   SS58: `5FQsWe1avwD29FFTJ3DtiDsJX6JGmjtW7vkaLes1QUUVdcPV`
///
/// * Hex: `0xeef4553e2fa8225cea907b6d467afbe05064a947afe54882a1085421e1d1ad66`
///
///   SS58: `5HU1qRoaEdeP4dNZU2JcPFNwE14SJvAWgXUfAFUqmdy4TdyQ`
const ADMIN_ACCOUNT_ID: AccountId = AccountId32::new([
	225, 96, 141, 169, 196, 68, 108, 63, 177, 69, 193, 246, 118, 195, 160, 124, 207, 95, 169, 146,
	34, 7, 154, 77, 28, 19, 179, 190, 41, 22, 66, 26,
]);

ord_parameter_types! {
	pub const RootAccountId: AccountId = AccountId32::new([0u8; 32]);

	pub const Admin: AccountId = ADMIN_ACCOUNT_ID;

	pub const CouncilAccountId: AccountId = ADMIN_ACCOUNT_ID;
	pub const TechCommitteeAccountId: AccountId = ADMIN_ACCOUNT_ID;
}

pub type EnsureAdminOrRoot =
	EitherOfDiverse<EnsureRoot<AccountId>, EnsureSignedBy<Admin, AccountId>>;

pub type InternalAssetId = u32;

parameter_types! {
	pub const AcurastPalletId: PalletId = PalletId(*b"acrstpid");
	pub const HyperdrivePalletId: PalletId = PalletId(*b"hyperpid");
	pub const FeeManagerPalletId: PalletId = PalletId(*b"acrstfee");
	pub const DefaultFeePercentage: sp_runtime::Percent = sp_runtime::Percent::from_percent(30);
	pub const DefaultMatcherFeePercentage: sp_runtime::Percent = sp_runtime::Percent::from_percent(10);
	pub const AcurastProcessorPackageNames: [&'static [u8]; 1] = [b"com.acurast.attested.executor.canary"];
	pub const ReportTolerance: u64 = 120_000;
}

/// Runtime configuration for pallet_acurast_fee_manager instance 1.
impl pallet_acurast_fee_manager::Config<pallet_acurast_fee_manager::Instance1> for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type DefaultFeePercentage = DefaultFeePercentage;
	type UpdateOrigin = EnsureAdminOrRoot;
	type WeightInfo = weight::pallet_acurast_fee_manager::WeightInfo<Self>;
}

/// Runtime configuration for pallet_acurast_fee_manager instance 2.
impl pallet_acurast_fee_manager::Config<pallet_acurast_fee_manager::Instance2> for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type DefaultFeePercentage = DefaultMatcherFeePercentage;
	type UpdateOrigin = EnsureAdminOrRoot;
	type WeightInfo = weight::pallet_acurast_fee_manager::WeightInfo<Self>;
}

/// Reward fee management implementation.
pub struct FeeManagement;
impl pallet_acurast_marketplace::FeeManager for FeeManagement {
	fn get_fee_percentage() -> sp_runtime::Percent {
		AcurastFeeManager::fee_percentage(AcurastFeeManager::fee_version())
	}

	fn get_matcher_percentage() -> sp_runtime::Percent {
		AcurastMatcherFeeManager::fee_percentage(AcurastMatcherFeeManager::fee_version())
	}

	fn pallet_id() -> PalletId {
		FeeManagerPalletId::get()
	}
}

/// Runtime configuration for pallet_acurast.
impl pallet_acurast::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type RegistrationExtra = ExtraFor<Self>;
	type MaxAllowedSources = MaxAllowedSources;
	type MaxCertificateRevocationListUpdates = frame_support::traits::ConstU32<10>;
	type MaxSlots = MaxSlots;
	type PalletId = AcurastPalletId;
	type MaxEnvVars = MaxEnvVars;
	type EnvKeyMaxSize = EnvKeyMaxSize;
	type EnvValueMaxSize = EnvValueMaxSize;
	type RevocationListUpdateBarrier = Barrier;
	type KeyAttestationBarrier = Barrier;
	type UnixTime = pallet_timestamp::Pallet<Runtime>;
	type JobHooks = pallet_acurast_marketplace::Pallet<Runtime>;
	type WeightInfo = weight::pallet_acurast::WeightInfo<Self>;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = benchmarking::AcurastBenchmarkHelper;
}

pub struct ManagerProvider;
impl pallet_acurast_marketplace::traits::ManagerProvider<Runtime> for ManagerProvider {
	fn manager_of(
		processor: &<Runtime as frame_system::Config>::AccountId,
	) -> Result<<Runtime as frame_system::Config>::AccountId, DispatchError> {
		match AcurastProcessorManager::manager_for_processor(processor) {
			Some(manager) => Ok(manager),
			None => Err(DispatchError::Other("Processor without manager.")),
		}
	}
}

pub struct ProcessorLastSeenProvider;
impl pallet_acurast_marketplace::traits::ProcessorLastSeenProvider<Runtime>
	for ProcessorLastSeenProvider
{
	fn last_seen(processor: &<Runtime as frame_system::Config>::AccountId) -> Option<u128> {
		AcurastProcessorManager::processor_last_seen(processor)
	}
}

/// Runtime configuration for pallet_acurast_marketplace.
impl pallet_acurast_marketplace::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type MaxAllowedConsumers = CU32<100>;
	type MaxProposedMatches = frame_support::traits::ConstU32<10>;
	type MaxFinalizeJobs = frame_support::traits::ConstU32<10>;
	type RegistrationExtra = ExtraFor<Self>;
	type PalletId = AcurastPalletId;
	type HyperdrivePalletId = HyperdrivePalletId;
	type ReportTolerance = ReportTolerance;
	type Balance = Balance;
	type RewardManager =
		pallet_acurast_marketplace::AssetRewardManager<FeeManagement, Balances, AcurastMarketplace>;
	type ManagerProvider = ManagerProvider;
	type ProcessorLastSeenProvider = ProcessorLastSeenProvider;
	type MarketplaceHooks = HyperdriveOutgoingMarketplaceHooks;
	type WeightInfo = weight::pallet_acurast_marketplace::WeightInfo<Self>;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = benchmarking::AcurastBenchmarkHelper;
}

pub struct HyperdriveOutgoingMarketplaceHooks;

impl MarketplaceHooks<Runtime> for HyperdriveOutgoingMarketplaceHooks {
	fn assign_job(job_id: &JobId<AccountId32>, pub_keys: &PubKeys) -> DispatchResultWithPostInfo {
		// inspect which hyperdrive-outgoing instance to be used
		let (origin, job_id_seq) = job_id;

		// depending on the origin=target chain to send message to, we search for a supported
		// processor public key supported on the target
		match origin {
			MultiOrigin::Acurast(_) => Ok(().into()), // nothing to be done for Acurast
			MultiOrigin::Tezos(_) => {
				let key = pub_keys
					.iter()
					.find(|key| match key {
						PubKey::SECP256r1(_) => true,
						_ => false,
					})
					.ok_or_else(|| DispatchError::Other("p256 public key does not exist"))?;

				AcurastHyperdriveOutgoingTezos::send_message(Action::AssignJob(
					job_id_seq.clone(),
					key.clone(),
				))
				.map_err(|_| DispatchError::Other("Could not send ASSIGN_JOB to tezos").into())
			},
			MultiOrigin::Ethereum(_) => {
				let key = pub_keys
					.iter()
					.find(|key| match key {
						PubKey::SECP256k1(_) => true,
						_ => false,
					})
					.ok_or_else(|| DispatchError::Other("k256 public key does not exist"))?;

				HyperdriveOutgoingEthereum::send_message(Action::AssignJob(
					job_id_seq.clone(),
					key.clone(),
				))
				.map_err(|_| DispatchError::Other("Could not send ASSIGN_JOB to ethereum").into())
			},
			MultiOrigin::AlephZero(_) => {
				let key = pub_keys
					.iter()
					.find(|key| match key {
						PubKey::SECP256k1(_) => true,
						_ => false,
					})
					.ok_or_else(|| DispatchError::Other("k256 public key does not exist"))?;

				HyperdriveOutgoingAlephZero::send_message(Action::AssignJob(
					job_id_seq.clone(),
					key.clone(),
				))
				.map_err(|_| DispatchError::Other("Could not send ASSIGN_JOB to ethereum").into())
			},
		}
	}

	fn finalize_job(
		job_id: &JobId<AccountId>,
		refund: <Runtime as pallet_acurast_marketplace::Config>::Balance,
	) -> DispatchResultWithPostInfo {
		// inspect which hyperdrive-outgoing instance to be used
		let (origin, job_id_seq) = job_id;

		match origin {
			MultiOrigin::Acurast(_) => Ok(().into()), // nothing to be done for Acurast
			MultiOrigin::Tezos(_) => AcurastHyperdriveOutgoingTezos::send_message(
				Action::FinalizeJob(job_id_seq.clone(), refund),
			)
			.map_err(|_| DispatchError::Other("Could not send FINALIZE_JOB to tezos").into()),
			MultiOrigin::Ethereum(_) => HyperdriveOutgoingEthereum::send_message(
				Action::FinalizeJob(job_id_seq.clone(), refund),
			)
			.map_err(|_| DispatchError::Other("Could not send FINALIZE_JOB to ethereum").into()),
			MultiOrigin::AlephZero(_) => HyperdriveOutgoingAlephZero::send_message(
				Action::FinalizeJob(job_id_seq.clone(), refund),
			)
			.map_err(|_| DispatchError::Other("Could not send FINALIZE_JOB to AlephZero").into()),
		}
	}
}

/// Struct use for various barrier implementations.
pub struct Barrier;

impl pallet_acurast::RevocationListUpdateBarrier<Runtime> for Barrier {
	fn can_update_revocation_list(
		origin: &<Runtime as frame_system::Config>::AccountId,
		_updates: &Vec<pallet_acurast::CertificateRevocationListUpdate>,
	) -> bool {
		let pallet_account: <Runtime as frame_system::Config>::AccountId =
			<Runtime as pallet_acurast::Config>::PalletId::get().into_account_truncating();
		&pallet_account == origin
	}
}

impl pallet_acurast::KeyAttestationBarrier<Runtime> for Barrier {
	fn accept_attestation_for_origin(
		_origin: &<Runtime as frame_system::Config>::AccountId,
		attestation: &pallet_acurast::Attestation,
	) -> bool {
		let attestation_application_id =
			attestation.key_description.tee_enforced.attestation_application_id.as_ref().or(
				attestation
					.key_description
					.software_enforced
					.attestation_application_id
					.as_ref(),
			);

		if let Some(attestation_application_id) = attestation_application_id {
			let package_names = attestation_application_id
				.package_infos
				.iter()
				.map(|package_info| package_info.package_name.as_slice())
				.collect::<Vec<_>>();
			let allowed = AcurastProcessorPackageNames::get();
			return package_names.iter().all(|package_name| allowed.contains(package_name))
		}

		false
	}
}

pub struct AdvertisementHandlerImpl;
impl pallet_acurast_processor_manager::AdvertisementHandler<Runtime> for AdvertisementHandlerImpl {
	fn advertise_for(
		processor: &<Runtime as frame_system::Config>::AccountId,
		advertisement: &<Runtime as pallet_acurast_processor_manager::Config>::Advertisement,
	) -> sp_runtime::DispatchResult {
		AcurastMarketplace::do_advertise(processor, advertisement)
	}
}

impl pallet_acurast_processor_manager::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Proof = MultiSignature;
	type ManagerId = u128;
	type ManagerIdProvider = AcurastManagerIdProvider;
	type ProcessorAssetRecovery = AcurastProcessorRecovery;
	type MaxPairingUpdates = ConstU32<20>;
	type MaxProcessorsInSetUpdateInfo = ConstU32<100>;
	type Counter = u64;
	type PairingProofExpirationTime = ConstU128<600000>;
	type UnixTime = pallet_timestamp::Pallet<Runtime>;
	type Advertisement = pallet_acurast_marketplace::AdvertisementFor<Self>;
	type AdvertisementHandler = AdvertisementHandlerImpl;
	type WeightInfo = weight::pallet_acurast_processor_manager::WeightInfo<Self>;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = benchmarking::AcurastBenchmarkHelper;
}

parameter_types! {
	pub const ManagerCollectionId: u128 = 0;
}

pub struct AcurastManagerIdProvider;

impl pallet_acurast_processor_manager::ManagerIdProvider<Runtime> for AcurastManagerIdProvider {
	fn create_manager_id(
		id: <Runtime as pallet_acurast_processor_manager::Config>::ManagerId,
		owner: &<Runtime as frame_system::Config>::AccountId,
	) -> frame_support::pallet_prelude::DispatchResult {
		if Uniques::collection_owner(ManagerCollectionId::get()).is_none() {
			Uniques::create_collection(
				&ManagerCollectionId::get(),
				&RootAccountId::get(),
				&RootAccountId::get(),
			)?;
		}
		Uniques::do_mint(ManagerCollectionId::get(), id, owner.clone(), |_| Ok(()))
	}

	fn manager_id_for(
		owner: &<Runtime as frame_system::Config>::AccountId,
	) -> Result<
		<Runtime as pallet_acurast_processor_manager::Config>::ManagerId,
		sp_runtime::DispatchError,
	> {
		Uniques::owned_in_collection(&ManagerCollectionId::get(), owner)
			.nth(0)
			.ok_or(frame_support::pallet_prelude::DispatchError::Other("Manager ID not found"))
	}

	fn owner_for(
		manager_id: <Runtime as pallet_acurast_processor_manager::Config>::ManagerId,
	) -> Result<
		<Runtime as frame_system::Config>::AccountId,
		frame_support::pallet_prelude::DispatchError,
	> {
		Uniques::owner(ManagerCollectionId::get(), manager_id).ok_or(
			frame_support::pallet_prelude::DispatchError::Other(
				"Onwer for provided Manager ID not found",
			),
		)
	}
}

pub struct AcurastProcessorRecovery;

impl pallet_acurast_processor_manager::ProcessorAssetRecovery<Runtime>
	for AcurastProcessorRecovery
{
	fn recover_assets(
		processor: &<Runtime as frame_system::Config>::AccountId,
		destination_account: &<Runtime as frame_system::Config>::AccountId,
	) -> frame_support::pallet_prelude::DispatchResult {
		let usable_balance = <Balances as Inspect<_>>::reducible_balance(
			processor,
			Preservation::Preserve,
			Fortitude::Polite,
		);
		if usable_balance > 0 {
			let burned = <Balances as Mutate<_>>::burn_from(
				processor,
				usable_balance,
				Precision::BestEffort,
				Fortitude::Polite,
			)?;
			Balances::mint_into(destination_account, burned)?;
		}

		Ok(())
	}
}

parameter_types! {
	pub const TransmissionQuorum: u8 = 1;
	pub const TransmissionRate: u64 = 1;

	pub const EthereumSnapshotRate: u64 = 10;
	pub const AlephZeroSnapshotRate: u64 = 1;

	pub const MaximumBlocksBeforeSnapshot: u32 = 2;

	pub const TezosNativeAssetId: u128 = 5000;
}

pub struct AcurastActionExecutor<T: pallet_acurast::Config>(PhantomData<T>);
impl pallet_acurast_hyperdrive::ActionExecutor<Runtime> for AcurastActionExecutor<Runtime> {
	fn execute(action: ParsedAction<Runtime>) -> DispatchResultWithPostInfo {
		match action {
			ParsedAction::RegisterJob(job_id, registration) =>
				Acurast::register_for(job_id, registration.into()),
			ParsedAction::DeregisterJob(job_id) => Acurast::deregister_for(job_id).into(),
			ParsedAction::FinalizeJob(job_ids) =>
				AcurastMarketplace::finalize_jobs_for(job_ids.into_iter()),
			ParsedAction::SetJobEnvironment(job_id, environments) => {
				for (source, env) in environments {
					Acurast::set_environment_for(job_id.clone(), source, env)?;
				}
				Ok(().into())
			},
			ParsedAction::Noop => {
				// Intentionally, just logging it
				log::debug!("Received NOOP operation from hyperdrive");

				Ok(().into())
			},
		}
	}
}

const INITIAL_TEZOS_HYPERDRIVE_CONTRACT: [u8; 28] = [
	5, 10, 0, 0, 0, 22, 1, 49, 237, 39, 148, 99, 33, 189, 254, 22, 221, 131, 88, 161, 99, 215, 210,
	149, 151, 199, 158, 0,
];

parameter_types! {
	/// The initial Tezos Hyperdrive address:
	///
	/// Corresponds to `KT1D8kmxQgZiMJjFLp5L1mkYQaysHyat7v7h`, packed: `0x050a000000160131ed27946321bdfe16dd8358a163d7d29597c79e00`
	pub TezosContract: StateOwner = INITIAL_TEZOS_HYPERDRIVE_CONTRACT.to_vec().try_into().unwrap();
	/// The acurast gateway on the ethereum network
	pub EthereumAcurastGateway: StateOwner = hex_literal::hex!("6a34E1f07B57eD968e72895690f3df41b11487eb").to_vec().try_into().unwrap();
	/// The acurast gateway on the aleph zero network (Not necessary)
	pub AlephZeroAcurastGateway: StateOwner = vec![].try_into().unwrap();
}

impl pallet_acurast_hyperdrive::Config<TezosInstance> for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type ParsableAccountId = AcurastAccountId;
	type TargetChainOwner = TezosContract;
	type TargetChainHash = H256;
	type TargetChainBlockNumber = u64;
	type Balance = Balance;
	type MaxTransmittersPerSnapshot = CU32<64>;
	type TargetChainHashing = sp_runtime::traits::Keccak256;
	type TransmissionRate = TransmissionRate;
	type TransmissionQuorum = TransmissionQuorum;
	type ActionExecutor = AcurastActionExecutor<Runtime>;
	type Proof = pallet_acurast_hyperdrive::chain::tezos::TezosProof<
		AcurastAccountId,
		<Self as frame_system::Config>::AccountId,
	>;
	type WeightInfo = weight::pallet_acurast_hyperdrive::WeightInfo<Runtime>;
}

impl pallet_acurast_hyperdrive::Config<EthereumInstance> for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type ParsableAccountId = AcurastAccountId;
	type TargetChainOwner = EthereumAcurastGateway;
	type TargetChainHash = H256;
	type TargetChainBlockNumber = u64;
	type Balance = Balance;
	type MaxTransmittersPerSnapshot = CU32<64>;
	type TargetChainHashing = sp_runtime::traits::Keccak256;
	type TransmissionRate = EthereumSnapshotRate;
	type TransmissionQuorum = TransmissionQuorum;
	type ActionExecutor = AcurastActionExecutor<Runtime>;
	type Proof = pallet_acurast_hyperdrive::chain::ethereum::EthereumProof<Self, AcurastAccountId>;
	type WeightInfo = weight::pallet_acurast_hyperdrive::WeightInfo<Runtime>;
}

impl pallet_acurast_hyperdrive::Config<AlephZeroInstance> for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type ParsableAccountId = AcurastAccountId;
	type TargetChainOwner = AlephZeroAcurastGateway;
	type TargetChainHash = H256;
	type TargetChainBlockNumber = u64;
	type Balance = Balance;
	type MaxTransmittersPerSnapshot = CU32<64>;
	type TargetChainHashing = sp_runtime::traits::Keccak256;
	type TransmissionRate = AlephZeroSnapshotRate;
	type TransmissionQuorum = TransmissionQuorum;
	type ActionExecutor = AcurastActionExecutor<Runtime>;
	type Proof = pallet_acurast_hyperdrive::chain::substrate::SubstrateProof<
		AcurastAccountId,
		<Self as frame_system::Config>::AccountId,
	>;
	type WeightInfo = weight::pallet_acurast_hyperdrive::WeightInfo<Runtime>;
}

impl pallet_acurast_hyperdrive_outgoing::Config<TezosInstance> for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type MMRInfo = TezosInstance;
	type TargetChainConfig = DefaultTezosConfig;
	type MaximumBlocksBeforeSnapshot = MaximumBlocksBeforeSnapshot;
	type OnNewRoot = ();
	type WeightInfo = weights::TezosHyperdriveOutgoingWeight;
}

impl pallet_acurast_hyperdrive_outgoing::Config<EthereumInstance> for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type MMRInfo = EthereumInstance;
	type TargetChainConfig = pallet_acurast_hyperdrive_outgoing::chain::ethereum::EthereumConfig;
	type MaximumBlocksBeforeSnapshot = MaximumBlocksBeforeSnapshot;
	type OnNewRoot = ();
	type WeightInfo = weights::TezosHyperdriveOutgoingWeight;
}

impl pallet_acurast_hyperdrive_outgoing::Config<AlephZeroInstance> for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type MMRInfo = AlephZeroInstance;
	type TargetChainConfig = pallet_acurast_hyperdrive_outgoing::chain::alephzero::AlephZeroConfig;
	type MaximumBlocksBeforeSnapshot = MaximumBlocksBeforeSnapshot;
	type OnNewRoot = ();
	type WeightInfo = weights::TezosHyperdriveOutgoingWeight;
}

parameter_types! {
	pub const Epoch: BlockNumber = 131072;
	pub Treasury: AccountId = FeeManagerPalletId::get().into_account_truncating();
}

impl pallet_acurast_rewards_treasury::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Epoch = Epoch;
	type Treasury = Treasury;
}

/// Runtime configuration for pallet_sudo.
impl pallet_sudo::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type RuntimeCall = RuntimeCall;
	type WeightInfo = pallet_sudo::weights::SubstrateWeight<Runtime>;
}

parameter_types! {
	pub const PreimageMaxSize: u32 = 4096 * 1024;
	pub const PreimageBaseDeposit: Balance = 1 * UNIT;
	pub const PreimageByteDeposit: Balance = 1 * MICROUNIT;
}

/// Runtime configuration for pallet_preimage.
impl pallet_preimage::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = pallet_preimage::weights::SubstrateWeight<Runtime>;
	type Currency = Balances;
	type ManagerOrigin = EnsureAdminOrRoot;
	type BaseDeposit = PreimageBaseDeposit;
	type ByteDeposit = PreimageByteDeposit;
}

parameter_types! {
	pub MaximumSchedulerWeight: Weight = Weight::from_parts(10_000_000, 0);
	pub const MaxScheduledPerBlock: u32 = 50;
}

/// Runtime configuration for pallet_scheduler.
impl pallet_scheduler::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type RuntimeOrigin = RuntimeOrigin;
	type PalletsOrigin = OriginCaller;
	type RuntimeCall = RuntimeCall;
	type MaximumWeight = MaximumSchedulerWeight;
	type ScheduleOrigin = EnsureAdminOrRoot;
	type MaxScheduledPerBlock = MaxScheduledPerBlock;
	type WeightInfo = pallet_scheduler::weights::SubstrateWeight<Self>;
	type OriginPrivilegeCmp = frame_support::traits::EqualPrivilegeOnly;
	type Preimages = Preimage;
}

impl pallet_uniques::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type CollectionId = u128;
	type ItemId = u128;
	type Currency = Balances;
	type ForceOrigin = EnsureAdminOrRoot;
	type CreateOrigin =
		AsEnsureOriginWithArg<EnsureRootWithSuccess<Self::AccountId, RootAccountId>>;
	type Locker = ();
	type CollectionDeposit = ConstU128<0>;
	type ItemDeposit = ConstU128<0>;
	type MetadataDepositBase = ConstU128<0>;
	type AttributeDepositBase = ConstU128<0>;
	type DepositPerByte = ConstU128<0>;
	type StringLimit = ConstU32<256>;
	type KeyLimit = ConstU32<256>;
	type ValueLimit = ConstU32<256>;
	#[cfg(feature = "runtime-benchmarks")]
	type Helper = ();
	type WeightInfo = pallet_uniques::weights::SubstrateWeight<Self>;
}

// The purpose of this offset is to ensure that a democratic proposal will not apply in the same
// block as a round change.
const ENACTMENT_OFFSET: u32 = 900;

impl pallet_democracy::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type EnactmentPeriod = ConstU32<{ 2 * DAYS + ENACTMENT_OFFSET }>;
	type LaunchPeriod = ConstU32<{ 7 * DAYS }>;
	type VotingPeriod = ConstU32<{ 14 * DAYS }>;
	type VoteLockingPeriod = ConstU32<{ 7 * DAYS }>;
	type FastTrackVotingPeriod = ConstU32<{ 1 * DAYS }>;
	type MinimumDeposit = ConstU128<{ 4 * UNIT * SUPPLY_FACTOR }>;

	type ExternalOrigin = EnsureSignedBy<CouncilAccountId, AccountId>;
	type ExternalMajorityOrigin = EnsureSignedBy<CouncilAccountId, AccountId>;
	type ExternalDefaultOrigin = EnsureSignedBy<CouncilAccountId, AccountId>;
	type SubmitOrigin =
		EnsureWithSuccess<EnsureSignedBy<RootAccountId, AccountId>, AccountId, RootAccountId>;
	type FastTrackOrigin = EnsureSignedBy<TechCommitteeAccountId, AccountId>;
	type InstantOrigin = EnsureSignedBy<TechCommitteeAccountId, AccountId>;
	type CancellationOrigin =
		EitherOfDiverse<EnsureRoot<AccountId>, EnsureSignedBy<CouncilAccountId, AccountId>>;
	type BlacklistOrigin = EnsureAdminOrRoot;
	type CancelProposalOrigin =
		EitherOfDiverse<EnsureRoot<AccountId>, EnsureSignedBy<TechCommitteeAccountId, AccountId>>;
	type VetoOrigin =
		EnsureWithSuccess<EnsureSignedBy<RootAccountId, AccountId>, AccountId, RootAccountId>;

	type CooloffPeriod = ConstU32<{ 7 * DAYS }>;
	type Slash = ();
	type InstantAllowed = ConstBool<true>;
	type Scheduler = Scheduler;
	type MaxVotes = ConstU32<100>;
	type PalletsOrigin = OriginCaller;
	type WeightInfo = pallet_democracy::weights::SubstrateWeight<Runtime>;
	type MaxProposals = ConstU32<100>;
	type Preimages = Preimage;
	type MaxDeposits = ConstU32<100>;
	type MaxBlacklisted = ConstU32<100>;
}

parameter_types! {
	// One storage item; key size is 32; value is size 4+4+16+20 bytes = 44 bytes.
	pub const DepositBase: Balance = deposit(1, 76);
	// Additional storage item size of 32 bytes.
	pub const DepositFactor: Balance = deposit(0, 20);
	pub const MaxSignatories: u32 = 100;
}

impl pallet_multisig::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type RuntimeCall = RuntimeCall;
	type Currency = Balances;
	type DepositBase = DepositBase;
	type DepositFactor = DepositFactor;
	type MaxSignatories = MaxSignatories;
	type WeightInfo = pallet_multisig::weights::SubstrateWeight<Runtime>;
}

impl pallet_utility::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type RuntimeCall = RuntimeCall;
	type PalletsOrigin = OriginCaller;
	type WeightInfo = pallet_utility::weights::SubstrateWeight<Runtime>;
}

// Create the runtime by composing the FRAME pallets that were previously configured.
construct_runtime!(
	pub enum Runtime
	{
		// System support stuff.
		System: frame_system::{Pallet, Call, Config<T>, Storage, Event<T>} = 0,
		ParachainSystem: cumulus_pallet_parachain_system::{
			Pallet, Call, Config<T>, Storage, Inherent, Event<T>, ValidateUnsigned,
		} = 1,
		Timestamp: pallet_timestamp::{Pallet, Call, Storage, Inherent} = 2,
		ParachainInfo: parachain_info::{Pallet, Storage, Config<T>} = 3,
		Sudo: pallet_sudo = 4,
		Scheduler: pallet_scheduler = 5,
		Preimage: pallet_preimage = 6,
		Multisig: pallet_multisig::{Pallet, Call, Storage, Event<T>} = 7,
		Utility: pallet_utility = 8,

		// Monetary stuff.
		Balances: pallet_balances::{Pallet, Call, Storage, Config<T>, Event<T>} = 10,
		TransactionPayment: pallet_transaction_payment::{Pallet, Storage, Event<T>} = 11,
		// (keep comment, just so we know that pallet assets used to be on this pallet index)
		// Assets: pallet_assets::{Pallet, Storage, Event<T>, Config<T>} = 12,
		Uniques: pallet_uniques::{Pallet, Storage, Event<T>, Call} = 14,

		// Governance stuff.
		Democracy: pallet_democracy::{Pallet, Storage, Config<T>, Event<T>, Call} = 15,

		// Consensus. The order of these are important and shall not change.
		Authorship: pallet_authorship::{Pallet, Storage} = 20,
		CollatorSelection: pallet_collator_selection::{Pallet, Call, Storage, Event<T>, Config<T>} = 21,
		Session: pallet_session::{Pallet, Call, Storage, Event, Config<T>} = 22,
		Aura: pallet_aura::{Pallet, Storage, Config<T>} = 23,
		AuraExt: cumulus_pallet_aura_ext::{Pallet, Storage, Config<T>} = 24,

		// XCM helpers.
		XcmpQueue: cumulus_pallet_xcmp_queue::{Pallet, Call, Storage, Event<T>} = 30,
		PolkadotXcm: pallet_xcm::{Pallet, Call, Event<T>, Origin, Config<T>} = 31,
		CumulusXcm: cumulus_pallet_xcm::{Pallet, Event<T>, Origin} = 32,
		DmpQueue: cumulus_pallet_dmp_queue::{Pallet, Call, Storage, Event<T>} = 33,

		// Acurast pallets
		Acurast: pallet_acurast::{Pallet, Call, Storage, Event<T>, Config<T>} = 40,
		AcurastProcessorManager: pallet_acurast_processor_manager::{Pallet, Call, Storage, Event<T>, Config<T>} = 41,
		AcurastFeeManager: pallet_acurast_fee_manager::<Instance1>::{Pallet, Call, Storage, Event<T>} = 42,
		AcurastMarketplace: pallet_acurast_marketplace::{Pallet, Call, Storage, Event<T>} = 43,
		AcurastMatcherFeeManager: pallet_acurast_fee_manager::<Instance2>::{Pallet, Call, Storage, Event<T>} = 44,
		// Hyperdrive (one instance for each connected chain)
		AcurastHyperdriveTezos: pallet_acurast_hyperdrive::<Instance1>::{Pallet, Call, Storage, Event<T>} = 45,
		// The instance here has to correspond to `pallet_acurast_hyperdrive_outgoing::instances::tezos::TargetChainTezos` (we can't use a reference there...)
		AcurastHyperdriveOutgoingTezos: pallet_acurast_hyperdrive_outgoing::<Instance1>::{Pallet, Call, Storage, Event<T>} = 46,
		AcurastRewardsTreasury: pallet_acurast_rewards_treasury::{Pallet, Storage, Event<T>} = 47,
		HyperdriveEthereum: pallet_acurast_hyperdrive::<Instance2>::{Pallet, Call, Storage, Event<T>} = 48,
		HyperdriveOutgoingEthereum: pallet_acurast_hyperdrive_outgoing::<Instance2>::{Pallet, Call, Storage, Event<T>} = 49,
		HyperdriveAlephZero: pallet_acurast_hyperdrive::<Instance3>::{Pallet, Call, Storage, Event<T>} = 50,
		HyperdriveOutgoingAlephZero: pallet_acurast_hyperdrive_outgoing::<Instance3>::{Pallet, Call, Storage, Event<T>} = 51,
	}
);

#[cfg(feature = "runtime-benchmarks")]
#[macro_use]
extern crate frame_benchmarking;

#[cfg(feature = "runtime-benchmarks")]
mod benches {
	define_benchmarks!(
		// TODO uncomment with fixed version of cumulus-pallet-parachain-system that includes PR https://github.com/paritytech/cumulus/pull/2766/files
		// [frame_system, SystemBench::<Runtime>]
		[pallet_balances, Balances]
		[pallet_session, SessionBench::<Runtime>]
		[pallet_timestamp, Timestamp]
		[pallet_collator_selection, CollatorSelection]
		[cumulus_pallet_xcmp_queue, XcmpQueue]
		[pallet_acurast, Acurast]
		[pallet_acurast_processor_manager, AcurastProcessorManager]
		[pallet_acurast_fee_manager, AcurastFeeManager]
		[pallet_acurast_marketplace, AcurastMarketplace]
		[pallet_acurast_hyperdrive, AcurastHyperdriveTezos]
		[pallet_acurast_hyperdrive_outgoing, AcurastHyperdriveOutgoingTezos]
		[pallet_acurast_hyperdrive_outgoing, HyperdriveOutgoingEthereum]
	);
}

impl_runtime_apis! {
	impl sp_consensus_aura::AuraApi<Block, AuraId> for Runtime {
		fn slot_duration() -> sp_consensus_aura::SlotDuration {
			sp_consensus_aura::SlotDuration::from_millis(Aura::slot_duration())
		}

		fn authorities() -> Vec<AuraId> {
			Aura::authorities().into_inner()
		}
	}

	impl sp_api::Core<Block> for Runtime {
		fn version() -> RuntimeVersion {
			VERSION
		}

		fn execute_block(block: Block) {
			Executive::execute_block(block)
		}

		fn initialize_block(header: &<Block as BlockT>::Header) {
			Executive::initialize_block(header)
		}
	}

	impl sp_api::Metadata<Block> for Runtime {
		fn metadata() -> OpaqueMetadata {
			OpaqueMetadata::new(Runtime::metadata().into())
		}

		fn metadata_at_version(version: u32) -> Option<OpaqueMetadata> {
			Runtime::metadata_at_version(version)
		}

		fn metadata_versions() -> sp_std::vec::Vec<u32> {
			Runtime::metadata_versions()
		}
	}

	impl sp_block_builder::BlockBuilder<Block> for Runtime {
		fn apply_extrinsic(extrinsic: <Block as BlockT>::Extrinsic) -> ApplyExtrinsicResult {
			Executive::apply_extrinsic(extrinsic)
		}

		fn finalize_block() -> <Block as BlockT>::Header {
			Executive::finalize_block()
		}

		fn inherent_extrinsics(data: sp_inherents::InherentData) -> Vec<<Block as BlockT>::Extrinsic> {
			data.create_extrinsics()
		}

		fn check_inherents(
			block: Block,
			data: sp_inherents::InherentData,
		) -> sp_inherents::CheckInherentsResult {
			data.check_extrinsics(&block)
		}
	}

	impl sp_transaction_pool::runtime_api::TaggedTransactionQueue<Block> for Runtime {
		fn validate_transaction(
			source: TransactionSource,
			tx: <Block as BlockT>::Extrinsic,
			block_hash: <Block as BlockT>::Hash,
		) -> TransactionValidity {
			Executive::validate_transaction(source, tx, block_hash)
		}
	}

	impl sp_offchain::OffchainWorkerApi<Block> for Runtime {
		fn offchain_worker(header: &<Block as BlockT>::Header) {
			Executive::offchain_worker(header)
		}
	}

	impl sp_session::SessionKeys<Block> for Runtime {
		fn generate_session_keys(seed: Option<Vec<u8>>) -> Vec<u8> {
			SessionKeys::generate(seed)
		}

		fn decode_session_keys(
			encoded: Vec<u8>,
		) -> Option<Vec<(Vec<u8>, KeyTypeId)>> {
			SessionKeys::decode_into_raw_public_keys(&encoded)
		}
	}

	impl frame_system_rpc_runtime_api::AccountNonceApi<Block, AccountId, Nonce> for Runtime {
		fn account_nonce(account: AccountId) -> Nonce {
			System::account_nonce(account)
		}
	}

	impl pallet_transaction_payment_rpc_runtime_api::TransactionPaymentApi<Block, Balance> for Runtime {
		fn query_info(
			uxt: <Block as BlockT>::Extrinsic,
			len: u32,
		) -> pallet_transaction_payment_rpc_runtime_api::RuntimeDispatchInfo<Balance> {
			TransactionPayment::query_info(uxt, len)
		}
		fn query_fee_details(
			uxt: <Block as BlockT>::Extrinsic,
			len: u32,
		) -> pallet_transaction_payment::FeeDetails<Balance> {
			TransactionPayment::query_fee_details(uxt, len)
		}
		fn query_weight_to_fee(weight: Weight) -> Balance {
			TransactionPayment::weight_to_fee(weight)
		}
		fn query_length_to_fee(length: u32) -> Balance {
			TransactionPayment::length_to_fee(length)
		}
	}

	impl pallet_transaction_payment_rpc_runtime_api::TransactionPaymentCallApi<Block, Balance, RuntimeCall>
		for Runtime
	{
		fn query_call_info(
			call: RuntimeCall,
			len: u32,
		) -> pallet_transaction_payment::RuntimeDispatchInfo<Balance> {
			TransactionPayment::query_call_info(call, len)
		}
		fn query_call_fee_details(
			call: RuntimeCall,
			len: u32,
		) -> pallet_transaction_payment::FeeDetails<Balance> {
			TransactionPayment::query_call_fee_details(call, len)
		}
		fn query_weight_to_fee(weight: Weight) -> Balance {
			TransactionPayment::weight_to_fee(weight)
		}
		fn query_length_to_fee(length: u32) -> Balance {
			TransactionPayment::length_to_fee(length)
		}
	}


	impl pallet_acurast_hyperdrive_outgoing::HyperdriveApi<Block, H256> for Runtime {
		fn number_of_leaves(instance: HyperdriveInstance) -> LeafIndex {
			match instance {
				HyperdriveInstance::Tezos => AcurastHyperdriveOutgoingTezos::number_of_leaves(),
				HyperdriveInstance::Ethereum => HyperdriveOutgoingEthereum::number_of_leaves(),
				HyperdriveInstance::AlephZero => HyperdriveOutgoingAlephZero::number_of_leaves(),
			}
		}

		fn first_mmr_block_number(instance: HyperdriveInstance) -> Option<NumberFor<Block>> {
			match instance {
				HyperdriveInstance::Tezos => AcurastHyperdriveOutgoingTezos::first_mmr_block_number(),
				HyperdriveInstance::Ethereum => HyperdriveOutgoingEthereum::first_mmr_block_number(),
				HyperdriveInstance::AlephZero => HyperdriveOutgoingAlephZero::first_mmr_block_number(),
			}
		}

		fn leaf_meta(instance: HyperdriveInstance, leaf_index: LeafIndex) -> Option<(<Block as BlockT>::Hash, H256)> {
			match instance {
				HyperdriveInstance::Tezos => AcurastHyperdriveOutgoingTezos::leaf_meta(leaf_index),
				HyperdriveInstance::Ethereum => HyperdriveOutgoingEthereum::leaf_meta(leaf_index),
				HyperdriveInstance::AlephZero => HyperdriveOutgoingAlephZero::leaf_meta(leaf_index),
			}
		}

		fn last_message_excl_by_block(instance: HyperdriveInstance, block_number: NumberFor<Block>) -> Option<LeafIndex> {
			match instance {
				HyperdriveInstance::Tezos => AcurastHyperdriveOutgoingTezos::block_leaf_index(block_number),
				HyperdriveInstance::Ethereum => HyperdriveOutgoingEthereum::block_leaf_index(block_number),
				HyperdriveInstance::AlephZero => HyperdriveOutgoingAlephZero::block_leaf_index(block_number),
			}
		}

		fn snapshot_roots(instance: HyperdriveInstance, next_expected_snapshot_number: SnapshotNumber) -> Result<Vec<(SnapshotNumber, <Block as BlockT>::Hash)>, MMRError> {
			match instance {
				HyperdriveInstance::Tezos => AcurastHyperdriveOutgoingTezos::snapshot_roots(next_expected_snapshot_number).collect(),
				HyperdriveInstance::Ethereum => HyperdriveOutgoingEthereum::snapshot_roots(next_expected_snapshot_number).collect(),
				HyperdriveInstance::AlephZero => HyperdriveOutgoingAlephZero::snapshot_roots(next_expected_snapshot_number).collect(),
			}
		}

		fn snapshot_root(instance: HyperdriveInstance, next_expected_snapshot_number: SnapshotNumber) -> Result<Option<(SnapshotNumber, <Block as BlockT>::Hash)>, MMRError> {
			match instance {
				HyperdriveInstance::Tezos => AcurastHyperdriveOutgoingTezos::snapshot_roots(next_expected_snapshot_number).next().transpose(),
				HyperdriveInstance::Ethereum => HyperdriveOutgoingEthereum::snapshot_roots(next_expected_snapshot_number).next().transpose(),
				HyperdriveInstance::AlephZero => HyperdriveOutgoingAlephZero::snapshot_roots(next_expected_snapshot_number).next().transpose(),
			}
		}

		fn generate_target_chain_proof(
			instance: HyperdriveInstance,
			next_message_number: LeafIndex,
			maximum_messages: Option<u64>,
			latest_known_snapshot_number: SnapshotNumber,
		) -> Result<Option<TargetChainProof<H256>>, MMRError> {
			match instance {
				HyperdriveInstance::Tezos => AcurastHyperdriveOutgoingTezos::generate_target_chain_proof(next_message_number, maximum_messages, latest_known_snapshot_number),
				HyperdriveInstance::Ethereum => HyperdriveOutgoingEthereum::generate_target_chain_proof(next_message_number, maximum_messages, latest_known_snapshot_number),
				HyperdriveInstance::AlephZero => HyperdriveOutgoingAlephZero::generate_target_chain_proof(next_message_number, maximum_messages, latest_known_snapshot_number),
			}
		}
	}

	impl pallet_acurast_marketplace::MarketplaceRuntimeApi<Block, Balance, AccountId, MaxAllowedSources> for Runtime {
		fn filter_matching_sources(
			registration: PartialJobRegistration<Balance, AccountId, MaxAllowedSources>,
			sources: Vec<AccountId>,
			consumer: Option<MultiOrigin<AccountId>>,
			latest_seen_after: Option<u128>,
		) -> Result<Vec<AccountId>, RuntimeApiError> {
			AcurastMarketplace::filter_matching_sources(registration, sources, consumer, latest_seen_after)
		}
	}

	impl cumulus_primitives_core::CollectCollationInfo<Block> for Runtime {
		fn collect_collation_info(header: &<Block as BlockT>::Header) -> cumulus_primitives_core::CollationInfo {
			ParachainSystem::collect_collation_info(header)
		}
	}

	#[cfg(feature = "try-runtime")]
	impl frame_try_runtime::TryRuntime<Block> for Runtime {
		fn on_runtime_upgrade(checks: bool) -> (Weight, Weight) {
			log::info!("try-runtime::on_runtime_upgrade parachain-acurast.");
			let weight = Executive::try_runtime_upgrade(checks).unwrap();
			(weight, RuntimeBlockWeights::get().max_block)
		}

		fn execute_block(block: Block, state_root_check: bool, signature_check: bool, try_state: frame_try_runtime::TryStateSelect) -> Weight {
			log::info!(
				target: "runtime::parachain-acurast", "try-runtime: executing block #{} ({:?}) / root checks: {:?} / sanity-checks: {:?}",
				block.header.number,
				block.header.hash(),
				state_root_check,
				try_state,
			);
			Executive::try_execute_block(block, state_root_check, signature_check, try_state).expect("try_execute_block failed")
		}
	}

	#[cfg(feature = "runtime-benchmarks")]
	impl frame_benchmarking::Benchmark<Block> for Runtime {
		fn benchmark_metadata(extra: bool) -> (
			Vec<frame_benchmarking::BenchmarkList>,
			Vec<frame_support::traits::StorageInfo>,
		) {
			use frame_benchmarking::{Benchmarking, BenchmarkList};
			use frame_support::traits::StorageInfoTrait;
			use cumulus_pallet_session_benchmarking::Pallet as SessionBench;

			let mut list = Vec::<BenchmarkList>::new();
			list_benchmarks!(list, extra);

			let storage_info = AllPalletsWithSystem::storage_info();
			return (list, storage_info)
		}

		fn dispatch_benchmark(
			config: frame_benchmarking::BenchmarkConfig
		) -> Result<Vec<frame_benchmarking::BenchmarkBatch>, sp_runtime::RuntimeString> {
			use frame_benchmarking::{Benchmarking, BenchmarkBatch, TrackedStorageKey};

			impl frame_system_benchmarking::Config for Runtime {
				// TODO uncomment with fixed version of cumulus-pallet-parachain-system that includes PR https://github.com/paritytech/cumulus/pull/2766/files
				// fn setup_set_code_requirements(code: &sp_std::vec::Vec<u8>) -> Result<(), BenchmarkError> {
				// 	ParachainSystem::initialize_for_set_code_benchmark(code.len() as u32);
				// 	 Ok(())
				// }
				//
				// fn verify_set_code() {
				// 	System::assert_last_event(cumulus_pallet_parachain_system::Event::<Runtime>::ValidationFunctionStored.into());
				// }
			}

			use cumulus_pallet_session_benchmarking::Pallet as SessionBench;
			impl cumulus_pallet_session_benchmarking::Config for Runtime {}

			let whitelist: Vec<TrackedStorageKey> = vec![
				// Block Number
				hex_literal::hex!("26aa394eea5630e07c48ae0c9558cef702a5c1b19ab7a04f536c519aca4983ac").to_vec().into(),
				// Total Issuance
				hex_literal::hex!("c2261276cc9d1f8598ea4b6a74b15c2f57c875e4cff74148e4628f264b974c80").to_vec().into(),
				// Execution Phase
				hex_literal::hex!("26aa394eea5630e07c48ae0c9558cef7ff553b5a9862a516939d82b3d3d8661a").to_vec().into(),
				// Event Count
				hex_literal::hex!("26aa394eea5630e07c48ae0c9558cef70a98fdbe9ce6c55837576c60c7af3850").to_vec().into(),
				// System Events
				hex_literal::hex!("26aa394eea5630e07c48ae0c9558cef780d41e5e16056765bc8461851072c9d7").to_vec().into(),
			];

			let mut batches = Vec::<BenchmarkBatch>::new();
			let params = (&config, &whitelist);
			add_benchmarks!(params, batches);

			if batches.is_empty() { return Err("Benchmark not found for this pallet.".into()) }
			Ok(batches)
		}
	}
}

struct CheckInherents;

impl cumulus_pallet_parachain_system::CheckInherents<Block> for CheckInherents {
	fn check_inherents(
		block: &Block,
		relay_state_proof: &cumulus_pallet_parachain_system::RelayChainStateProof,
	) -> sp_inherents::CheckInherentsResult {
		let relay_chain_slot = relay_state_proof
			.read_slot()
			.expect("Could not read the relay chain slot from the proof");

		let inherent_data =
			cumulus_primitives_timestamp::InherentDataProvider::from_relay_chain_slot_and_duration(
				relay_chain_slot,
				sp_std::time::Duration::from_secs(6),
			)
			.create_inherent_data()
			.expect("Could not create the timestamp inherent data");

		inherent_data.check_extrinsics(block)
	}
}

cumulus_pallet_parachain_system::register_validate_block! {
	Runtime = Runtime,
	BlockExecutor = cumulus_pallet_aura_ext::BlockExecutor::<Runtime, Executive>,
	CheckInherents = CheckInherents,
}

#[cfg(test)]
mod tests {
	use super::*;
	use sp_core::ByteArray;
	use std::str::FromStr;

	#[test]
	fn create() {
		// Public key bytes corresponding to account `0x0458ad576b404c1aa5404f2f8da1932a22ee3c0cd42e7cf567706d24201fbd1c`
		let multisig_member1: AccountId =
			AccountId32::from_str("5CAQPebv8ZzDk8pYR5mzWsUzamcsYxMgWuv5rMAtzrWTcgh1").unwrap();
		// Public key bytes corresponding to account `0x0c3638b65541bcb16d29a38a7ff5fc7983978b5fa315aa7da528f05210e96f61`
		let multisig_member2: AccountId =
			AccountId32::from_str("5CLiYDEbpsdH8o6bYW6tDMfHi4NdsMWTmQ2WnsdU4H9CzcaL").unwrap();
		// Public key bytes corresponding to account `0x10de214612b271e2cfee25f121222d6423fa722487ff2fe1cb9a42ff28407578`
		let multisig_member3: AccountId =
			AccountId32::from_str("5CSpcKHjBhPLBEcwh9a2jBagT2PVoAqnjMZ3xBY9n44G5Voo").unwrap();
		let multisig_account =
			Multisig::multi_account_id(&[multisig_member1, multisig_member2, multisig_member3], 2);

		println!("{:?}", multisig_account.to_string());
		println!("{:?}", multisig_account.as_slice());

		assert_eq!(ADMIN_ACCOUNT_ID.as_slice(), multisig_account.as_slice());
		assert_eq!(
			"5HADK95FVMQRjh4uVFtGumgMdMgVqvtNQ3AGYpB9BNFjHVaZ",
			multisig_account.to_string()
		);
	}
}
