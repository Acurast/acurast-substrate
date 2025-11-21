#![allow(unused_imports)]

use crate::{
	chain::tezos::TezosParser, stub::AcurastAccountId, types::RawIncomingAction, weights,
	ActionExecutor, ParsedAction, ProxyAddress, StateProof, StateProofNode,
};
use derive_more::{Display, From, Into};
use frame_support::{
	derive_impl,
	instances::Instance1,
	pallet_prelude::*,
	parameter_types,
	traits::{ConstU16, ConstU64},
	Deserialize, PalletId, Serialize,
};
use frame_system::{self as system, EnsureRoot};
use hex_literal::hex;
use pallet_acurast::{MessageBody, MessageProcessor, ProxyChain, ProxyAcurastChain, CU32};
use pallet_acurast_marketplace::RegistrationExtra;
use cumulus_primitives_core::ParaId;
use sp_core::*;
use sp_runtime::{
	traits::{BlakeTwo256, IdentityLookup, Keccak256},
	AccountId32, BuildStorage, MultiSignature,
};
use sp_std::prelude::*;

type Block = frame_system::mocking::MockBlock<Test>;

parameter_types! {
	pub const TransmissionRate: u64 = 5;
	pub const TransmissionQuorum: u8 = 2;

	pub const AcurastPalletId: PalletId = PalletId(*b"acrstpid");
	pub const MinimumPeriod: u64 = 2000;

    /// The acurast contract on the aleph zero network
	pub AlephZeroContract: AccountId = hex_literal::hex!("e2ab38a7567ec7e9cb208ffff65ea5b5a610a6f1cc7560a27d61b47223d6baa3").into();
	pub AlephZeroContractSelector: [u8; 4] = hex_literal::hex!("7cd99c82");
	pub VaraContract: AccountId = hex_literal::hex!("e2ab38a7567ec7e9cb208ffff65ea5b5a610a6f1cc7560a27d61b47223d6baa3").into(); // TODO(vara)
	pub AcurastPalletAccount: AccountId = AcurastPalletId::get().into_account_truncating();
	pub HyperdriveIbcFeePalletAccount: AccountId = HyperdriveIbcFeePalletId::get().into_account_truncating();

    pub MinTTL: BlockNumber = 15;
    pub IncomingTTL: BlockNumber = 50;
	pub MinDeliveryConfirmationSignatures: u32 = 1;
	pub MinReceiptConfirmationSignatures: u32 = 1;
	pub const MinFee: Balance = 1;
	pub const ParachainId: ParaId = ParaId::new(2000);
	pub const SelfChain: ProxyAcurastChain = ProxyAcurastChain::Acurast;
}

// Configure a mock runtime to test the pallet.
frame_support::construct_runtime!(
	pub enum Test {
		System: frame_system,
		Timestamp: pallet_timestamp::{Pallet, Call, Storage, Inherent},
		Acurast: pallet_acurast::{Pallet, Call, Storage, Event<T>},
		AcurastHyperdrive: crate::{Pallet, Call, Storage, Event<T>},
        AcurastHyperdriveIbc: pallet_acurast_hyperdrive_ibc::{Pallet, Call, Storage, Event<T>},
	}
);

#[derive_impl(frame_system::config_preludes::ParaChainDefaultConfig as frame_system::DefaultConfig)]
impl frame_system::Config for Test {
	type AccountId = AccountId32;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Nonce = u64;
	type Hash = H256;
	type Block = Block;
	type BlockHashCount = ConstU64<250>;
	type Version = ();
	type AccountData = ();
	type DbWeight = ();
	type BlockWeights = ();
	type BlockLength = ();
	type SS58Prefix = ConstU16<42>;
	type OnSetCode = ();
	type MaxConsumers = ConstU32<16>;
}

impl pallet_timestamp::Config for Test {
	type Moment = u64;
	type OnTimestampSet = ();
	type MinimumPeriod = MinimumPeriod;
	type WeightInfo = ();
}

pub type MaxAllowedSources = CU32<4>;
pub type MaxSlots = CU32<64>;

impl pallet_acurast::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type RegistrationExtra =
		RegistrationExtra<Balance, <Self as frame_system::Config>::AccountId, Self::MaxSlots>;
	type MaxAllowedSources = MaxAllowedSources;
	type MaxCertificateRevocationListUpdates = frame_support::traits::ConstU32<10>;
	type MaxSlots = MaxSlots;
	type PalletId = AcurastPalletId;
	type MaxEnvVars = CU32<10>;
	type EnvKeyMaxSize = CU32<32>;
	type EnvValueMaxSize = CU32<1024>;
	type RevocationListUpdateBarrier = ();
	type KeyAttestationBarrier = ();
	type UnixTime = pallet_timestamp::Pallet<Test>;
	type JobHooks = ();
	type WeightInfo = pallet_acurast::weights::WeightInfo<Test>;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = benchmarking::AcurastBenchmarkHelper;
}

impl crate::Config<Instance1> for Test {
	type RuntimeEvent = RuntimeEvent;
    type ActionExecutor = ();
	type Sender = AcurastPalletAccount;
	type ParsableAccountId = AcurastAccountId;
	type AlephZeroContract = AlephZeroContract;
	type AlephZeroContractSelector = AlephZeroContractSelector;
	type VaraContract = VaraContract;
	type Balance = Balance;
	type WeightInfo = weights::WeightInfo<Test>;
}

impl pallet_acurast_hyperdrive_ibc::Config<Instance1> for Test {
	type RuntimeEvent = RuntimeEvent;
    type MinTTL = MinTTL;
    type IncomingTTL = IncomingTTL;
	type MinDeliveryConfirmationSignatures = MinDeliveryConfirmationSignatures;
	type MinReceiptConfirmationSignatures = MinReceiptConfirmationSignatures;
	type MinFee = MinFee;
	type Currency = Balances;
	type RuntimeHoldReason = RuntimeHoldReason;
	type MessageIdHashing = BlakeTwo256;
	type MessageProcessor = HyperdriveMessageProcessor;
	type UpdateOrigin = EnsureRoot<Self::AccountId>;
	type ParachainId = ParachainId;
	type SelfChain = SelfChain;
	type WeightInfo = weights::WeightInfo<Test>;
}

// Build genesis storage according to the mock runtime.
pub fn new_test_ext() -> sp_io::TestExternalities {
	let storage = system::GenesisConfig::<Test>::default().build_storage().unwrap().into();

	let mut ext = sp_io::TestExternalities::new(storage);
	ext.execute_with(|| System::set_block_number(1));
	ext
}

pub fn events() -> Vec<RuntimeEvent> {
	log::debug!("{:#?}", System::events());
	let evt = System::events().into_iter().map(|evt| evt.event).collect::<Vec<_>>();

	System::reset_events();

	evt
}

pub type Balance = u128;

impl<T: pallet_acurast::Config> ActionExecutor<T> for () {
	fn execute(_: ParsedAction<T>) -> DispatchResultWithPostInfo {
		Ok(().into())
	}
}
