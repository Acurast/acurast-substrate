use cumulus_primitives_core::{AggregateMessageOrigin, Weight};
use frame_support::{
	ord_parameter_types, pallet_prelude::DispatchClass, parameter_types,
	weights::constants::WEIGHT_REF_TIME_PER_SECOND, PalletId,
};
use frame_system::limits::{BlockLength, BlockWeights};
use sp_runtime::{traits::AccountIdConversion, AccountId32, Perbill};
use sp_std::prelude::*;
use sp_version::RuntimeVersion;
use xcm::latest::prelude::BodyId;

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
	spec_version: 2,
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

// 5CGV1Sm6Qzt3s5qabiDAEjni6xT15MZ8LumkVPob4SJqAN7C
pub const MULTISIG_MEMBER_1: AccountId = AccountId32::new([
	8, 251, 177, 59, 216, 3, 125, 72, 41, 72, 88, 240, 125, 15, 17, 172, 138, 10, 183, 215, 145,
	119, 239, 89, 112, 0, 234, 105, 99, 217, 189, 5,
]);
// 5DFhdYCuTc4uubFu6XGpiF5uKu6e7erNZa6QKExZDRFMTuv8
pub const MULTISIG_MEMBER_2: AccountId = AccountId32::new([
	52, 159, 43, 188, 132, 82, 63, 226, 222, 8, 14, 207, 143, 150, 49, 52, 42, 104, 136, 25, 84,
	85, 140, 149, 166, 63, 56, 72, 146, 113, 28, 72,
]);
// 5DXDTbjLtDDUXzFy24Fhkjs9fY3PQwQR2ohzjQPT1JvUAcEy
pub const MULTISIG_MEMBER_3: AccountId = AccountId32::new([
	64, 116, 67, 152, 92, 197, 129, 25, 49, 53, 219, 79, 187, 35, 129, 198, 57, 185, 238, 224, 105,
	205, 104, 222, 248, 190, 10, 144, 70, 218, 69, 16,
]);
// 5Dt7iJBxvWztigqXiXqm8EU5xVBWcUrfXA5am1e8sF1RjUuW
pub const MULTISIG_MEMBER_4: AccountId = AccountId32::new([
	80, 101, 18, 161, 124, 202, 251, 175, 7, 64, 113, 14, 71, 239, 46, 123, 244, 227, 85, 74, 132,
	46, 86, 125, 95, 175, 176, 198, 50, 128, 245, 55,
]);
// 5EEe4WLNneqz3Fp2n61ZcTiGU6GLEvUgVmnkKaaxARSdVpdg
pub const MULTISIG_MEMBER_5: AccountId = AccountId32::new([
	96, 12, 36, 143, 93, 8, 144, 90, 166, 15, 141, 0, 105, 112, 187, 145, 231, 152, 24, 235, 48,
	13, 180, 130, 11, 159, 109, 239, 177, 70, 179, 72,
]);
// 5EUnFHHEFd4mzTA6cjg8JfKHeteCDrcEhMdxUXSK3QzHSPe8
pub const MULTISIG_MEMBER_6: AccountId = AccountId32::new([
	106, 213, 34, 94, 7, 36, 164, 241, 250, 180, 251, 154, 234, 18, 223, 61, 158, 96, 7, 95, 187,
	186, 210, 166, 202, 181, 151, 62, 172, 25, 43, 24,
]);
// 5EbvNf3q5Xb918UvHBuB6rPfYuom38QAqw8osV5TQeaELWxP
pub const MULTISIG_MEMBER_7: AccountId = AccountId32::new([
	112, 71, 53, 229, 240, 87, 176, 45, 195, 169, 238, 86, 24, 175, 3, 43, 201, 20, 148, 172, 219,
	44, 84, 65, 29, 111, 162, 87, 219, 163, 245, 118,
]);

/// The permissioned multisig account `5E9Yq3ViHdMdtw8qdixTkDKQcNKJ9wbJ1pDoEPRZL8WUW41j`.
///
/// It consists of pre-generated 3-out-of-7 multisig account built from the members defined above (in this order).
pub const ADMIN_ACCOUNT_ID: AccountId = AccountId32::new([
	92, 42, 77, 139, 255, 132, 23, 211, 77, 53, 218, 186, 69, 60, 178, 234, 114, 214, 95, 185, 189,
	32, 227, 175, 88, 174, 120, 110, 220, 237, 199, 129,
]);

// The purpose of this offset is to ensure that a democratic proposal will not apply in the same
// block as a round change.
pub const ENACTMENT_OFFSET: u32 = 900;

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
	pub const ExecutiveBody: BodyId = BodyId::Executive;

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
	pub const TreasuryPalletId: PalletId = PalletId(*b"trsrypid");

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
	pub OutgoingTransferTTL: BlockNumber = 15;
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
	pub const Admin: AccountId = ADMIN_ACCOUNT_ID;
}
