use frame_support::{
	derive_impl, parameter_types,
	traits::{ConstU16, ConstU32, ConstU64},
};
use parity_scale_codec::Encode;
use sp_core::{sr25519::Pair, Pair as PairTrait, H256};
use sp_runtime::{
	traits::{ConvertInto, IdentityLookup},
	AccountId32, BuildStorage, MultiSignature,
};

#[cfg(feature = "runtime-benchmarks")]
use crate::benchmarking::BenchmarkHelper;

pub type AccountId = AccountId32;
type Block = frame_system::mocking::MockBlock<Test>;
pub type Balance = u128;
pub type BlockNumber = u32;

#[derive(Default)]
pub struct ExtBuilder;

impl ExtBuilder {
	pub fn build(self) -> sp_io::TestExternalities {
		let t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();

		let mut ext = sp_io::TestExternalities::new(t);
		ext.execute_with(|| System::set_block_number(1));
		ext
	}
}

pub const EXISTENTIAL_DEPOSIT: Balance = MILLIUNIT;
pub const UNIT: Balance = 1_000_000;
pub const MILLIUNIT: Balance = UNIT / 1_000;

pub const MILLISECS_PER_BLOCK: u64 = 6000;

pub const MINUTES: BlockNumber = 60_000 / (MILLISECS_PER_BLOCK as BlockNumber);
pub const HOURS: BlockNumber = MINUTES * 60;
pub const DAYS: BlockNumber = HOURS * 24;

frame_support::construct_runtime!(
	pub enum Test {
		System: frame_system::{Pallet, Call, Config<T>, Storage, Event<T>} = 0,
		Timestamp: pallet_timestamp::{Pallet, Call, Storage, Inherent},
		Balances: pallet_balances::{Pallet, Call, Storage, Config<T>, Event<T>},
		AcurastTokenClaim: crate::{Pallet, Call, Storage, Event<T>}
	}
);

parameter_types! {
	pub const BlockHashCount: BlockNumber = 2400;
	pub const MinimumPeriod: u64 = 6000;
	pub const ExistentialDeposit: Balance = EXISTENTIAL_DEPOSIT;
	pub const MaxReserves: u32 = 50;
	pub const MaxLocks: u32 = 50;
	pub const LockDuration: BlockNumber = 48 * 28 * DAYS;
	pub Funder: AccountId = generate_pair_account("Alice").1;
	pub Signer: AccountId = generate_pair_account("Bob").1;
}

#[derive_impl(frame_system::config_preludes::ParaChainDefaultConfig as frame_system::DefaultConfig)]
impl frame_system::Config for Test {
	type AccountId = AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Nonce = u64;
	type Hash = H256;
	type Block = Block;
	type BlockHashCount = ConstU64<250>;
	type Version = ();
	type AccountData = pallet_balances::AccountData<Balance>;
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

impl pallet_balances::Config for Test {
	/// The type for recording an account's balance.
	type Balance = Balance;
	type DustRemoval = ();
	/// The ubiquitous event type.
	type RuntimeEvent = RuntimeEvent;
	type ExistentialDeposit = ExistentialDeposit;
	type AccountStore = System;
	type WeightInfo = pallet_balances::weights::SubstrateWeight<Test>;
	type MaxLocks = MaxLocks;
	type MaxReserves = MaxReserves;
	type ReserveIdentifier = [u8; 8];
	type RuntimeHoldReason = RuntimeHoldReason;
	type RuntimeFreezeReason = RuntimeFreezeReason;
	type FreezeIdentifier = RuntimeFreezeReason;
	type MaxFreezes = ConstU32<50>;
	type DoneSlashHandler = ();
}

impl crate::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type Signature = MultiSignature;
	type Signer = Signer;
	type Funder = Funder;
	type VestingDuration = LockDuration;
	type BlockNumberToBalance = ConvertInto;
	type WeightInfo = crate::weights::WeightInfo<Self>;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = ();
}

pub fn events() -> Vec<RuntimeEvent> {
	let evt = System::events().into_iter().map(|evt| evt.event).collect::<Vec<_>>();

	System::reset_events();

	evt
}

pub fn generate_pair_account(seed: &str) -> (Pair, AccountId) {
	let pair =
		Pair::from_string(&format!("//{}", seed), None).expect("static values are valid; qed");
	let account_id: AccountId = pair.public().into();

	(pair, account_id)
}

pub fn generate_signature(signer: &Pair, account: &AccountId, amount: Balance) -> MultiSignature {
	let message =
		[b"<Bytes>".to_vec(), account.encode(), amount.encode(), b"</Bytes>".to_vec()].concat();
	signer.sign(&message).into()
}

#[cfg(feature = "runtime-benchmarks")]
impl BenchmarkHelper<Test> for () {
	fn dummy_signature() -> MultiSignature {
		use sp_core::crypto::UncheckedFrom;

		MultiSignature::Sr25519(sp_core::sr25519::Signature::unchecked_from([0u8; 64]))
	}
}
