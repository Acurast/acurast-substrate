use cumulus_primitives_core::{AggregateMessageOrigin, Weight};
use frame_support::{
	ord_parameter_types, pallet_prelude::DispatchClass, parameter_types,
	weights::constants::WEIGHT_REF_TIME_PER_SECOND, PalletId,
};
use frame_system::limits::{BlockLength, BlockWeights};
use sp_runtime::{traits::AccountIdConversion, AccountId32, Perbill};
use sp_std::prelude::*;
use sp_version::RuntimeVersion;

use acurast_runtime_common::{
	constants::{HOURS, MICROUNIT, MILLIUNIT, UNIT},
	types::{AccountId, Balance, BlockNumber},
	weight::{BlockExecutionWeight, ExtrinsicBaseWeight},
};

use crate::{apis::RUNTIME_API_VERSIONS, deposit, RuntimeHoldReason};

#[sp_version::runtime_version]
pub const VERSION: RuntimeVersion = RuntimeVersion {
	spec_name: sp_std::borrow::Cow::Borrowed("acurast-parachain"),
	impl_name: sp_std::borrow::Cow::Borrowed("acurast-parachain"),
	authoring_version: 1,
	spec_version: 5,
	impl_version: 1,
	apis: RUNTIME_API_VERSIONS,
	transaction_version: 1,
	system_version: 1,
};

// Provide a common factor between runtimes based on a supply of 1_000_000_000_000 tokens == 1 UNIT.
pub const SUPPLY_FACTOR: Balance = 1;

pub const STORAGE_BYTE_FEE: Balance = 100 * MICROUNIT * SUPPLY_FACTOR;

/// The existential deposit. Set to 1/10 of the Connected Relay Chain.
pub const EXISTENTIAL_DEPOSIT: Balance = MILLIUNIT;

/// We assume that ~5% of the block weight is consumed by `on_initialize` handlers. This is
/// used to limit the maximal weight of a single extrinsic.
pub const AVERAGE_ON_INITIALIZE_RATIO: Perbill = Perbill::from_percent(5);

/// We allow `Normal` extrinsics to fill up the block up to 75%, the rest can be used by
/// `Operational` extrinsics.
pub const NORMAL_DISPATCH_RATIO: Perbill = Perbill::from_percent(75);

