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
use frame_system as system;
use hex_literal::hex;
use pallet_acurast::CU32;
use pallet_acurast_marketplace::RegistrationExtra;
use sp_core::*;
use sp_runtime::{
	traits::{BlakeTwo256, IdentityLookup, Keccak256},
	AccountId32, BuildStorage, MultiSignature,
};
use sp_std::prelude::*;

type Block = frame_system::mocking::MockBlock<Test>;

parameter_types! {
	pub TargetChainProxyAddress: ProxyAddress = ProxyAddress::try_from(hex!("050a0000001600009f7f36d0241d3e6a82254216d7de5780aa67d8f9").to_vec()).unwrap();
	pub const TransmissionRate: u64 = 5;
	pub const TransmissionQuorum: u8 = 2;

	pub const AcurastPalletId: PalletId = PalletId(*b"acrstpid");
	pub const MinimumPeriod: u64 = 2000;
}

// Configure a mock runtime to test the pallet.
frame_support::construct_runtime!(
	pub enum Test {
		System: frame_system,
		Timestamp: pallet_timestamp::{Pallet, Call, Storage, Inherent},
		Acurast: pallet_acurast::{Pallet, Call, Storage, Event<T>},
		TezosHyperdrive: crate::<Instance1>,
		EthereumHyperdrive: crate::<Instance2>,
		AlephZeroHyperdrive: crate::<Instance3>,
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

impl crate::Config<TezosInstance> for Test {
	type RuntimeEvent = RuntimeEvent;
	type ParsableAccountId = AcurastAccountId;
	type AlephZeroContract = TargetChainProxyAddress;
	type TargetChainHash = H256;
	type TargetChainBlockNumber = u64;
	type Balance = Balance;
	type MaxTransmittersPerSnapshot = CU32<64>;
	type TargetChainHashing = Keccak256;
	type TransmissionRate = TransmissionRate;
	type TransmissionQuorum = TransmissionQuorum;
	type ActionExecutor = ();
	type Proof = crate::chain::tezos::TezosProof<
		Self::ParsableAccountId,
		<Self as frame_system::Config>::AccountId,
	>;
	type WeightInfo = weights::WeightInfo<Test>;
}

impl crate::Config<EthereumInstance> for Test {
	type RuntimeEvent = RuntimeEvent;
	type ParsableAccountId = AcurastAccountId;
	type AlephZeroContract = TargetChainProxyAddress;
	type TargetChainHash = H256;
	type TargetChainBlockNumber = u64;
	type Balance = Balance;
	type MaxTransmittersPerSnapshot = CU32<64>;
	type TargetChainHashing = Keccak256;
	type TransmissionRate = TransmissionRate;
	type TransmissionQuorum = TransmissionQuorum;
	type ActionExecutor = ();
	type Proof = crate::chain::ethereum::EthereumProof<Self, AcurastAccountId>;
	type WeightInfo = weights::WeightInfo<Test>;
}

impl crate::Config<Instance1> for Test {
	type RuntimeEvent = RuntimeEvent;
	type ParsableAccountId = AcurastAccountId;
	type AlephZeroContract = TargetChainProxyAddress;
	type TargetChainHash = H256;
	type TargetChainBlockNumber = u64;
	type Balance = Balance;
	type MaxTransmittersPerSnapshot = CU32<64>;
	type TargetChainHashing = Keccak256;
	type TransmissionRate = TransmissionRate;
	type TransmissionQuorum = TransmissionQuorum;
	type ActionExecutor = ();
	type Proof = crate::chain::substrate::SubstrateMessageDecoder<
		Self::ParsableAccountId,
		<Self as frame_system::Config>::AccountId,
	>;
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