/// We allow for 2 seconds of compute with a 12 second average block time.
pub const MAXIMUM_BLOCK_WEIGHT: Weight = Weight::from_parts(
	WEIGHT_REF_TIME_PER_SECOND.saturating_mul(2),
	cumulus_primitives_core::relay_chain::MAX_POV_SIZE as u64,
);

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

	pub const MinimumPeriod: u64 = 0; //SLOT_DURATION / 2;

	pub const ExistentialDeposit: Balance = EXISTENTIAL_DEPOSIT;
	pub const MaxLocks: u32 = 50;
	pub const MaxReserves: u32 = 50;
	/// Relay Chain `TransactionByteFee` / 10
	pub const TransactionByteFee: Balance = 10 * MICROUNIT;
	pub const OperationalFeeMultiplier: u8 = 5;

	pub const ReservedXcmpWeight: Weight = MAXIMUM_BLOCK_WEIGHT.saturating_div(4);
	pub const ReservedDmpWeight: Weight = MAXIMUM_BLOCK_WEIGHT.saturating_div(4);
	pub const RelayOrigin: AggregateMessageOrigin = AggregateMessageOrigin::Parent;

	pub MessageQueueServiceWeight: Weight = Perbill::from_percent(35) * RuntimeBlockWeights::get().max_block;

	pub const Period: u32 = 6 * HOURS;
	pub const Offset: u32 = 0;
	pub const MaxAuthorities: u32 = 100_000;

	pub const PotId: PalletId = PalletId(*b"PotStake");
	pub const MaxCandidates: u32 = 20;
	pub const MinCandidates: u32 = 4;
	pub const MaxInvulnerables: u32 = 100;

	pub const AcurastPalletId: PalletId = PalletId(*b"acrstpid");
	pub const HyperdrivePalletId: PalletId = PalletId(*b"hyperpid");
	pub const HyperdriveIbcFeePalletId: PalletId = PalletId(*b"hyibcfee");
	pub HyperdriveTokenPalletAccount: AccountId = PalletId(*b"hyptoken").into_account_truncating();
	pub HyperdriveTokenEthereumVault: AccountId = PalletId(*b"hyptveth").into_account_truncating();
	pub HyperdriveTokenEthereumFeeVault: AccountId = PalletId(*b"hyptfeth").into_account_truncating();
	pub HyperdriveTokenSolanaVault: AccountId = PalletId(*b"hyptvsol").into_account_truncating();
	pub OperationalFeeAccount: AccountId = HyperdriveTokenPalletAccount::get();
	pub HyperdriveTokenSolanaFeeVault: AccountId = PalletId(*b"hyptfsol").into_account_truncating();
	pub const FeeManagerPalletId: PalletId = PalletId(*b"acrstfee");
	pub const ComputePalletId: PalletId = PalletId(*b"cmptepid");

	pub const DefaultFeePercentage: sp_runtime::Percent = sp_runtime::Percent::from_percent(30);
	pub const DefaultMatcherFeePercentage: sp_runtime::Percent = sp_runtime::Percent::from_percent(10);
	pub const CorePackageName: &'static [u8] = b"com.acurast.attested.executor.mainnet";
	pub const LitePackageName: &'static [u8] = b"com.acurast.attested.executor.sbs.mainnet";
	pub const CorePackageNameCanary: &'static [u8] = b"com.acurast.attested.executor.canary";
	pub const LitePackageNameCanary: &'static [u8] = b"com.acurast.attested.executor.sbs.canary";
	pub const BundleId: &'static [u8] = b"GV2452922R.com.acurast.executor";
	pub const CoreSignatureDigest: &'static [u8] = hex_literal::hex!("ec70c2a4e072a0f586552a68357b23697c9d45f1e1257a8c4d29a25ac4982433").as_slice();
	pub const LiteSignatureDigest: &'static [u8] = hex_literal::hex!("ea21af13f3b724c662f3da05247acc5a68a45331a90220f0d90a6024d7fa8f36").as_slice();
	pub const LiteSolSignatureDigest: &'static [u8] = hex_literal::hex!("e095733f011ae6934a02d65a0945fcf24c16af7598c1c23405dcc4f3cb9ee5bc").as_slice();
	pub PackageNames: Vec<&'static [u8]> = vec![CorePackageName::get(), LitePackageName::get(), CorePackageNameCanary::get(), LitePackageNameCanary::get()];
	pub BundleIds: Vec<&'static [u8]> = vec![BundleId::get()];
	pub LitePackageNames: Vec<&'static [u8]> = vec![LitePackageName::get(), LitePackageNameCanary::get()];
	pub CorePackageNames: Vec<&'static [u8]> = vec![CorePackageName::get(), CorePackageNameCanary::get()];
	pub SignatureDigests: Vec<&'static [u8]> = vec![CoreSignatureDigest::get(), LiteSignatureDigest::get(), LiteSolSignatureDigest::get()];
	pub LiteSignatureDigests: Vec<&'static [u8]> = vec![LiteSignatureDigest::get(), LiteSolSignatureDigest::get()];
	pub CoreSignatureDigests: Vec<&'static [u8]> = vec![CoreSignatureDigest::get()];
	pub const ReportTolerance: u64 = 120_000;

	pub const ManagerCollectionId: u128 = 0;
	pub const CommitmentCollectionId: u128 = 1;

	/// The acurast contract on the aleph zero network
	pub AlephZeroContract: AccountId = hex_literal::hex!("e2ab38a7567ec7e9cb208ffff65ea5b5a610a6f1cc7560a27d61b47223d6baa3").into();
	pub AlephZeroContractSelector: [u8; 4] = hex_literal::hex!("7cd99c82");
	pub VaraContract: AccountId = hex_literal::hex!("e2ab38a7567ec7e9cb208ffff65ea5b5a610a6f1cc7560a27d61b47223d6baa3").into(); // TODO(vara)
	pub AcurastPalletAccount: AccountId = AcurastPalletId::get().into_account_truncating();
	pub HyperdriveIbcFeePalletAccount: AccountId = HyperdriveIbcFeePalletId::get().into_account_truncating();

	pub MinTTL: BlockNumber = 15;
	pub IncomingTTL: BlockNumber = 50;
	pub OutgoingTransferTTL: BlockNumber = 50;
	pub MinDeliveryConfirmationSignatures: u32 = 1;
	pub MinReceiptConfirmationSignatures: u32 = 1;

	pub const Epoch: BlockNumber = 131072;

	pub const PreimageMaxSize: u32 = 4096 * 1024;
	pub const PreimageBaseDeposit: Balance = UNIT / 10;
	pub const PreimageByteDeposit: Balance = MICROUNIT;
	pub const PreimageHoldReason: RuntimeHoldReason = RuntimeHoldReason::Preimage(pallet_preimage::HoldReason::Preimage);

	pub MaximumSchedulerWeight: Weight = Perbill::from_percent(80) * RuntimeBlockWeights::get().max_block;
	pub const MaxScheduledPerBlock: u32 = 50;

	// One storage item; key size is 32; value is size 4+4+16+20 bytes = 44 bytes.
	pub const DepositBase: Balance = deposit(1, 76);
	// Additional storage item size of 32 bytes.
	pub const DepositFactor: Balance = deposit(0, 20);
	pub const MaxSignatories: u32 = 100;
}

ord_parameter_types! {
	pub const RootAccountId: AccountId = AccountId32::new([0u8; 32]);
}
